//! QUIC/TLS 1.3 mutual-authenticated peer-to-peer transport session and endpoints.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use aissistant_security::DeviceIdentity;
use quinn::crypto::rustls::{QuicClientConfig, QuicServerConfig};
use quinn::{ClientConfig, Endpoint, RecvStream, SendStream, ServerConfig, TransportConfig};
use rustls::pki_types::CertificateDer;

use crate::cert::{extract_ed25519_public_key, generate_tls_cert, PinnedEd25519Verifier};
use crate::error::{NetworkError, Result};

/// Standard ALPN protocol identifier for Universal AI Assistant P2P sessions.
pub const ALPN_P2P_PROTOCOL: &[u8] = b"aissistant-p2p/1";

/// Default connection keepalive heartbeat interval (5 seconds).
pub const DEFAULT_KEEPALIVE_INTERVAL: Duration = Duration::from_secs(5);

/// Default connection max idle timeout (30 seconds).
pub const DEFAULT_IDLE_TIMEOUT: Duration = Duration::from_secs(30);

/// Extracts the authenticated 32-byte Ed25519 public key of the remote peer from a Quinn connection.
#[must_use]
pub fn extract_peer_public_key(connection: &quinn::Connection) -> Option<[u8; 32]> {
    let identity = connection.peer_identity()?;
    let certs = identity.downcast::<Vec<CertificateDer<'static>>>().ok()?;
    let first_cert = certs.first()?;
    extract_ed25519_public_key(first_cert.as_ref())
}

/// Builds a `quinn::ServerConfig` enforcing mutual TLS 1.3 with pinned Ed25519 verification.
pub fn build_quic_server_config(
    identity: &DeviceIdentity,
    trusted_peer_keys: Vec<[u8; 32]>,
    pairing_mode: bool,
) -> Result<ServerConfig> {
    let (cert_der, key_der) = generate_tls_cert(identity, "aissistant-server")?;

    let client_verifier = if pairing_mode {
        Arc::new(PinnedEd25519Verifier::new_pairing())
    } else {
        Arc::new(PinnedEd25519Verifier::new_pinned(trusted_peer_keys))
    };

    let mut rustls_server = rustls::ServerConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .map_err(|e| NetworkError::Internal(format!("Failed to configure TLS protocol versions: {e}")))?
    .with_client_cert_verifier(client_verifier)
    .with_single_cert(vec![cert_der], key_der)
    .map_err(NetworkError::Tls)?;

    rustls_server.alpn_protocols = vec![ALPN_P2P_PROTOCOL.to_vec()];

    let mut transport_config = TransportConfig::default();
    if let Ok(timeout) = DEFAULT_IDLE_TIMEOUT.try_into() {
        transport_config.max_idle_timeout(Some(timeout));
    }
    transport_config.keep_alive_interval(Some(DEFAULT_KEEPALIVE_INTERVAL));

    let quic_crypto = QuicServerConfig::try_from(rustls_server)
        .map_err(|e| NetworkError::Internal(format!("Failed to build QUIC server crypto: {e}")))?;

    let mut server_config = ServerConfig::with_crypto(Arc::new(quic_crypto));
    server_config.transport_config(Arc::new(transport_config));

    Ok(server_config)
}

/// Builds a `quinn::ClientConfig` enforcing mutual TLS 1.3 with pinned Ed25519 verification.
pub fn build_quic_client_config(
    identity: &DeviceIdentity,
    trusted_peer_keys: Vec<[u8; 32]>,
    pairing_mode: bool,
) -> Result<ClientConfig> {
    let (cert_der, key_der) = generate_tls_cert(identity, "aissistant-client")?;

    let server_verifier = if pairing_mode {
        Arc::new(PinnedEd25519Verifier::new_pairing())
    } else {
        Arc::new(PinnedEd25519Verifier::new_pinned(trusted_peer_keys))
    };

    let mut rustls_client = rustls::ClientConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .map_err(|e| NetworkError::Internal(format!("Failed to configure TLS protocol versions: {e}")))?
    .dangerous()
    .with_custom_certificate_verifier(server_verifier)
    .with_client_auth_cert(vec![cert_der], key_der)
    .map_err(NetworkError::Tls)?;

    rustls_client.alpn_protocols = vec![ALPN_P2P_PROTOCOL.to_vec()];

    let mut transport_config = TransportConfig::default();
    if let Ok(timeout) = DEFAULT_IDLE_TIMEOUT.try_into() {
        transport_config.max_idle_timeout(Some(timeout));
    }
    transport_config.keep_alive_interval(Some(DEFAULT_KEEPALIVE_INTERVAL));

    let quic_crypto = QuicClientConfig::try_from(rustls_client)
        .map_err(|e| NetworkError::Internal(format!("Failed to build QUIC client crypto: {e}")))?;

    let mut client_config = ClientConfig::new(Arc::new(quic_crypto));
    client_config.transport_config(Arc::new(transport_config));

    Ok(client_config)
}

