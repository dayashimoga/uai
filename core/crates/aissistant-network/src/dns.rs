//! DNS wire-format parser and builder conforming to RFC 1035, RFC 6762 (mDNS), and RFC 6763 (DNS-SD).

use std::net::{Ipv4Addr, Ipv6Addr};

use crate::error::{NetworkError, Result};

/// DNS Record Types
pub const TYPE_A: u16 = 1;
pub const TYPE_PTR: u16 = 12;
pub const TYPE_TXT: u16 = 16;
pub const TYPE_AAAA: u16 = 28;
pub const TYPE_SRV: u16 = 33;
pub const TYPE_ANY: u16 = 255;

/// DNS Class IN (Internet)
pub const CLASS_IN: u16 = 1;
/// mDNS Cache-Flush Flag (Bit 15 of RR Class)
pub const CACHE_FLUSH_BIT: u16 = 0x8000;

/// Parsed or constructed DNS question.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DnsQuestion {
    /// Domain name queried.
    pub name: String,
    /// Record type (e.g. PTR, SRV, TXT, A, AAAA).
    pub qtype: u16,
    /// Record class (typically CLASS_IN).
    pub qclass: u16,
    /// Whether a unicast response is requested (QU flag).
    pub unicast_response: bool,
}

/// Parsed DNS Resource Record data payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RData {
    /// IPv4 host address.
    A(Ipv4Addr),
    /// IPv6 host address.
    Aaaa(Ipv6Addr),
    /// Domain name pointer.
    Ptr(String),
    /// Service instance location and port.
    Srv {
        /// Priority order.
        priority: u16,
        /// Weight selection.
        weight: u16,
        /// TCP/UDP listening port.
        port: u16,
        /// Target host domain name.
        target: String,
    },
    /// Text key-value metadata pairs.
    Txt(Vec<String>),
    /// Raw unparsed payload for unrecognized types.
    Raw(Vec<u8>),
}

/// Parsed or constructed DNS Resource Record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DnsRecord {
    /// Domain name of the record.
    pub name: String,
    /// Record type.
    pub rtype: u16,
    /// Record class.
    pub rclass: u16,
    /// Cache flush bit flag.
    pub cache_flush: bool,
    /// Time-to-live in seconds.
    pub ttl: u32,
    /// Record data payload.
    pub rdata: RData,
}

/// Complete DNS packet structure.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DnsPacket {
    /// Query transaction ID.
    pub id: u16,
    /// Packet flags (QR, AA, etc.).
    pub flags: u16,
    /// Questions section.
    pub questions: Vec<DnsQuestion>,
    /// Answer resource records.
    pub answers: Vec<DnsRecord>,
    /// Authority resource records.
    pub authorities: Vec<DnsRecord>,
    /// Additional resource records.
    pub additionals: Vec<DnsRecord>,
}

impl DnsPacket {
    /// Creates a new DNS query packet.
    #[must_use]
    pub fn new_query(id: u16, questions: Vec<DnsQuestion>) -> Self {
        Self {
            id,
            flags: 0x0000, // Standard Query
            questions,
            answers: Vec::new(),
            authorities: Vec::new(),
            additionals: Vec::new(),
        }
    }

    /// Creates a new mDNS authoritative response packet.
    #[must_use]
    pub fn new_response(id: u16, answers: Vec<DnsRecord>, additionals: Vec<DnsRecord>) -> Self {
        Self {
            id,
            flags: 0x8400, // QR = 1 (Response), AA = 1 (Authoritative)
            questions: Vec::new(),
            answers,
            authorities: Vec::new(),
            additionals,
        }
    }

    /// Serializes the DNS packet into RFC-compliant binary wire format.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(512);

        // Header
        buf.extend_from_slice(&self.id.to_be_bytes());
        buf.extend_from_slice(&self.flags.to_be_bytes());
        buf.extend_from_slice(&(self.questions.len() as u16).to_be_bytes());
        buf.extend_from_slice(&(self.answers.len() as u16).to_be_bytes());
        buf.extend_from_slice(&(self.authorities.len() as u16).to_be_bytes());
        buf.extend_from_slice(&(self.additionals.len() as u16).to_be_bytes());

