//! Network and mDNS discovery error taxonomy.

use thiserror::Error;

/// Result alias for network operations.
pub type Result<T> = std::result::Result<T, NetworkError>;

/// Network subsystem error variants.
#[derive(Debug, Error)]
pub enum NetworkError {
    /// Failed to bind UDP socket to local address/port.
    #[error("Failed to bind UDP socket at {endpoint}: {reason}")]
    SocketBindFailed {
        /// Target socket endpoint.
        endpoint: String,
        /// Underlying failure reason.
        reason: String,
    },

    /// Failed to join the multicast discovery group.
    #[error("Failed to join multicast group {group}: {reason}")]
    MulticastJoinFailed {
        /// Multicast IP group.
        group: String,
        /// Underlying failure reason.
        reason: String,
    },

    /// Malformed or truncated DNS packet received over the wire.
    #[error("Malformed DNS packet: {0}")]
    PacketMalformed(String),

    /// Standard I/O socket error.
    #[error("I/O network error: {0}")]
    Io(#[from] std::io::Error),

    /// Generic internal network subsystem failure.
    #[error("Internal network error: {0}")]
    Internal(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_network_error_display() {
        let err = NetworkError::PacketMalformed("Truncated header".to_string());
        assert_eq!(err.to_string(), "Malformed DNS packet: Truncated header");
    }
}
