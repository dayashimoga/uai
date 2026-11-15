//! Strongly typed, validated domain identifiers.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::ops::Deref;
use std::str::FromStr;
use uuid::Uuid;

use crate::error::CoreError;

macro_rules! define_id {
    ($name:ident, $entity:literal, $validator:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(String);

        impl $name {
            #[doc = concat!("Creates a new validated `", stringify!($name), "` from a string slice.")]
            pub fn new(raw: impl AsRef<str>) -> Result<Self, CoreError> {
                let s = raw.as_ref().trim();
                $validator(s)?;
                Ok(Self(s.to_string()))
            }

            #[doc = concat!("Returns the inner string reference of this `", stringify!($name), "`.")]
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl FromStr for $name {
            type Err = CoreError;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Self::new(s)
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                &self.0
            }
        }

        impl Deref for $name {
            type Target = str;

            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        impl Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                serializer.serialize_str(&self.0)
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                let s = String::deserialize(deserializer)?;
                Self::new(s).map_err(serde::de::Error::custom)
            }
        }
    };
}

fn validate_generic_slug(
    entity: &'static str,
    s: &str,
    min: usize,
    max: usize,
) -> Result<(), CoreError> {
    if s.len() < min || s.len() > max {
        return Err(CoreError::InvalidId {
            entity,
            value: s.to_string(),
            reason: format!(
                "length must be between {min} and {max} characters (got {})",
                s.len()
            ),
        });
    }

    if !s
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == ':')
    {
        return Err(CoreError::InvalidId {
            entity,
            value: s.to_string(),
            reason: "characters must be ASCII alphanumeric, '-', '_', or ':'".to_string(),
        });
    }

    Ok(())
}

fn validate_device_id(s: &str) -> Result<(), CoreError> {
    validate_generic_slug("Device", s, 3, 64)
}

fn validate_conversation_id(s: &str) -> Result<(), CoreError> {
    validate_generic_slug("Conversation", s, 3, 64)
}

fn validate_turn_id(s: &str) -> Result<(), CoreError> {
    validate_generic_slug("Turn", s, 1, 64)
}

fn validate_audit_entry_id(s: &str) -> Result<(), CoreError> {
    validate_generic_slug("AuditEntry", s, 1, 64)
}

fn validate_skill_id(s: &str) -> Result<(), CoreError> {
    if s.len() < 3 || s.len() > 64 {
        return Err(CoreError::InvalidId {
            entity: "Skill",
            value: s.to_string(),
            reason: format!(
                "length must be between 3 and 64 characters (got {})",
                s.len()
            ),
        });
    }

    let segments: Vec<&str> = s.split('.').collect();
    if segments.len() < 2 {
        return Err(CoreError::InvalidId {
            entity: "Skill",
            value: s.to_string(),
            reason: "skill ID must contain at least one namespace separator '.' (e.g. 'file.read')"
                .to_string(),
        });
    }

    for segment in &segments {
        if segment.is_empty() {
            return Err(CoreError::InvalidId {
                entity: "Skill",
                value: s.to_string(),
                reason: "skill namespace segments cannot be empty".to_string(),
            });
        }
        if !segment
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
        {
            return Err(CoreError::InvalidId {
                entity: "Skill",
                value: s.to_string(),
                reason:
                    "skill namespace segments must be lowercase ASCII alphanumeric with '_' or '-'"
                        .to_string(),
            });
        }
    }

    Ok(())
}

fn validate_model_id(s: &str) -> Result<(), CoreError> {
    if s.len() < 2 || s.len() > 128 {
        return Err(CoreError::InvalidId {
            entity: "Model",
            value: s.to_string(),
            reason: format!(
                "length must be between 2 and 128 characters (got {})",
                s.len()
            ),
        });
    }

    if !s.chars().all(|c| {
        c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' || c == '/' || c == ':'
    }) {
        return Err(CoreError::InvalidId {
            entity: "Model",
            value: s.to_string(),
            reason: "characters must be ASCII alphanumeric, '-', '_', '.', '/', or ':'".to_string(),
        });
    }

    Ok(())
}

define_id!(
    DeviceId,
    "Device",
    validate_device_id,
    "Cryptographically bound unique device identifier."
);
define_id!(
    ConversationId,
    "Conversation",
    validate_conversation_id,
    "Identifier for a multi-turn conversation session."
);
define_id!(
    TurnId,
    "Turn",
    validate_turn_id,
    "Identifier for an individual interaction turn within a conversation."
);
define_id!(
    SkillId,
    "Skill",
    validate_skill_id,
    "Namespaced identifier for an assistant skill or tool (e.g. `file.read`)."
);
define_id!(
    ModelId,
    "Model",
    validate_model_id,
    "Identifier for an AI model or checkpoint."
);
define_id!(
    AuditEntryId,
    "AuditEntry",
    validate_audit_entry_id,
    "Identifier for a tamper-evident audit log record."
);

impl DeviceId {
    /// Generates a new random UUIDv4-based device identifier.
    #[must_use]
    pub fn generate() -> Self {
        Self(Uuid::new_v4().to_string())
    }
}

impl ConversationId {
    /// Generates a new random UUIDv4-based conversation identifier.
    #[must_use]
    pub fn generate() -> Self {
        Self(Uuid::new_v4().to_string())
    }
}