        // Questions
        for q in &self.questions {
            encode_domain_name(&q.name, &mut buf);
            buf.extend_from_slice(&q.qtype.to_be_bytes());
            let qclass = if q.unicast_response {
                q.qclass | CACHE_FLUSH_BIT
            } else {
                q.qclass
            };
            buf.extend_from_slice(&qclass.to_be_bytes());
        }

        // Helper closure for records
        let write_record = |record: &DnsRecord, out: &mut Vec<u8>| {
            encode_domain_name(&record.name, out);
            out.extend_from_slice(&record.rtype.to_be_bytes());
            let rclass = if record.cache_flush {
                record.rclass | CACHE_FLUSH_BIT
            } else {
                record.rclass
            };
            out.extend_from_slice(&rclass.to_be_bytes());
            out.extend_from_slice(&record.ttl.to_be_bytes());

            // Serialize RDATA
            let rdata_start = out.len();
            out.extend_from_slice(&0u16.to_be_bytes()); // Placeholder for rdlength

            match &record.rdata {
                RData::A(ip) => {
                    out.extend_from_slice(&ip.octets());
                }
                RData::Aaaa(ip) => {
                    out.extend_from_slice(&ip.octets());
                }
                RData::Ptr(target) => {
                    encode_domain_name(target, out);
                }
                RData::Srv {
                    priority,
                    weight,
                    port,
                    target,
                } => {
                    out.extend_from_slice(&priority.to_be_bytes());
                    out.extend_from_slice(&weight.to_be_bytes());
                    out.extend_from_slice(&port.to_be_bytes());
                    encode_domain_name(target, out);
                }
                RData::Txt(strings) => {
                    for s in strings {
                        let bytes = s.as_bytes();
                        let len = bytes.len().min(255) as u8;
                        out.push(len);
                        out.extend_from_slice(&bytes[..len as usize]);
                    }
                }
                RData::Raw(raw) => {
                    out.extend_from_slice(raw);
                }
            }

            let rdlength = (out.len() - rdata_start - 2) as u16;
            out[rdata_start..rdata_start + 2].copy_from_slice(&rdlength.to_be_bytes());
        };

        for r in &self.answers {
            write_record(r, &mut buf);
        }
        for r in &self.authorities {
            write_record(r, &mut buf);
        }
        for r in &self.additionals {
            write_record(r, &mut buf);
        }

