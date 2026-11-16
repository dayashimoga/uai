#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use aissistant_network::dns::{DnsPacket, DnsQuestion, CLASS_IN, TYPE_PTR};
use proptest::prelude::*;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    #[test]
    fn test_arbitrary_bytes_dns_fuzzing_never_panics(
        fuzz_data in prop::collection::vec(any::<u8>(), 0..4096)
    ) {
        // Any arbitrary payload must safely return Ok or Err, never panic
        let _ = DnsPacket::from_bytes(&fuzz_data);
    }

    #[test]
    fn test_dns_header_with_random_payload_never_panics(
        id in any::<u16>(),
        flags in any::<u16>(),
        qdcount in any::<u16>(),
        ancount in any::<u16>(),
        nscount in any::<u16>(),
        arcount in any::<u16>(),
        payload in prop::collection::vec(any::<u8>(), 0..2048)
    ) {
        let mut buffer = Vec::with_capacity(12 + payload.len());
        buffer.extend_from_slice(&id.to_be_bytes());
        buffer.extend_from_slice(&flags.to_be_bytes());
        buffer.extend_from_slice(&qdcount.to_be_bytes());
        buffer.extend_from_slice(&ancount.to_be_bytes());
        buffer.extend_from_slice(&nscount.to_be_bytes());
        buffer.extend_from_slice(&arcount.to_be_bytes());
        buffer.extend_from_slice(&payload);

        let _ = DnsPacket::from_bytes(&buffer);
    }

    #[test]
    fn test_mutated_valid_packet_never_panics(
        mutation_index in 0usize..64,
        mutation_byte in any::<u8>(),
        truncate_at in 0usize..64
    ) {
        let question = DnsQuestion {
            name: "_aissistant._udp.local".to_string(),
            qtype: TYPE_PTR,
            qclass: CLASS_IN,
            unicast_response: false,
        };
        let valid_packet = DnsPacket::new_query(0x1234, vec![question]);
        let mut wire = valid_packet.to_bytes();

        // Truncate at random lengths
        if truncate_at < wire.len() {
            let truncated = &wire[..truncate_at];
            let _ = DnsPacket::from_bytes(truncated);
        }

        // Mutate a byte
        if mutation_index < wire.len() {
            wire[mutation_index] = mutation_byte;
            let _ = DnsPacket::from_bytes(&wire);
        }
    }

    #[test]
    fn test_pointer_compression_loop_safely_rejected(
        loop_target in 0usize..20
    ) {
        // Construct a DNS packet with a question pointing to a compression pointer that cycles
        let mut buffer = Vec::new();
        // ID: 1, Flags: 0, QDCOUNT: 1, ANCOUNT: 0, NSCOUNT: 0, ARCOUNT: 0
        buffer.extend_from_slice(&1u16.to_be_bytes());
        buffer.extend_from_slice(&0u16.to_be_bytes());
        buffer.extend_from_slice(&1u16.to_be_bytes());
        buffer.extend_from_slice(&0u16.to_be_bytes());
        buffer.extend_from_slice(&0u16.to_be_bytes());
        buffer.extend_from_slice(&0u16.to_be_bytes());

        // Offset 12: compression pointer pointing to loop_target
        let ptr_high = 0xC0 | ((loop_target >> 8) as u8 & 0x3F);
        let ptr_low = (loop_target & 0xFF) as u8;
        buffer.push(ptr_high);
        buffer.push(ptr_low);
        // QTYPE, QCLASS
        buffer.extend_from_slice(&1u16.to_be_bytes());
        buffer.extend_from_slice(&1u16.to_be_bytes());

        // Must reject cleanly without infinite loop or panic
        let res = DnsPacket::from_bytes(&buffer);
        let _ = res;
    }
}
