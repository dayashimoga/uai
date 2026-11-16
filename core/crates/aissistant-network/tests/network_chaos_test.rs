#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Network chaos, packet drop, abrupt disconnect, endpoint rebinding, and reconnection tests.

use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::sync::Arc;

use aissistant_network::{create_virtual_session_pair, NetworkError, QuicTransportEngine};
use aissistant_security::DeviceIdentity;

#[tokio::test]
async fn test_packet_drop_injection_and_recovery() {
    let addr_a = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 54101));
    let addr_b = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 54102));
    let (sess_a, sess_b) = create_virtual_session_pair([10u8; 32], addr_a, [20u8; 32], addr_b);

    // Baseline send/recv
    sess_a.send(b"hello_1").await.expect("Initial send");
    let received = sess_b.recv().await.expect("Initial recv");
    assert_eq!(received, b"hello_1");

    // Inject packet loss on next send
    sess_a.inject_drop_next();
    sess_a
        .send(b"dropped_packet")
        .await
        .expect("Send during drop");

    // The dropped packet is discarded. Send next packet to verify recovery.
    sess_a
        .send(b"recovered_packet")
        .await
        .expect("Send after drop");

    let received_after = sess_b.recv().await.expect("Recv after drop");
    assert_eq!(
        received_after, b"recovered_packet",
        "Stream must continue with next packet"
    );
}

#[tokio::test]
async fn test_abrupt_virtual_session_termination() {
    let addr_a = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 54103));
    let addr_b = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 54104));
    let (sess_a, _sess_b) = create_virtual_session_pair([30u8; 32], addr_a, [40u8; 32], addr_b);

    // Terminate session A abruptly
    sess_a.close();
    assert!(sess_a.is_closed());

    // Subsequent sends must fail closed
    let send_result = sess_a.send(b"data_after_close").await;
    assert!(matches!(
        send_result,
        Err(NetworkError::ConnectionClosed(_))
    ));

    // Receiver must detect closed channel
    let recv_result = sess_a.recv().await;
    assert!(matches!(
        recv_result,
        Err(NetworkError::ConnectionClosed(_))
    ));
}

#[tokio::test]
async fn test_quic_endpoint_rebinding_and_peer_reconnect() {
    let identity_a = Arc::new(DeviceIdentity::generate());
    let identity_b = Arc::new(DeviceIdentity::generate());

    let key_a = identity_a.verifying_key_bytes();
    let key_b = identity_b.verifying_key_bytes();

    let mut engine_a = QuicTransportEngine::new(identity_a);
    let mut engine_b = QuicTransportEngine::new(identity_b);

    let bind_a = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0));
    let bind_b = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0));

    let addr_a = engine_a
        .start_endpoint(bind_a, vec![key_b], false)
        .expect("Start engine A");
    let addr_b = engine_b
        .start_endpoint(bind_b, vec![key_a], false)
        .expect("Start engine B");

    // Spawn engine B acceptor
    let b_accept_handle =
        tokio::spawn(async move { engine_b.accept().await.expect("Engine B accept") });

    // Node A connects to Node B
    let session_ab = engine_a
        .connect_peer(addr_b, Some(key_b), "aissistant-server")
        .await
        .expect("Node A connect to Node B");

    let session_ba = b_accept_handle.await.expect("Accept handle join");

    assert_eq!(session_ab.peer_public_key(), key_b);
    assert_eq!(session_ba.peer_public_key(), key_a);
    assert_eq!(engine_a.active_sessions().len(), 1);

    // Simulate Network Interface Migration (e.g. Wi-Fi -> Hotspot)
    // Engine A rebinds to a new local port
    let new_bind_a = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0));
    let new_addr_a = engine_a
        .rebind_endpoint(new_bind_a, vec![key_b], false)
        .expect("Rebind engine A");

    assert_ne!(addr_a, new_addr_a, "Rebound endpoint must have new port");
    assert_eq!(engine_a.active_sessions().len(), 0, "Old sessions drained");

    // Clean up
    engine_a.close_all();
}