        buf
    }

    /// Deserializes a DNS packet from raw wire bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < 12 {
            return Err(NetworkError::PacketMalformed(
                "Header is shorter than 12 bytes".to_string(),
            ));
        }

        let id = u16::from_be_bytes([bytes[0], bytes[1]]);
        let flags = u16::from_be_bytes([bytes[2], bytes[3]]);
        let qdcount = u16::from_be_bytes([bytes[4], bytes[5]]) as usize;
        let ancount = u16::from_be_bytes([bytes[6], bytes[7]]) as usize;
        let nscount = u16::from_be_bytes([bytes[8], bytes[9]]) as usize;
        let arcount = u16::from_be_bytes([bytes[10], bytes[11]]) as usize;

        let mut offset = 12;

        let mut questions = Vec::with_capacity(qdcount);
        for _ in 0..qdcount {
            let (name, next_offset) = decode_domain_name(bytes, offset)?;
            offset = next_offset;
            if offset + 4 > bytes.len() {
                return Err(NetworkError::PacketMalformed(
                    "Truncated question".to_string(),
                ));
            }
            let qtype = u16::from_be_bytes([bytes[offset], bytes[offset + 1]]);
            let raw_qclass = u16::from_be_bytes([bytes[offset + 2], bytes[offset + 3]]);
            offset += 4;

            let unicast_response = (raw_qclass & CACHE_FLUSH_BIT) != 0;
            let qclass = raw_qclass & !CACHE_FLUSH_BIT;

            questions.push(DnsQuestion {
                name,
                qtype,
                qclass,
                unicast_response,
            });
        }

        let mut parse_records = |count: usize| -> Result<Vec<DnsRecord>> {
            let mut list = Vec::with_capacity(count);
            for _ in 0..count {
                let (name, next_offset) = decode_domain_name(bytes, offset)?;
                offset = next_offset;
                if offset + 10 > bytes.len() {
                    return Err(NetworkError::PacketMalformed(
                        "Truncated record header".to_string(),
                    ));
                }

                let rtype = u16::from_be_bytes([bytes[offset], bytes[offset + 1]]);
                let raw_rclass = u16::from_be_bytes([bytes[offset + 2], bytes[offset + 3]]);
                let ttl = u32::from_be_bytes([
                    bytes[offset + 4],
                    bytes[offset + 5],
                    bytes[offset + 6],
                    bytes[offset + 7],
                ]);
                let rdlength = u16::from_be_bytes([bytes[offset + 8], bytes[offset + 9]]) as usize;
                offset += 10;

                if offset + rdlength > bytes.len() {
                    return Err(NetworkError::PacketMalformed(
                        "Record data extends beyond packet length".to_string(),
                    ));
                }

                let rdata_bytes = &bytes[offset..offset + rdlength];
                let rdata = match rtype {
                    TYPE_A if rdlength == 4 => RData::A(Ipv4Addr::new(
                        rdata_bytes[0],
                        rdata_bytes[1],
                        rdata_bytes[2],
                        rdata_bytes[3],
                    )),
                    TYPE_AAAA if rdlength == 16 => {
                        let mut octets = [0u8; 16];
                        octets.copy_from_slice(rdata_bytes);
                        RData::Aaaa(Ipv6Addr::from(octets))
                    }
                    TYPE_PTR => {
                        let (ptr_target, _) = decode_domain_name(bytes, offset)?;
                        RData::Ptr(ptr_target)
                    }
                    TYPE_SRV if rdlength >= 6 => {
                        let priority = u16::from_be_bytes([rdata_bytes[0], rdata_bytes[1]]);
                        let weight = u16::from_be_bytes([rdata_bytes[2], rdata_bytes[3]]);
                        let port = u16::from_be_bytes([rdata_bytes[4], rdata_bytes[5]]);
                        let (target, _) = decode_domain_name(bytes, offset + 6)?;
                        RData::Srv {
                            priority,
                            weight,
                            port,
                            target,
                        }
                    }
                    TYPE_TXT => {
                        let mut strings = Vec::new();
                        let mut txt_offset = 0;
                        while txt_offset < rdlength {
                            let len = rdata_bytes[txt_offset] as usize;
                            txt_offset += 1;
                            if txt_offset + len <= rdlength {
                                let s = String::from_utf8_lossy(
                                    &rdata_bytes[txt_offset..txt_offset + len],
                                );
                                strings.push(s.to_string());
                                txt_offset += len;
                            } else {
                                break;
                            }
                        }
                        RData::Txt(strings)
                    }
                    _ => RData::Raw(rdata_bytes.to_vec()),
                };

                offset += rdlength;

                let cache_flush = (raw_rclass & CACHE_FLUSH_BIT) != 0;
                let rclass = raw_rclass & !CACHE_FLUSH_BIT;

                list.push(DnsRecord {
                    name,
                    rtype,
                    rclass,
                    cache_flush,
                    ttl,
                    rdata,
                });
            }
            Ok(list)
        };

        let answers = parse_records(ancount)?;
        let authorities = parse_records(nscount)?;
        let additionals = parse_records(arcount)?;

        Ok(Self {
            id,
            flags,
            questions,
            answers,
            authorities,
            additionals,
        })
    }
}

fn encode_domain_name(name: &str, out: &mut Vec<u8>) {
    for label in name.split('.') {
        let label_trimmed = label.trim();
        if !label_trimmed.is_empty() {
            let bytes = label_trimmed.as_bytes();
            let len = bytes.len().min(63) as u8;
            out.push(len);
            out.extend_from_slice(&bytes[..len as usize]);
        }
    }
    out.push(0); // Null terminator
}

