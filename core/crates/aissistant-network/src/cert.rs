//! X.509 certificate generation with Ed25519 identity keys and custom pinned peer verifiers for mutual TLS 1.3.

use aissistant_security::DeviceIdentity;
use rcgen::{CertificateParams, DistinguishedName as RcgenDn, DnType, KeyPair, PKCS_ED25519};
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::server::danger::{ClientCertVerified, ClientCertVerifier};
use rustls::{DigitallySignedStruct, DistinguishedName, Error as RustlsError, SignatureScheme};

use crate::discovery::format_fingerprint;
use crate::error::{NetworkError, Result};

/// SPKI header pattern for Ed25519 keys according to RFC 8410.
///
/// 0x30, 0x05: AlgorithmIdentifier SEQUENCE (len 5)
/// 0x06, 0x03, 0x2b, 0x65, 0x70: OID 1.3.101.112 (id-Ed25519)
/// 0x03, 0x21, 0x00: BIT STRING (len 33, 0 unused bits)
pub const ED25519_SPKI_PREFIX: [u8; 10] =
    [0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x03, 0x21, 0x00];

/// Encodes an Ed25519 32-byte secret key into standard PKCS#8 v1 DER format.
#[must_use]
pub fn ed25519_secret_to_pkcs8(secret_bytes: &[u8; 32]) -> [u8; 48] {
    let mut pkcs8 = [0u8; 48];
    // Header: SEQUENCE(46), INTEGER(0), SEQUENCE(5), OID(id-Ed25519), OCTET_STRING(34), OCTET_STRING(32)
    let header: [u8; 16] = [
        0x30, 0x2e, 0x02, 0x01, 0x00, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x04, 0x22, 0x04,
        0x20,
    ];
    pkcs8[..16].copy_from_slice(&header);
    pkcs8[16..48].copy_from_slice(secret_bytes);
    pkcs8
}

/// Extracts the 32-byte raw Ed25519 public key from a DER-encoded X.509 certificate.
#[must_use]
pub fn extract_ed25519_public_key(cert_der: &[u8]) -> Option<[u8; 32]> {
    let prefix_len = ED25519_SPKI_PREFIX.len();
    if cert_der.len() < prefix_len + 32 {
        return None;
    }

    for i in 0..=cert_der.len() - (prefix_len + 32) {
        if cert_der[i..i + prefix_len] == ED25519_SPKI_PREFIX {
            let key_start = i + prefix_len;
            let mut key = [0u8; 32];
            key.copy_from_slice(&cert_der[key_start..key_start + 32]);
            return Some(key);
        }
    }
    None
}

/// Generates a self-signed TLS 1.3 certificate and private key DER for Quinn/Rustls from a `DeviceIdentity`.
pub fn generate_tls_cert(
    identity: &DeviceIdentity,
    san_name: &str,
) -> Result<(
    CertificateDer<'static>,
    rustls_pki_types::PrivateKeyDer<'static>,
)> {
    let secret = identity.secret_bytes();
    let pkcs8 = ed25519_secret_to_pkcs8(&secret);

    let pkcs8_der = rustls_pki_types::PrivatePkcs8KeyDer::from(pkcs8.as_slice());
    let key_pair =
        KeyPair::from_pkcs8_der_and_sign_algo(&pkcs8_der, &PKCS_ED25519).map_err(|e| {
            NetworkError::Internal(format!("Failed to parse Ed25519 keypair into rcgen: {e}"))
        })?;

    let mut params = CertificateParams::new(vec![san_name.to_string()])
        .map_err(|e| NetworkError::Internal(format!("Failed to create cert params: {e}")))?;

    let mut dn = RcgenDn::new();
    dn.push(DnType::CommonName, san_name);
    params.distinguished_name = dn;

    let cert = params
        .self_signed(&key_pair)
        .map_err(|e| NetworkError::Internal(format!("Failed to sign cert: {e}")))?;

    let cert_der = CertificateDer::from(cert.der().to_vec());
    let key_der = rustls_pki_types::PrivateKeyDer::Pkcs8(
        rustls_pki_types::PrivatePkcs8KeyDer::from(pkcs8.to_vec()),
    );

    Ok((cert_der, key_der))
}

/// Custom certificate verifier enforcing pinned Ed25519 device identities during mTLS.
#[derive(Debug, Clone)]
pub struct PinnedEd25519Verifier {
    /// Allowed peer Ed25519 public keys. If empty and pairing mode is active, any valid Ed25519 cert is accepted.
    allowed_keys: Vec<[u8; 32]>,
    /// Whether this verifier is operating in unpinned pairing mode.
    pairing_mode: bool,
}

impl PinnedEd25519Verifier {
    /// Creates a verifier with a list of pinned peer public keys.
    #[must_use]
    pub fn new_pinned(allowed_keys: Vec<[u8; 32]>) -> Self {
        Self {
            allowed_keys,
            pairing_mode: false,
        }
    }

    /// Creates a verifier operating in pairing mode (accepts any valid Ed25519 key).
    #[must_use]
    pub fn new_pairing() -> Self {
        Self {
            allowed_keys: Vec::new(),
            pairing_mode: true,
        }
    }