/// Established mutual TLS 1.3 QUIC connection wrapping bi-directional streams.
#[derive(Debug, Clone)]
pub struct QuicSession {
    connection: quinn::Connection,
    peer_public_key: [u8; 32],
    remote_address: SocketAddr,
}

impl QuicSession {
    /// Wraps an established Quinn connection.
    pub fn new(connection: quinn::Connection) -> Result<Self> {
        let peer_public_key = extract_peer_public_key(&connection).ok_or_else(|| {
            NetworkError::PeerAuthenticationFailed("Missing peer Ed25519 public key".to_string())
        })?;
        let remote_address = connection.remote_address();

        Ok(Self {
            connection,
            peer_public_key,
            remote_address,
        })
    }

    /// Returns the verified 32-byte Ed25519 public key of the remote peer.
    #[must_use]
    pub const fn peer_public_key(&self) -> [u8; 32] {
        self.peer_public_key
    }

    /// Returns the remote socket endpoint of the peer.
    #[must_use]
    pub const fn remote_address(&self) -> SocketAddr {
        self.remote_address
    }

    /// Opens a new bidirectional QUIC stream.
    pub async fn open_bi(&self) -> Result<(SendStream, RecvStream)> {
        self.connection
            .open_bi()
            .await
            .map_err(|e| NetworkError::Quic(format!("Failed to open bi-stream: {e}")))
    }

    /// Accepts an incoming bidirectional QUIC stream initiated by the peer.
    pub async fn accept_bi(&self) -> Result<(SendStream, RecvStream)> {
        self.connection
            .accept_bi()
            .await
            .map_err(|e| NetworkError::Quic(format!("Failed to accept bi-stream: {e}")))
    }

    /// Sends an unreliable or out-of-band datagram payload across the session.
    pub fn send_datagram(&self, data: &[u8]) -> Result<()> {
        self.connection
            .send_datagram(data.to_vec().into())
            .map_err(|e| NetworkError::Quic(format!("Failed to send datagram: {e}")))
    }

    /// Reads an incoming datagram payload.
    pub async fn read_datagram(&self) -> Result<Vec<u8>> {
        let bytes = self
            .connection
            .read_datagram()
            .await
            .map_err(|e| NetworkError::Quic(format!("Failed to read datagram: {e}")))?;
        Ok(bytes.to_vec())
    }

    /// Gracefully closes the connection with code 0.
    pub fn close(&self) {
        self.connection.close(0u32.into(), b"graceful close");
    }

    /// Checks if the connection has been closed or terminated.
    #[must_use]
    pub fn is_closed(&self) -> bool {
        self.connection.close_reason().is_some()
    }
}

/// QUIC transport engine managing endpoints and active mTLS peer sessions.
#[derive(Debug)]
pub struct QuicTransportEngine {
    identity: Arc<DeviceIdentity>,
    endpoint: Option<Endpoint>,
    sessions: Arc<RwLock<HashMap<[u8; 32], QuicSession>>>,
}

