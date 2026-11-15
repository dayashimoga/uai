//! Cryptographic primitives: Ed25519 identity keys and XChaCha20-Poly1305 AEAD.

use aissistant_core::{CoreError, DeviceId};
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use rand::rngs::OsRng;
use rand::RngCore;
use zeroize::Zeroizing;

/// Nonce length for XChaCha20-Poly1305 in bytes.
pub const XCHACHA20_NONCE_LEN: usize = 24;
/// Poly1305 authentication tag length in bytes.
pub const POLY1305_TAG_LEN: usize = 16;
/// Minimum length of an authenticated payload (nonce + tag).
pub const MIN_AEAD_PAYLOAD_LEN: usize = XCHACHA20_NONCE_LEN + POLY1305_TAG_LEN;

/// Cryptographic device identity based on an Ed25519 keypair.
pub struct DeviceIdentity {
    signing_key: SigningKey,
    verifying_key: VerifyingKey,
}

impl std::fmt::Debug for DeviceIdentity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DeviceIdentity")
            .field("verifying_key", &hex::encode(self.verifying_key_bytes()))
            .finish_non_exhaustive()
    }
}

impl DeviceIdentity {
    /// Generates a new random Ed25519 device identity using OS entropy.
    #[must_use]
    pub fn generate() -> Self {
        let mut csprng = OsRng;
        let signing_key = SigningKey::generate(&mut csprng);
        let verifying_key = signing_key.verifying_key();
        Self {
            signing_key,
            verifying_key,
        }
    }

    /// Reconstructs a device identity from a 32-byte Ed25519 seed/private key.
    pub fn from_bytes(secret_bytes: &[u8; 32]) -> Self {
        let signing_key = SigningKey::from_bytes(secret_bytes);
        let verifying_key = signing_key.verifying_key();
        Self {
            signing_key,
            verifying_key,
        }
    }

    /// Returns the raw 32-byte secret key material.
    #[must_use]
    pub fn secret_bytes(&self) -> Zeroizing<[u8; 32]> {
        Zeroizing::new(self.signing_key.to_bytes())
    }

    /// Returns the 32-byte public verifying key.
    #[must_use]
    pub fn verifying_key_bytes(&self) -> [u8; 32] {
        self.verifying_key.to_bytes()
    }

    /// Returns the public `VerifyingKey`.
    #[must_use]
    pub const fn verifying_key(&self) -> &VerifyingKey {
        &self.verifying_key
    }

    /// Derives a deterministic `DeviceId` slug from this identity's public key.
    pub fn to_device_id(&self) -> Result<DeviceId, CoreError> {
        let hex_prefix: String = self
            .verifying_key_bytes()
            .iter()
            .take(16)
            .map(|b| format!("{b:02x}"))
            .collect();
        DeviceId::new(format!("dev_{hex_prefix}"))
    }

    /// Signs an arbitrary message using this device identity's private key.
    #[must_use]
    pub fn sign(&self, message: &[u8]) -> [u8; 64] {
        self.signing_key.sign(message).to_bytes()
    }

    /// Verifies an Ed25519 signature against an expected public key and message.
    pub fn verify(
        verifying_key_bytes: &[u8; 32],
        message: &[u8],
        signature_bytes: &[u8; 64],
    ) -> Result<(), CoreError> {
        let verifying_key =
            VerifyingKey::from_bytes(verifying_key_bytes).map_err(|e| CoreError::InvalidId {
                entity: "VerifyingKey",
                value: "corrupted_ed25519_key".to_string(),
                reason: e.to_string(),
            })?;

        let signature = Signature::from_bytes(signature_bytes);
        verifying_key
            .verify(message, &signature)
            .map_err(|e| CoreError::PermissionDenied {
                action: "crypto.verify".to_string(),
                reason: format!("Ed25519 cryptographic signature verification failed: {e}"),
            })
    }
}

/// Symmetrically authenticated 256-bit key for local database and sensitive field encryption.
pub struct AeadKey(Zeroizing<[u8; 32]>);

impl std::fmt::Debug for AeadKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("AeadKey").field(&"[REDACTED]").finish()
    }
}

impl AeadKey {
    /// Generates a new cryptographically random 256-bit AEAD key using OS entropy.
    #[must_use]
    pub fn generate() -> Self {
        let mut key = [0u8; 32];
        OsRng.fill_bytes(&mut key);
        Self(Zeroizing::new(key))
    }