fn decode_domain_name(bytes: &[u8], mut offset: usize) -> Result<(String, usize)> {
    let mut labels = Vec::new();
    let mut jumped = false;
    let mut return_offset = offset;
    let mut steps = 0;

    while offset < bytes.len() {
        steps += 1;
        if steps > 100 {
            return Err(NetworkError::PacketMalformed(
                "Domain name pointer loop detected".to_string(),
            ));
        }

        let len_byte = bytes[offset];
        if len_byte == 0 {
            if !jumped {
                return_offset = offset + 1;
            }
            break;
        }

        // Pointer compression check (leading two bits are 11 -> 0xC0)
        if (len_byte & 0xC0) == 0xC0 {
            if offset + 1 >= bytes.len() {
                return Err(NetworkError::PacketMalformed(
                    "Truncated domain name pointer".to_string(),
                ));
            }
            let ptr_offset = (((len_byte & 0x3F) as usize) << 8) | (bytes[offset + 1] as usize);
            if !jumped {
                return_offset = offset + 2;
                jumped = true;
            }
            offset = ptr_offset;
            continue;
        }

        let label_len = len_byte as usize;
        offset += 1;
        if offset + label_len > bytes.len() {
            return Err(NetworkError::PacketMalformed(
                "Truncated domain name label".to_string(),
            ));
        }

        let label = String::from_utf8_lossy(&bytes[offset..offset + label_len]);
        labels.push(label.to_string());
        offset += label_len;

        if !jumped {
            return_offset = offset;
        }
    }

    let joined = labels.join(".");
    Ok((joined, return_offset))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn test_dns_packet_query_roundtrip() {
        let question = DnsQuestion {
            name: "_aissistant._udp.local".to_string(),
            qtype: TYPE_PTR,
            qclass: CLASS_IN,
            unicast_response: false,
        };

        let packet = DnsPacket::new_query(0x1234, vec![question]);
        let bytes = packet.to_bytes();
        let parsed = DnsPacket::from_bytes(&bytes).unwrap();

        assert_eq!(parsed.id, 0x1234);
        assert_eq!(parsed.questions.len(), 1);
        assert_eq!(parsed.questions[0].name, "_aissistant._udp.local");
        assert_eq!(parsed.questions[0].qtype, TYPE_PTR);
        assert_eq!(parsed.questions[0].qclass, CLASS_IN);
    }

    #[test]
    fn test_dns_packet_response_roundtrip_with_srv_txt_a() {
        let srv = DnsRecord {
            name: "Desktop._aissistant._udp.local".to_string(),
            rtype: TYPE_SRV,
            rclass: CLASS_IN,
            cache_flush: true,
            ttl: 120,
            rdata: RData::Srv {
                priority: 0,
                weight: 0,
                port: 51820,
                target: "desktop-host.local".to_string(),
            },
        };

        let txt = DnsRecord {
            name: "Desktop._aissistant._udp.local".to_string(),
            rtype: TYPE_TXT,
            rclass: CLASS_IN,
            cache_flush: true,
            ttl: 120,
            rdata: RData::Txt(vec![
                "proto_ver=1".to_string(),
                "platform=windows".to_string(),
                "fingerprint=a1b2c3d4e5f60708".to_string(),
            ]),
        };

        let a = DnsRecord {
            name: "desktop-host.local".to_string(),
            rtype: TYPE_A,
            rclass: CLASS_IN,
            cache_flush: true,
            ttl: 120,
            rdata: RData::A(Ipv4Addr::new(192, 168, 1, 100)),
        };

        let packet = DnsPacket::new_response(0, vec![srv], vec![txt, a]);
        let bytes = packet.to_bytes();
        let parsed = DnsPacket::from_bytes(&bytes).unwrap();

        assert_eq!(parsed.answers.len(), 1);
        assert_eq!(parsed.additionals.len(), 2);

        if let RData::Srv { port, target, .. } = &parsed.answers[0].rdata {
            assert_eq!(*port, 51820);
            assert_eq!(target, "desktop-host.local");
        } else {
            panic!("Expected SRV record");
        }

        if let RData::Txt(strings) = &parsed.additionals[0].rdata {
            assert_eq!(strings.len(), 3);
            assert!(strings.contains(&"platform=windows".to_string()));
        } else {
            panic!("Expected TXT record");
        }

        if let RData::A(ip) = &parsed.additionals[1].rdata {
            assert_eq!(*ip, Ipv4Addr::new(192, 168, 1, 100));
        } else {
            panic!("Expected A record");
        }
    }
}
