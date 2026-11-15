---
name: device-networking-remote
description: >-
  Use this skill when implementing mDNS local discovery, cryptographic device
  pairing, QUIC/TLS 1.3 transport, remote skill execution, clipboard sync,
  resumable file transfer, or low-latency screen streaming.
---

# Device Networking & Remote Control Skill

## Role & Ownership
Owns the `aissistant-network` and `aissistant-remote` Rust crates, peer registry, mDNS discovery, QUIC transport (`quinn` + `rustls`), and cross-device streaming protocols.

## Responsibilities
1. Discover local peers over mDNS-SD (`_aissistant._udp.local.`) without leaking private user identities in plaintext TXT records.
2. Execute mutual cryptographic pairing using Ed25519 long-term device identity keys, ephemeral X25519 key exchange, and a 6-digit Short Authentication String (SAS) / QR code verification ceremony.
3. Multiplex control RPCs, clipboard synchronization, chunked SHA-256 verified file transfers, and adaptive screen/audio streams over QUIC streams and datagrams.
4. Support optional user-configured encrypted relay/tunnel endpoints (e.g., WireGuard/Tailscale/self-hosted QUIC relay) with zero mandatory third-party cloud dependency.

## Boundaries
- Reject all unauthenticated or unpinned peer connections at the TLS handshake layer.
- Enforce per-peer capability ACLs (e.g., Peer A can send files with confirmation, cannot inject remote input).

## Verification & Review Checklist
- [ ] Loopback and multi-node integration tests verify pairing, MITM/wrong-SAS rejection, RPC execution, and interrupted file transfer resumption.
- [ ] Network drop and IP roaming recovery tests pass.