    /// Instantiates an AEAD key from a 32-byte secret slice.
    #[must_use]
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(Zeroizing::new(bytes))
    }

    /// Encrypts plaintext with associated authenticated data using XChaCha20-Poly1305.
    /// Returns 24-byte random nonce prepended to the ciphertext and tag.
    pub fn encrypt(&self, plaintext: &[u8], associated_data: &[u8]) -> Result<Vec<u8>, CoreError> {
        let cipher = XChaCha20Poly1305::new_from_slice(self.0.as_slice()).map_err(|e| {
            CoreError::Internal {
                message: format!("Failed to initialize XChaCha20-Poly1305: {e}"),
            }
        })?;

        let mut nonce_bytes = [0u8; XCHACHA20_NONCE_LEN];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = XNonce::from_slice(&nonce_bytes);

        let payload = Payload {
            msg: plaintext,
            aad: associated_data,
        };

        let ciphertext = cipher
            .encrypt(nonce, payload)
            .map_err(|e| CoreError::Internal {
                message: format!("AEAD encryption failed: {e}"),
            })?;

        let mut output = Vec::with_capacity(XCHACHA20_NONCE_LEN + ciphertext.len());
        output.extend_from_slice(&nonce_bytes);
        output.extend_from_slice(&ciphertext);
        Ok(output)
    }

    /// Decrypts a payload produced by `encrypt` using XChaCha20-Poly1305.
    pub fn decrypt(&self, payload: &[u8], associated_data: &[u8]) -> Result<Vec<u8>, CoreError> {
        if payload.len() < MIN_AEAD_PAYLOAD_LEN {
            return Err(CoreError::InvalidId {
                entity: "AeadPayload",
                value: format!("{} bytes", payload.len()),
                reason: format!("Payload too short to contain nonce and tag (minimum {MIN_AEAD_PAYLOAD_LEN} bytes)"),
            });
        }

        let (nonce_bytes, ciphertext) = payload.split_at(XCHACHA20_NONCE_LEN);
        let nonce = XNonce::from_slice(nonce_bytes);

        let cipher = XChaCha20Poly1305::new_from_slice(self.0.as_slice()).map_err(|e| {
            CoreError::Internal {
                message: format!("Failed to initialize XChaCha20-Poly1305: {e}"),
            }
        })?;

        let payload_struct = Payload {
            msg: ciphertext,
            aad: associated_data,
        };

        cipher
            .decrypt(nonce, payload_struct)
            .map_err(|e| CoreError::PermissionDenied {
                action: "crypto.aead_decrypt".to_string(),
                reason: format!("AEAD decryption or integrity authentication failed: {e}"),
            })
    }
}

mod hex {
    pub(super) fn encode(data: impl AsRef<[u8]>) -> String {
        data.as_ref().iter().map(|b| format!("{b:02x}")).collect()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_device_identity_generation_and_signing() {
        let id = DeviceIdentity::generate();
        let message = b"Authorize remote action: file.read on desktop";

        let signature = id.sign(message);
        assert_eq!(signature.len(), 64);

        // Verification must succeed
        assert!(DeviceIdentity::verify(&id.verifying_key_bytes(), message, &signature).is_ok());

        // Corrupted message must fail verification
        let corrupted_msg = b"Authorize remote action: file.delete on desktop";
        assert!(
            DeviceIdentity::verify(&id.verifying_key_bytes(), corrupted_msg, &signature).is_err()
        );

        // Corrupted key must fail verification
        let mut bad_key = id.verifying_key_bytes();
        bad_key[0] ^= 0xFF;
        assert!(DeviceIdentity::verify(&bad_key, message, &signature).is_err());
    }

    #[test]
    fn test_device_identity_from_seed() {
        let seed = [42u8; 32];
        let id1 = DeviceIdentity::from_bytes(&seed);
        let id2 = DeviceIdentity::from_bytes(&seed);

        assert_eq!(id1.verifying_key_bytes(), id2.verifying_key_bytes());
        assert_eq!(id1.to_device_id().unwrap(), id2.to_device_id().unwrap());
    }

    #[test]
    fn test_aead_encryption_and_decryption_roundtrip() {
        let key = AeadKey::generate();
        let plaintext = b"Sensitive memory: user preference for offline assistant";
        let aad = b"context: turn_000001";

        let ciphertext = key.encrypt(plaintext, aad).unwrap();
        assert_ne!(ciphertext.as_slice(), plaintext);
        assert!(ciphertext.len() >= plaintext.len() + MIN_AEAD_PAYLOAD_LEN);

        let decrypted = key.decrypt(&ciphertext, aad).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_aead_tampered_ciphertext_rejected() {
        let key = AeadKey::generate();
        let plaintext = b"Secret credentials";
        let aad = b"session_id_42";

        let mut ciphertext = key.encrypt(plaintext, aad).unwrap();

        // Flip a byte in the encrypted body
        let last_idx = ciphertext.len() - 1;
        ciphertext[last_idx] ^= 0x01;

        assert!(key.decrypt(&ciphertext, aad).is_err());
    }

    #[test]
    fn test_aead_tampered_aad_rejected() {
        let key = AeadKey::generate();
        let plaintext = b"Secret credentials";
        let aad1 = b"session_id_42";
        let aad2 = b"session_id_43";

        let ciphertext = key.encrypt(plaintext, aad1).unwrap();
        assert!(key.decrypt(&ciphertext, aad2).is_err());
    }

    #[test]
    fn test_aead_payload_too_short() {
        let key = AeadKey::generate();
        assert!(key.decrypt(b"too_short", b"").is_err());
    }
}
