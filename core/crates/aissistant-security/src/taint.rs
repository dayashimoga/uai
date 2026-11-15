//! Untrusted input taint tracking for indirect prompt injection defense.

use aissistant_core::DeviceId;
use serde::{Deserialize, Serialize};

/// Origin of external or untrusted data introduced into the conversation context.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "source_type", rename_all = "snake_case")]
pub enum TaintSource {
    /// Content ingested from a local filesystem path.
    LocalFile {
        /// Canonical or relative path of the file.
        path: String,
    },
    /// Content read from the OS system clipboard.
    Clipboard,
    /// Text extracted from a screenshot or camera frame via OCR.
    ScreenOcr,
    /// Payload or instruction received from a paired remote peer device.
    RemotePeerMessage {
        /// Identifier of the peer device that transmitted the payload.
        peer_id: DeviceId,
    },
    /// Arbitrary external web or network stream data.
    ExternalData {
        /// Label or description of the external source.
        label: String,
    },
}

/// Detailed record of a taint event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaintRecord {
    /// Origin of the untrusted input.
    pub source: TaintSource,
    /// Monotonic timestamp in milliseconds when the taint was registered.
    pub timestamp_ms: u64,
    /// Human-readable description or snippet of the untrusted input.
    pub description: String,
}

/// Live taint state of a conversation turn or task execution context.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", content = "records", rename_all = "snake_case")]
pub enum TaintState {
    /// Context contains only trusted user inputs and verified system prompts.
    #[default]
    Clean,
    /// Context has ingested untrusted external inputs capable of carrying adversarial instructions.
    Tainted(Vec<TaintRecord>),
}

impl TaintState {
    /// Returns true if the state contains one or more untrusted inputs.
    #[must_use]
    pub const fn is_tainted(&self) -> bool {
        matches!(self, Self::Tainted(_))
    }

    /// Returns true if the state is clean.
    #[must_use]
    pub const fn is_clean(&self) -> bool {
        matches!(self, Self::Clean)
    }

    /// Returns slice of active taint records.
    #[must_use]
    pub fn records(&self) -> &[TaintRecord] {
        match self {
            Self::Clean => &[],
            Self::Tainted(records) => records.as_slice(),
        }
    }

    /// Transitions the state to tainted by adding a new taint record.
    pub fn add_taint(
        &mut self,
        source: TaintSource,
        timestamp_ms: u64,
        description: impl Into<String>,
    ) {
        let record = TaintRecord {
            source,
            timestamp_ms,
            description: description.into(),
        };

        match self {
            Self::Clean => {
                *self = Self::Tainted(vec![record]);
            }
            Self::Tainted(records) => {
                records.push(record);
            }
        }
    }

    /// Resets the taint state back to clean (e.g. after a conversation turn is reset).
    pub fn clear(&mut self) {
        *self = Self::Clean;
    }
}

/// State tracker maintaining context taint across multi-step execution turns.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaintTracker {
    state: TaintState,
}

impl TaintTracker {
    /// Creates a new clean taint tracker.
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: TaintState::Clean,
        }
    }

    /// Returns the current taint state.
    #[must_use]
    pub const fn state(&self) -> &TaintState {
        &self.state
    }

    /// Records an untrusted data event.
    pub fn mark_tainted(
        &mut self,
        source: TaintSource,
        timestamp_ms: u64,
        description: impl Into<String>,
    ) {
        self.state.add_taint(source, timestamp_ms, description);
    }

    /// Clears any registered taint records.
    pub fn reset(&mut self) {
        self.state.clear();
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn test_initial_state_clean() {
        let tracker = TaintTracker::new();
        assert!(tracker.state().is_clean());
        assert!(!tracker.state().is_tainted());
        assert!(tracker.state().records().is_empty());
    }

    #[test]
    fn test_taint_accumulation_and_clearing() {
        let mut tracker = TaintTracker::new();

        tracker.mark_tainted(
            TaintSource::LocalFile {
                path: "untrusted_script.py".to_string(),
            },
            1000,
            "Downloaded file content",
        );

        assert!(tracker.state().is_tainted());
        assert_eq!(tracker.state().records().len(), 1);

        tracker.mark_tainted(TaintSource::Clipboard, 2000, "Copied snippet from browser");
        assert_eq!(tracker.state().records().len(), 2);

        tracker.reset();
        assert!(tracker.state().is_clean());
        assert_eq!(tracker.state().records().len(), 0);
    }

    #[test]
    fn test_peer_taint_source() {
        let mut state = TaintState::default();
        let peer_id = DeviceId::new("phone_01").unwrap();
        state.add_taint(
            TaintSource::RemotePeerMessage {
                peer_id: peer_id.clone(),
            },
            500,
            "Inbound remote command prompt",
        );

        assert!(state.is_tainted());
        let records = state.records();
        assert_eq!(records.len(), 1);
        assert!(matches!(
            &records[0].source,
            TaintSource::RemotePeerMessage { peer_id: id } if id == &peer_id
        ));
    }

    #[test]
    fn test_serde_roundtrip() {
        let mut tracker = TaintTracker::new();
        tracker.mark_tainted(TaintSource::ScreenOcr, 12345, "Window OCR text");

        let json = serde_json::to_string(&tracker).unwrap();
        let deserialized: TaintTracker = serde_json::from_str(&json).unwrap();
        assert_eq!(tracker, deserialized);
    }
}