impl QuicTransportEngine {
    /// Creates a new QUIC transport engine with the local device identity.
    #[must_use]
    pub fn new(identity: Arc<DeviceIdentity>) -> Self {
        Self {
            identity,
            endpoint: None,
            sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Starts a listening QUIC endpoint bound to `bind_addr` accepting mutual TLS 1.3 connections.
    pub fn start_endpoint(
        &mut self,
        bind_addr: SocketAddr,
        trusted_keys: Vec<[u8; 32]>,
        pairing_mode: bool,
    ) -> Result<SocketAddr> {
        let server_config =
            build_quic_server_config(&self.identity, trusted_keys.clone(), pairing_mode)?;
        let client_config = build_quic_client_config(&self.identity, trusted_keys, pairing_mode)?;

        let mut endpoint = Endpoint::server(server_config, bind_addr).map_err(|e| {
            NetworkError::SocketBindFailed {
                endpoint: bind_addr.to_string(),
                reason: e.to_string(),
            }
        })?;

        endpoint.set_default_client_config(client_config);
        let local_addr = endpoint
            .local_addr()
            .map_err(|e| NetworkError::Internal(format!("Failed to get local addr: {e}")))?;

        self.endpoint = Some(endpoint);
        Ok(local_addr)
    }

    /// Connects to a remote peer over mutual TLS 1.3, optionally enforcing expected public key.
    pub async fn connect_peer(
        &self,
        target_addr: SocketAddr,
        expected_key: Option<[u8; 32]>,
        server_name: &str,
    ) -> Result<QuicSession> {
        let endpoint = self.endpoint.as_ref().ok_or_else(|| {
            NetworkError::Internal("Transport endpoint not initialized".to_string())
        })?;

        let connecting = endpoint
            .connect(target_addr, server_name)
            .map_err(|e| NetworkError::Quic(format!("Failed to initiate connection: {e}")))?;

        let connection = connecting
            .await
            .map_err(|e| NetworkError::Quic(format!("QUIC handshake failed: {e}")))?;

        let session = QuicSession::new(connection)?;

        if let Some(expected) = expected_key {
            if session.peer_public_key() != expected {
                session.close();
                return Err(NetworkError::PeerAuthenticationFailed(
                    "Peer key does not match expected key".to_string(),
                ));
            }
        }

        if let Ok(mut map) = self.sessions.write() {
            map.insert(session.peer_public_key(), session.clone());
        }

        Ok(session)
    }

    /// Accepts an incoming connection from the endpoint.
    pub async fn accept(&self) -> Result<QuicSession> {
        let endpoint = self.endpoint.as_ref().ok_or_else(|| {
            NetworkError::Internal("Transport endpoint not initialized".to_string())
        })?;

        let incoming = endpoint
            .accept()
            .await
            .ok_or_else(|| NetworkError::ConnectionClosed("Endpoint shut down".to_string()))?;

        let connection = incoming
            .await
            .map_err(|e| NetworkError::Quic(format!("Handshake failed on accept: {e}")))?;

        let session = QuicSession::new(connection)?;

        if let Ok(mut map) = self.sessions.write() {
            map.insert(session.peer_public_key(), session.clone());
        }

        Ok(session)
    }

    /// Returns a list of active peer sessions.
    pub fn active_sessions(&self) -> Vec<QuicSession> {
        self.sessions
            .read()
            .map(|m| m.values().cloned().collect())
            .unwrap_or_default()
    }

    /// Closes all active sessions.
    pub fn close_all(&self) {
        if let Ok(mut map) = self.sessions.write() {
            for session in map.values() {
                session.close();
            }
            map.clear();
        }
    }

    /// Disconnects and closes an active peer session by public key.
    pub fn disconnect_peer(&self, peer_key: &[u8; 32]) -> bool {
        if let Ok(mut map) = self.sessions.write() {
            if let Some(session) = map.remove(peer_key) {
                session.close();
                return true;
            }
        }
        false
    }

    /// Reconnects to a remote peer, discarding any existing stale session.
    pub async fn reconnect_peer(
        &self,
        target_addr: SocketAddr,
        expected_key: [u8; 32],
        server_name: &str,
    ) -> Result<QuicSession> {
        self.disconnect_peer(&expected_key);
        self.connect_peer(target_addr, Some(expected_key), server_name)
            .await
    }

    /// Rebinds the transport endpoint to a new socket address (e.g. following Wi-Fi-to-hotspot transition).
    pub fn rebind_endpoint(
        &mut self,
        new_bind_addr: SocketAddr,
        trusted_keys: Vec<[u8; 32]>,
        pairing_mode: bool,
    ) -> Result<SocketAddr> {
        self.close_all();
        if let Some(endpoint) = self.endpoint.take() {
            endpoint.close(0u32.into(), b"rebinding");
        }
        self.start_endpoint(new_bind_addr, trusted_keys, pairing_mode)
    }
}

/// In-memory virtual transport session for deterministic unit and CI testing.
#[derive(Debug, Clone)]
pub struct VirtualSession {
    local_key: [u8; 32],
    peer_key: [u8; 32],
    remote_addr: SocketAddr,
    tx: tokio::sync::mpsc::Sender<Vec<u8>>,
    rx: Arc<tokio::sync::Mutex<tokio::sync::mpsc::Receiver<Vec<u8>>>>,
    closed: Arc<std::sync::atomic::AtomicBool>,
    drop_next: Arc<std::sync::atomic::AtomicBool>,
}

impl VirtualSession {
    /// Returns the local public key.
    #[must_use]
    pub const fn local_public_key(&self) -> [u8; 32] {
        self.local_key
    }

    /// Returns the peer's public key.
    #[must_use]
    pub const fn peer_public_key(&self) -> [u8; 32] {
        self.peer_key
    }

    /// Returns the remote socket endpoint.
    #[must_use]
    pub const fn remote_address(&self) -> SocketAddr {
        self.remote_addr
    }

    /// Injects a single packet drop on the next outbound transmission.
    pub fn inject_drop_next(&self) {
        self.drop_next
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }

    /// Sends a data frame over the virtual session channel.
    pub async fn send(&self, data: &[u8]) -> Result<()> {
        if self.is_closed() {
            return Err(NetworkError::ConnectionClosed(
                "Virtual session closed".into(),
            ));
        }
        if self
            .drop_next
            .swap(false, std::sync::atomic::Ordering::Relaxed)
        {
            // Simulated packet loss: payload silently discarded
            return Ok(());
        }
        self.tx
            .send(data.to_vec())
            .await
            .map_err(|_| NetworkError::ConnectionClosed("Virtual session receiver dropped".into()))
    }

    /// Receives a data frame from the virtual session channel.
    pub async fn recv(&self) -> Result<Vec<u8>> {
        if self.is_closed() {
            return Err(NetworkError::ConnectionClosed(
                "Virtual session closed".into(),
            ));
        }
        let mut rx = self.rx.lock().await;
        rx.recv()
            .await
            .ok_or_else(|| NetworkError::ConnectionClosed("Virtual channel sender dropped".into()))
    }

    /// Checks if the virtual session is closed.
    #[must_use]
    pub fn is_closed(&self) -> bool {
        self.closed.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Closes the virtual session.
    pub fn close(&self) {
        self.closed
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

/// Creates a connected pair of virtual sessions simulating an established mTLS QUIC connection.
#[must_use]
pub fn create_virtual_session_pair(
    key_a: [u8; 32],
    addr_a: SocketAddr,
    key_b: [u8; 32],
    addr_b: SocketAddr,
) -> (VirtualSession, VirtualSession) {
    let (tx_a_to_b, rx_b) = tokio::sync::mpsc::channel(64);
    let (tx_b_to_a, rx_a) = tokio::sync::mpsc::channel(64);
    let closed_a = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let closed_b = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let drop_a = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let drop_b = Arc::new(std::sync::atomic::AtomicBool::new(false));

    let session_a = VirtualSession {
        local_key: key_a,
        peer_key: key_b,
        remote_addr: addr_b,
        tx: tx_a_to_b,
        rx: Arc::new(tokio::sync::Mutex::new(rx_a)),
        closed: closed_a,
        drop_next: drop_a,
    };

    let session_b = VirtualSession {
        local_key: key_b,
        peer_key: key_a,
        remote_addr: addr_a,
        tx: tx_b_to_a,
        rx: Arc::new(tokio::sync::Mutex::new(rx_b)),
        closed: closed_b,
        drop_next: drop_b,
    };

    (session_a, session_b)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, SocketAddrV4};

    #[tokio::test]
    async fn test_virtual_session_bidirectional_data_exchange() {
        let addr_a = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 51820));
        let addr_b = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 51821));
        let (sess_a, sess_b) = create_virtual_session_pair([1u8; 32], addr_a, [2u8; 32], addr_b);

        assert_eq!(sess_a.peer_public_key(), [2u8; 32]);
        assert_eq!(sess_b.peer_public_key(), [1u8; 32]);
        assert!(!sess_a.is_closed());
        assert!(!sess_b.is_closed());

        // Send A -> B
        sess_a.send(b"hello from node a").await.unwrap();
        let received_b = sess_b.recv().await.unwrap();
        assert_eq!(received_b, b"hello from node a");

        // Send B -> A
        sess_b.send(b"acknowledgment from node b").await.unwrap();
        let received_a = sess_a.recv().await.unwrap();
        assert_eq!(received_a, b"acknowledgment from node b");

        // Close A
        sess_a.close();
        assert!(sess_a.is_closed());
        assert!(sess_a.send(b"blocked").await.is_err());
    }