    /// Verifies that the end-entity certificate contains an allowed Ed25519 public key.
    fn verify_peer_cert(
        &self,
        end_entity: &CertificateDer<'_>,
    ) -> std::result::Result<[u8; 32], RustlsError> {
        let pub_key = extract_ed25519_public_key(end_entity.as_ref())
            .ok_or_else(|| RustlsError::General("Certificate missing Ed25519 public key".into()))?;

        if self.pairing_mode {
            return Ok(pub_key);
        }

        if self.allowed_keys.contains(&pub_key) {
            Ok(pub_key)
        } else {
            let mut fp = [0u8; 8];
            fp.copy_from_slice(&pub_key[..8]);
            Err(RustlsError::General(format!(
                "Peer public key {} is not pinned in local DeviceRegistry",
                format_fingerprint(&fp)
            )))
        }
    }
}

impl ServerCertVerifier for PinnedEd25519Verifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> std::result::Result<ServerCertVerified, RustlsError> {
        self.verify_peer_cert(end_entity)?;
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, RustlsError> {
        // TLS 1.2 is explicitly rejected; universal assistant strictly enforces TLS 1.3
        Err(RustlsError::PeerIncompatible(
            rustls::PeerIncompatible::Tls12NotOffered,
        ))
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, RustlsError> {
        if dss.scheme != SignatureScheme::ED25519 {
            return Err(RustlsError::General(
                "Signature scheme is not ED25519".into(),
            ));
        }

        let pub_key_bytes = extract_ed25519_public_key(cert.as_ref())
            .ok_or_else(|| RustlsError::General("Missing Ed25519 public key".into()))?;

        let verifying_key = ed25519_dalek::VerifyingKey::from_bytes(&pub_key_bytes)
            .map_err(|_| RustlsError::General("Invalid Ed25519 public key bytes".into()))?;

        let signature_bytes: [u8; 64] = dss.signature().try_into().map_err(|_| {
            RustlsError::General("Invalid Ed25519 signature length (expected 64 bytes)".into())
        })?;

        let signature = ed25519_dalek::Signature::from_bytes(&signature_bytes);

        use ed25519_dalek::Verifier;
        verifying_key
            .verify(message, &signature)
            .map_err(|_| RustlsError::General("Ed25519 signature verification failed".into()))?;

        Ok(HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        vec![SignatureScheme::ED25519]
    }
}

impl ClientCertVerifier for PinnedEd25519Verifier {
    fn root_hint_subjects(&self) -> &[DistinguishedName] {
        &[]
    }

    fn verify_client_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _now: UnixTime,
    ) -> std::result::Result<ClientCertVerified, RustlsError> {
        self.verify_peer_cert(end_entity)?;
        Ok(ClientCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, RustlsError> {
        // TLS 1.2 is explicitly rejected; universal assistant strictly enforces TLS 1.3
        Err(RustlsError::PeerIncompatible(
            rustls::PeerIncompatible::Tls12NotOffered,
        ))
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, RustlsError> {
        if dss.scheme != SignatureScheme::ED25519 {
            return Err(RustlsError::General(
                "Signature scheme is not ED25519".into(),
            ));
        }

        let pub_key_bytes = extract_ed25519_public_key(cert.as_ref())
            .ok_or_else(|| RustlsError::General("Missing Ed25519 public key".into()))?;

        let verifying_key = ed25519_dalek::VerifyingKey::from_bytes(&pub_key_bytes)
            .map_err(|_| RustlsError::General("Invalid Ed25519 public key bytes".into()))?;

        let signature_bytes: [u8; 64] = dss.signature().try_into().map_err(|_| {
            RustlsError::General("Invalid Ed25519 signature length (expected 64 bytes)".into())
        })?;

        let signature = ed25519_dalek::Signature::from_bytes(&signature_bytes);

        use ed25519_dalek::Verifier;
        verifying_key
            .verify(message, &signature)
            .map_err(|_| RustlsError::General("Ed25519 signature verification failed".into()))?;

        Ok(HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        vec![SignatureScheme::ED25519]
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_and_extract_ed25519_cert() {
        let identity = DeviceIdentity::generate();
        let (cert_der, key_der) = generate_tls_cert(&identity, "aissistant-node").unwrap();

        assert!(!cert_der.is_empty());
        match key_der {
            rustls_pki_types::PrivateKeyDer::Pkcs8(p) => {
                assert_eq!(p.secret_pkcs8_der().len(), 48)
            }
            _ => panic!("Expected PKCS8 private key"),
        }

        let extracted = extract_ed25519_public_key(&cert_der).unwrap();
        assert_eq!(extracted, identity.verifying_key_bytes());
    }

    #[test]
    fn test_verifier_pins_matching_peer_and_rejects_untrusted() {
        let peer_a = DeviceIdentity::generate();
        let peer_b = DeviceIdentity::generate();

        let (cert_a, _) = generate_tls_cert(&peer_a, "node-a").unwrap();
        let (cert_b, _) = generate_tls_cert(&peer_b, "node-b").unwrap();

        // Verifier with peer_a pinned
        let verifier = PinnedEd25519Verifier::new_pinned(vec![peer_a.verifying_key_bytes()]);

        assert!(verifier.verify_peer_cert(&cert_a).is_ok());
        assert!(verifier.verify_peer_cert(&cert_b).is_err());

        // Pairing mode verifier accepts any valid Ed25519 cert
        let pairing_verifier = PinnedEd25519Verifier::new_pairing();
        assert!(pairing_verifier.verify_peer_cert(&cert_a).is_ok());
        assert!(pairing_verifier.verify_peer_cert(&cert_b).is_ok());
    }
}