impl TurnId {
    /// Generates a new random UUIDv4-based turn identifier.
    #[must_use]
    pub fn generate() -> Self {
        Self(Uuid::new_v4().to_string())
    }

    /// Creates a turn identifier from a sequential sequence number.
    #[must_use]
    pub fn from_sequence(seq: u64) -> Self {
        Self(format!("turn_{seq:06}"))
    }
}

impl AuditEntryId {
    /// Generates a new random UUIDv4-based audit entry identifier.
    #[must_use]
    pub fn generate() -> Self {
        Self(Uuid::new_v4().to_string())
    }

    /// Creates an audit entry identifier from a sequential sequence number.
    #[must_use]
    pub fn from_sequence(seq: u64) -> Self {
        Self(format!("audit_{seq:010}"))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_device_id_generation_and_validation() {
        let id = DeviceId::generate();
        assert!(!id.as_str().is_empty());
        assert_eq!(id, DeviceId::new(id.as_str()).unwrap());
        assert_eq!(id.to_string(), id.as_str());

        let from_str: DeviceId = id.as_str().parse().unwrap();
        assert_eq!(id, from_str);

        // Valid custom slug
        let custom = DeviceId::new("dev_workstation-01").unwrap();
        assert_eq!(custom.as_str(), "dev_workstation-01");
        assert_eq!(&*custom, "dev_workstation-01");
        assert_eq!(custom.as_ref(), "dev_workstation-01");

        // Invalid: too short
        assert!(DeviceId::new("a").is_err());
        // Invalid: too long
        let too_long = "a".repeat(65);
        assert!(DeviceId::new(too_long).is_err());
        // Invalid characters
        assert!(DeviceId::new("device@home").is_err());
    }

    #[test]
    fn test_conversation_and_turn_ids() {
        let conv_id = ConversationId::generate();
        assert!(ConversationId::new(conv_id.as_str()).is_ok());
        let conv_parsed: ConversationId = conv_id.as_str().parse().unwrap();
        assert_eq!(conv_id, conv_parsed);

        let turn_id = TurnId::from_sequence(42);
        assert_eq!(turn_id.as_str(), "turn_000042");
        assert_eq!(turn_id.to_string(), "turn_000042");

        let turn_gen = TurnId::generate();
        assert!(TurnId::new(turn_gen.as_str()).is_ok());
        let turn_parsed: TurnId = turn_id.as_str().parse().unwrap();
        assert_eq!(turn_id, turn_parsed);
    }

    #[test]
    fn test_skill_id_validation() {
        // Valid skills
        assert!(SkillId::new("file.read").is_ok());
        assert!(SkillId::new("system.media.play").is_ok());
        assert!(SkillId::new("device_net.remote-exec").is_ok());

        let parsed: SkillId = "system.volume".parse().unwrap();
        assert_eq!(parsed.as_str(), "system.volume");

        // Invalid: too short
        assert!(SkillId::new("a.").is_err());
        // Invalid: too long
        let too_long = format!("a.{}.b", "x".repeat(65));
        assert!(SkillId::new(too_long).is_err());
        // Invalid: no namespace dot
        assert!(SkillId::new("fileread").is_err());
        // Invalid: empty segment
        assert!(SkillId::new("file..read").is_err());
        // Invalid: uppercase
        assert!(SkillId::new("File.Read").is_err());
        // Invalid character in segment
        assert!(SkillId::new("file.read!").is_err());
    }

    #[test]
    fn test_model_id_validation() {
        assert!(ModelId::new("qwen2.5-coder-7b-instruct:q4_k_m").is_ok());
        assert!(ModelId::new("meta-llama/Llama-3.2-3B-Instruct").is_ok());
        assert!(ModelId::new("whisper-base.en").is_ok());

        let parsed: ModelId = "phi-3.5:mini".parse().unwrap();
        assert_eq!(parsed.as_str(), "phi-3.5:mini");

        // Invalid: empty or whitespace only
        assert!(ModelId::new("").is_err());
        // Invalid: too long
        let too_long = "a".repeat(129);
        assert!(ModelId::new(too_long).is_err());
        assert!(ModelId::new("model with spaces").is_err());
        assert!(ModelId::new("model?bad").is_err());
    }

    #[test]
    fn test_audit_entry_id() {
        let audit = AuditEntryId::from_sequence(1);
        assert_eq!(audit.as_str(), "audit_0000000001");
        assert_eq!(audit.to_string(), "audit_0000000001");
        let parsed: AuditEntryId = audit.as_str().parse().unwrap();
        assert_eq!(audit, parsed);

        let gen = AuditEntryId::generate();
        assert!(AuditEntryId::new(gen.as_str()).is_ok());
    }

    #[test]
    fn test_serde_roundtrip() {
        let skill = SkillId::new("audio.tts").unwrap();
        let json = serde_json::to_string(&skill).unwrap();
        assert_eq!(json, "\"audio.tts\"");
        let deserialized: SkillId = serde_json::from_str(&json).unwrap();
        assert_eq!(skill, deserialized);

        // Deserializing invalid format must fail
        assert!(serde_json::from_str::<SkillId>("\"invalid\"").is_err());
    }
}