    #[tokio::test]
    async fn test_quic_configs_build_cleanly() {
        let identity = DeviceIdentity::generate();
        let trusted = vec![[0x55; 32]];

        let server_cfg = build_quic_server_config(&identity, trusted.clone(), false).unwrap();
        let client_cfg = build_quic_client_config(&identity, trusted, false).unwrap();

        let _ = server_cfg;
        let _ = client_cfg;
    }

    #[tokio::test]
    async fn test_quic_loopback_mtls_connection() {
        let id_server = Arc::new(DeviceIdentity::generate());
        let id_client = Arc::new(DeviceIdentity::generate());

        let mut engine_server = QuicTransportEngine::new(id_server.clone());
        let mut engine_client = QuicTransportEngine::new(id_client.clone());

        let trusted_for_server = vec![id_client.verifying_key_bytes()];
        let trusted_for_client = vec![id_server.verifying_key_bytes()];

        let bind_addr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0));
        let server_addr = engine_server
            .start_endpoint(bind_addr, trusted_for_server, false)
            .unwrap();

        let _client_addr = engine_client
            .start_endpoint(bind_addr, trusted_for_client, false)
            .unwrap();

        // Spawn server accept task
        let accept_handle = tokio::spawn(async move {
            let session = engine_server.accept().await.unwrap();
            let (mut send, mut recv) = session.accept_bi().await.unwrap();
            let mut buf = [0u8; 16];
            let n = recv.read(&mut buf).await.unwrap().unwrap();
            assert_eq!(&buf[..n], b"ping");
            send.write_all(b"pong").await.unwrap();
            send.finish().unwrap();
            session
        });

        // Client connects to server
        let client_session = engine_client
            .connect_peer(
                server_addr,
                Some(id_server.verifying_key_bytes()),
                "aissistant-server",
            )
            .await
            .unwrap();

        assert_eq!(
            client_session.peer_public_key(),
            id_server.verifying_key_bytes()
        );

        let (mut send, mut recv) = client_session.open_bi().await.unwrap();
        send.write_all(b"ping").await.unwrap();
        send.finish().unwrap();

        let mut reply = [0u8; 16];
        let n = recv.read(&mut reply).await.unwrap().unwrap();
        assert_eq!(&reply[..n], b"pong");

        let server_session = accept_handle.await.unwrap();
        assert_eq!(
            server_session.peer_public_key(),
            id_client.verifying_key_bytes()
        );

        client_session.close();
        server_session.close();
    }

    #[tokio::test]
    async fn test_quic_rejects_untrusted_peer() {
        let id_server = Arc::new(DeviceIdentity::generate());
        let id_attacker = Arc::new(DeviceIdentity::generate());

        let mut engine_server = QuicTransportEngine::new(id_server.clone());
        let mut engine_attacker = QuicTransportEngine::new(id_attacker.clone());

        // Server only trusts some random key, NOT attacker!
        let trusted_for_server = vec![[0x99; 32]];
        let trusted_for_attacker = vec![id_server.verifying_key_bytes()];

        let bind_addr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0));
        let server_addr = engine_server
            .start_endpoint(bind_addr, trusted_for_server, false)
            .unwrap();

        let _attacker_addr = engine_attacker
            .start_endpoint(bind_addr, trusted_for_attacker, false)
            .unwrap();

        // Server accept task
        let accept_handle = tokio::spawn(async move { engine_server.accept().await });

        // Attacker attempts to connect -> server must reject untrusted client
        let _connect_res = engine_attacker
            .connect_peer(
                server_addr,
                Some(id_server.verifying_key_bytes()),
                "aissistant-server",
            )
            .await;

        let accept_res = accept_handle.await.unwrap();
        assert!(accept_res.is_err());
    }

    #[tokio::test]
    async fn test_quic_client_rejects_untrusted_server() {
        let id_server = Arc::new(DeviceIdentity::generate());
        let id_client = Arc::new(DeviceIdentity::generate());

        let mut engine_server = QuicTransportEngine::new(id_server.clone());
        let mut engine_client = QuicTransportEngine::new(id_client.clone());

        let trusted_for_server = vec![id_client.verifying_key_bytes()];
        // Client only trusts a rogue key, NOT the server!
        let trusted_for_client = vec![[0x88; 32]];

        let bind_addr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0));
        let server_addr = engine_server
            .start_endpoint(bind_addr, trusted_for_server, false)
            .unwrap();

        let _client_addr = engine_client
            .start_endpoint(bind_addr, trusted_for_client, false)
            .unwrap();

        // Server accept task to advance the handshake
        tokio::spawn(async move {
            let _ = engine_server.accept().await;
        });

        // Client attempts to connect to untrusted server -> must fail handshake!
        let connect_res = engine_client
            .connect_peer(
                server_addr,
                Some(id_server.verifying_key_bytes()),
                "aissistant-server",
            )
            .await;

        assert!(connect_res.is_err());
    }
}
