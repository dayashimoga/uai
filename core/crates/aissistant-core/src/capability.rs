//! Five-platform capability matrix, runtime status detection, and graceful degradation models.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;

use crate::error::CoreError;

/// Target operating system platforms supported by the Universal AI Assistant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlatformTarget {
    /// Microsoft Windows (10 / 11)
    Windows,
    /// Linux (X11 / Wayland desktop distributions)
    Linux,
    /// Apple macOS (13+ Ventura, Sonoma, Sequoia)
    MacOS,
    /// Google Android (API 29+ / Android 10+)
    Android,
    /// Apple iOS (iOS 16+)
    Ios,
}

impl PlatformTarget {
    /// Detects the target platform of the currently running binary.
    #[must_use]
    pub const fn current() -> Self {
        #[cfg(target_os = "windows")]
        {
            Self::Windows
        }
        #[cfg(target_os = "linux")]
        {
            Self::Linux
        }
        #[cfg(target_os = "macos")]
        {
            Self::MacOS
        }
        #[cfg(target_os = "android")]
        {
            Self::Android
        }
        #[cfg(target_os = "ios")]
        {
            Self::Ios
        }
        #[cfg(not(any(
            target_os = "windows",
            target_os = "linux",
            target_os = "macos",
            target_os = "android",
            target_os = "ios"
        )))]
        {
            // Default fallback for unknown target environments
            Self::Linux
        }
    }

    /// Returns the static string representation of this target platform.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Windows => "windows",
            Self::Linux => "linux",
            Self::MacOS => "macos",
            Self::Android => "android",
            Self::Ios => "ios",
        }
    }

    /// Returns all supported target platforms.
    #[must_use]
    pub const fn all() -> &'static [Self] {
        &[
            Self::Windows,
            Self::Linux,
            Self::MacOS,
            Self::Android,
            Self::Ios,
        ]
    }
}

impl fmt::Display for PlatformTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for PlatformTarget {
    type Err = CoreError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "windows" | "win" => Ok(Self::Windows),
            "linux" => Ok(Self::Linux),
            "macos" | "darwin" | "osx" => Ok(Self::MacOS),
            "android" => Ok(Self::Android),
            "ios" => Ok(Self::Ios),
            other => Err(CoreError::InvalidId {
                entity: "PlatformTarget",
                value: other.to_string(),
                reason: "supported targets are windows, linux, macos, android, ios".to_string(),
            }),
        }
    }
}

/// System and AI capabilities tracked by the runtime matrix.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    /// Local LLM GGUF inference via llama.cpp.
    LocalLlm,
    /// Offline STT, VAD, and neural TTS via sherpa-onnx.
    OfflineSpeech,
    /// Continuous background wake-word listening.
    BackgroundWakeWord,
    /// Local file and directory management.
    FileSystem,
    /// Application launching and window management.
    ApplicationManagement,
    /// System media playback and volume control.
    SystemMediaControl,
    /// Local screen capture and streaming host capability.
    ScreenStreamingHost,
    /// Remote input injection (mouse/keyboard control) as host.
    RemoteInputInjectionHost,
    /// Local mDNS discovery and mutual-TLS QUIC transport.
    LanPeerNetworking,
    /// Offline optical character recognition and multimodal vision understanding.
    VisionOcr,
    /// Offline multilingual translation and live voice interpreter dialogue mode.
    LiveInterpreter,
}

impl Capability {
    /// Returns the canonical string identifier for this capability.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::LocalLlm => "local_llm",
            Self::OfflineSpeech => "offline_speech",
            Self::BackgroundWakeWord => "background_wake_word",
            Self::FileSystem => "file_system",
            Self::ApplicationManagement => "application_management",
            Self::SystemMediaControl => "system_media_control",
            Self::ScreenStreamingHost => "screen_streaming_host",
            Self::RemoteInputInjectionHost => "remote_input_injection_host",
            Self::LanPeerNetworking => "lan_peer_networking",
            Self::VisionOcr => "vision_ocr",
            Self::LiveInterpreter => "live_interpreter",
        }
    }

    /// Returns a slice containing all defined capabilities.
    #[must_use]
    pub const fn all() -> &'static [Self] {
        &[
            Self::LocalLlm,
            Self::OfflineSpeech,
            Self::BackgroundWakeWord,
            Self::FileSystem,
            Self::ApplicationManagement,
            Self::SystemMediaControl,
            Self::ScreenStreamingHost,
            Self::RemoteInputInjectionHost,
            Self::LanPeerNetworking,
            Self::VisionOcr,
            Self::LiveInterpreter,
        ]
    }
}

impl fmt::Display for Capability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for Capability {
    type Err = CoreError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "local_llm" => Ok(Self::LocalLlm),
            "offline_speech" => Ok(Self::OfflineSpeech),
            "background_wake_word" => Ok(Self::BackgroundWakeWord),
            "file_system" => Ok(Self::FileSystem),
            "application_management" => Ok(Self::ApplicationManagement),
            "system_media_control" => Ok(Self::SystemMediaControl),
            "screen_streaming_host" => Ok(Self::ScreenStreamingHost),
            "remote_input_injection_host" => Ok(Self::RemoteInputInjectionHost),
            "lan_peer_networking" => Ok(Self::LanPeerNetworking),
            "vision_ocr" => Ok(Self::VisionOcr),
            "live_interpreter" => Ok(Self::LiveInterpreter),
            other => Err(CoreError::InvalidId {
                entity: "Capability",
                value: other.to_string(),
                reason: "unknown capability identifier".to_string(),
            }),
        }
    }
}

/// Operational status of a capability on a specific platform or runtime instance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum CapabilityStatus {
    /// Fully supported without OS restriction.
    Supported,
    /// Supported with platform-enforced limitations.
    Degraded {
        /// Explanation of the degradation and recommended user behavior.
        limitation: String,
    },
    /// Unavailable due to OS sandboxing or architectural constraints.
    Unsupported {
        /// Reason for lack of support.
        reason: String,
    },
    /// Requires an explicit OS-level permission grant.
    RequiresPermission {
        /// The required permission identifier (e.g. `microphone`, `accessibility`, `tcc_screen`).
        permission: String,
        /// Whether the permission is currently granted.
        granted: bool,
    },
}

impl CapabilityStatus {
    /// Returns true if the capability is operational (supported or degraded, and permissions granted).
    #[must_use]
    pub fn is_available(&self) -> bool {
        match self {
            Self::Supported | Self::Degraded { .. } => true,
            Self::RequiresPermission { granted, .. } => *granted,
            Self::Unsupported { .. } => false,
        }
    }

    /// Returns true if the capability operates in a degraded mode.
    #[must_use]
    pub const fn is_degraded(&self) -> bool {
        matches!(self, Self::Degraded { .. })
    }

    /// Returns true if the capability is completely unsupported on the target.
    #[must_use]
    pub const fn is_unsupported(&self) -> bool {
        matches!(self, Self::Unsupported { .. })
    }
}

/// Reference matrix mapping capabilities across the five target platforms.
#[derive(Debug, Clone, Copy)]
pub struct CapabilityMatrix;

impl CapabilityMatrix {
    /// Evaluates the baseline capability status for a given platform target.
    #[must_use]
    pub fn query(target: PlatformTarget, capability: Capability) -> CapabilityStatus {
        match capability {
            Capability::LocalLlm | Capability::OfflineSpeech | Capability::SystemMediaControl => {
                CapabilityStatus::Supported
            }

            Capability::BackgroundWakeWord => match target {
                PlatformTarget::Windows | PlatformTarget::Linux | PlatformTarget::MacOS => {
                    CapabilityStatus::Supported
                }
                PlatformTarget::Android => CapabilityStatus::RequiresPermission {
                    permission: "android.permission.RECORD_AUDIO".to_string(),
                    granted: false,
                },
                PlatformTarget::Ios => CapabilityStatus::Degraded {
                    limitation: "Active only while app is in foreground or active audio session; background trigger via Siri Shortcuts or Action Button".to_string(),
                },
            },

            Capability::FileSystem => match target {
                PlatformTarget::Windows | PlatformTarget::Linux => CapabilityStatus::Supported,
                PlatformTarget::MacOS => CapabilityStatus::RequiresPermission {
                    permission: "tcc.files_and_folders".to_string(),
                    granted: false,
                },
                PlatformTarget::Android => CapabilityStatus::RequiresPermission {
                    permission: "android.permission.MANAGE_EXTERNAL_STORAGE".to_string(),
                    granted: false,
                },
                PlatformTarget::Ios => CapabilityStatus::Degraded {
                    limitation: "Scoped to App Sandbox and UIDocumentPicker security-scoped bookmarks".to_string(),
                },
            },

            Capability::ApplicationManagement => match target {
                PlatformTarget::Windows | PlatformTarget::Linux => CapabilityStatus::Supported,
                PlatformTarget::MacOS => CapabilityStatus::RequiresPermission {
                    permission: "tcc.accessibility".to_string(),
                    granted: false,
                },
                PlatformTarget::Android => CapabilityStatus::RequiresPermission {
                    permission: "android.permission.BIND_ACCESSIBILITY_SERVICE".to_string(),
                    granted: false,
                },
                PlatformTarget::Ios => CapabilityStatus::Degraded {
                    limitation: "URL schemes and App Intents only; cross-app window management prohibited by sandbox".to_string(),
                },
            },

            Capability::ScreenStreamingHost => match target {
                PlatformTarget::Windows | PlatformTarget::Linux => CapabilityStatus::Supported,
                PlatformTarget::MacOS => CapabilityStatus::RequiresPermission {
                    permission: "tcc.screen_recording".to_string(),
                    granted: false,
                },
                PlatformTarget::Android => CapabilityStatus::RequiresPermission {
                    permission: "android.media.projection".to_string(),
                    granted: false,
                },
                PlatformTarget::Ios => CapabilityStatus::Degraded {
                    limitation: "View-only capture via ReplayKit Broadcast Extension".to_string(),
                },
            },

            Capability::RemoteInputInjectionHost => match target {
                PlatformTarget::Windows | PlatformTarget::Linux => CapabilityStatus::Supported,
                PlatformTarget::MacOS => CapabilityStatus::RequiresPermission {
                    permission: "tcc.accessibility".to_string(),
                    granted: false,
                },
                PlatformTarget::Android => CapabilityStatus::RequiresPermission {
                    permission: "android.permission.BIND_ACCESSIBILITY_SERVICE".to_string(),
                    granted: false,
                },
                PlatformTarget::Ios => CapabilityStatus::Unsupported {
                    reason: "iOS forbids synthetic cross-app input injection; iOS operates as a remote controller client".to_string(),
                },
            },

            Capability::LanPeerNetworking => match target {
                PlatformTarget::Windows
                | PlatformTarget::Linux
                | PlatformTarget::MacOS
                | PlatformTarget::Android => CapabilityStatus::Supported,
                PlatformTarget::Ios => CapabilityStatus::RequiresPermission {
                    permission: "ios.permission.LOCAL_NETWORK".to_string(),
                    granted: false,
                },
            },

            Capability::VisionOcr => match target {
                PlatformTarget::Windows | PlatformTarget::Linux | PlatformTarget::MacOS => {
                    CapabilityStatus::Supported
                }
                PlatformTarget::Android => CapabilityStatus::RequiresPermission {
                    permission: "android.permission.CAMERA".to_string(),
                    granted: false,
                },
                PlatformTarget::Ios => CapabilityStatus::RequiresPermission {
                    permission: "ios.permission.CAMERA".to_string(),
                    granted: false,
                },
            },

            Capability::LiveInterpreter => CapabilityStatus::Supported,
        }
    }

    /// Computes the complete baseline status map for a target platform.
    #[must_use]
    pub fn matrix_for(target: PlatformTarget) -> HashMap<Capability, CapabilityStatus> {
        let mut map = HashMap::new();
        for &cap in Capability::all() {
            map.insert(cap, Self::query(target, cap));
        }
        map
    }
}

/// Dynamic runtime registry tracking live capability statuses and granted permissions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityRegistry {
    target: PlatformTarget,
    statuses: HashMap<Capability, CapabilityStatus>,
}

impl CapabilityRegistry {
    /// Creates a registry for the specified target using baseline matrix defaults.
    #[must_use]
    pub fn new(target: PlatformTarget) -> Self {
        Self {
            target,
            statuses: CapabilityMatrix::matrix_for(target),
        }
    }

    /// Creates a registry for the current host operating system.
    #[must_use]
    pub fn for_current_host() -> Self {
        Self::new(PlatformTarget::current())
    }

    /// Returns the target platform associated with this registry.
    #[must_use]
    pub const fn target(&self) -> PlatformTarget {
        self.target
    }

    /// Returns the status of a specific capability.
    #[must_use]
    pub fn status(&self, capability: Capability) -> CapabilityStatus {
        self.statuses
            .get(&capability)
            .cloned()
            .unwrap_or_else(|| CapabilityMatrix::query(self.target, capability))
    }

    /// Returns true if the capability is operational in the current runtime state.
    #[must_use]
    pub fn is_supported(&self, capability: Capability) -> bool {
        self.status(capability).is_available()
    }

    /// Updates the granted state of a permission requirement for a capability.
    pub fn set_permission_granted(
        &mut self,
        capability: Capability,
        permission: &str,
        granted: bool,
    ) {
        if let Some(CapabilityStatus::RequiresPermission {
            permission: req_perm,
            granted: state,
        }) = self.statuses.get_mut(&capability)
        {
            if req_perm == permission {
                *state = granted;
            }
        }
    }

    /// Overrides the status of a capability explicitly.
    pub fn set_status(&mut self, capability: Capability, status: CapabilityStatus) {
        self.statuses.insert(capability, status);
    }

    /// Returns an immutable reference to the live statuses map.
    #[must_use]
    pub const fn all_statuses(&self) -> &HashMap<Capability, CapabilityStatus> {
        &self.statuses
    }
}

/// Host system hardware and OS probe result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostProbeResult {
    /// Detected target platform.
    pub target: PlatformTarget,
    /// Target CPU architecture (e.g. `x86_64`, `aarch64`).
    pub arch: String,
    /// Host operating system name.
    pub os: String,
    /// Host target family (e.g. `windows`, `unix`).
    pub family: String,
    /// Runtime capability registry for this host.
    pub registry: CapabilityRegistry,
}

impl HostProbeResult {
    /// Safely probes the current host environment without panics.
    #[must_use]
    pub fn probe() -> Self {
        let target = PlatformTarget::current();
        Self {
            target,
            arch: std::env::consts::ARCH.to_string(),
            os: std::env::consts::OS.to_string(),
            family: std::env::consts::FAMILY.to_string(),
            registry: CapabilityRegistry::for_current_host(),
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_platform_target_roundtrips() {
        for &target in PlatformTarget::all() {
            let s = target.as_str();
            let parsed: PlatformTarget = s.parse().unwrap();
            assert_eq!(target, parsed);
            assert_eq!(format!("{target}"), s);
        }

        assert!("unknown".parse::<PlatformTarget>().is_err());
    }

    #[test]
    fn test_capability_identifiers_roundtrips() {
        for &cap in Capability::all() {
            let s = cap.as_str();
            let parsed: Capability = s.parse().unwrap();
            assert_eq!(cap, parsed);
            assert_eq!(format!("{cap}"), s);
        }

        assert!("invalid_cap".parse::<Capability>().is_err());
    }

    #[test]
    fn test_ios_capability_degradation_matrix() {
        // iOS remote input injection must be unsupported
        let input_status =
            CapabilityMatrix::query(PlatformTarget::Ios, Capability::RemoteInputInjectionHost);
        assert!(input_status.is_unsupported());
        assert!(!input_status.is_available());

        // iOS background wake word must be degraded
        let wake_status =
            CapabilityMatrix::query(PlatformTarget::Ios, Capability::BackgroundWakeWord);
        assert!(wake_status.is_degraded());
        assert!(wake_status.is_available());

        // iOS file system must be degraded to sandbox
        let fs_status = CapabilityMatrix::query(PlatformTarget::Ios, Capability::FileSystem);
        assert!(fs_status.is_degraded());
        assert!(fs_status.is_available());

        // iOS application management must be degraded
        let app_status =
            CapabilityMatrix::query(PlatformTarget::Ios, Capability::ApplicationManagement);
        assert!(app_status.is_degraded());

        // iOS screen streaming host must be degraded (view-only)
        let screen_status =
            CapabilityMatrix::query(PlatformTarget::Ios, Capability::ScreenStreamingHost);
        assert!(screen_status.is_degraded());

        // iOS LAN requires permission
        let net_status =
            CapabilityMatrix::query(PlatformTarget::Ios, Capability::LanPeerNetworking);
        assert!(matches!(
            net_status,
            CapabilityStatus::RequiresPermission { .. }
        ));
    }

    #[test]
    fn test_desktop_capabilities() {
        // Windows and Linux have full desktop control capabilities
        assert_eq!(
            CapabilityMatrix::query(
                PlatformTarget::Windows,
                Capability::RemoteInputInjectionHost
            ),
            CapabilityStatus::Supported
        );
        assert_eq!(
            CapabilityMatrix::query(PlatformTarget::Linux, Capability::ScreenStreamingHost),
            CapabilityStatus::Supported
        );

        // macOS requires screen capture and accessibility TCC permissions
        let mac_screen =
            CapabilityMatrix::query(PlatformTarget::MacOS, Capability::ScreenStreamingHost);
        assert!(matches!(
            mac_screen,
            CapabilityStatus::RequiresPermission { .. }
        ));
        assert!(!mac_screen.is_available());

        let mac_input =
            CapabilityMatrix::query(PlatformTarget::MacOS, Capability::RemoteInputInjectionHost);
        assert!(matches!(
            mac_input,
            CapabilityStatus::RequiresPermission { .. }
        ));

        let mac_fs = CapabilityMatrix::query(PlatformTarget::MacOS, Capability::FileSystem);
        assert!(matches!(
            mac_fs,
            CapabilityStatus::RequiresPermission { .. }
        ));
    }

    #[test]
    fn test_android_capabilities() {
        let wake = CapabilityMatrix::query(PlatformTarget::Android, Capability::BackgroundWakeWord);
        assert!(matches!(wake, CapabilityStatus::RequiresPermission { .. }));

        let fs = CapabilityMatrix::query(PlatformTarget::Android, Capability::FileSystem);
        assert!(matches!(fs, CapabilityStatus::RequiresPermission { .. }));

        let screen =
            CapabilityMatrix::query(PlatformTarget::Android, Capability::ScreenStreamingHost);
        assert!(matches!(
            screen,
            CapabilityStatus::RequiresPermission { .. }
        ));

        let net = CapabilityMatrix::query(PlatformTarget::Android, Capability::LanPeerNetworking);
        assert_eq!(net, CapabilityStatus::Supported);
    }

    #[test]
    fn test_dynamic_permission_updating() {
        let mut registry = CapabilityRegistry::new(PlatformTarget::MacOS);
        assert!(!registry.is_supported(Capability::ScreenStreamingHost));

        registry.set_permission_granted(
            Capability::ScreenStreamingHost,
            "tcc.screen_recording",
            true,
        );
        assert!(registry.is_supported(Capability::ScreenStreamingHost));

        registry.set_permission_granted(
            Capability::ScreenStreamingHost,
            "tcc.screen_recording",
            false,
        );
        assert!(!registry.is_supported(Capability::ScreenStreamingHost));

        // Test non-matching permission string does not change state
        registry.set_permission_granted(Capability::ScreenStreamingHost, "wrong.permission", true);
        assert!(!registry.is_supported(Capability::ScreenStreamingHost));

        // Test override
        registry.set_status(Capability::ScreenStreamingHost, CapabilityStatus::Supported);
        assert!(registry.is_supported(Capability::ScreenStreamingHost));
    }

    #[test]
    fn test_host_probe_execution() {
        let probe = HostProbeResult::probe();
        assert!(!probe.arch.is_empty());
        assert!(!probe.os.is_empty());
        assert!(!probe.family.is_empty());
        assert_eq!(probe.target, PlatformTarget::current());
        assert!(probe.registry.is_supported(Capability::LocalLlm));
        assert_eq!(probe.registry.target(), PlatformTarget::current());
        assert_eq!(probe.registry.all_statuses().len(), Capability::all().len());
    }

    #[test]
    fn test_capability_serde_roundtrip() {
        let registry = CapabilityRegistry::for_current_host();
        let json = serde_json::to_string(&registry).unwrap();
        let deserialized: CapabilityRegistry = serde_json::from_str(&json).unwrap();
        assert_eq!(registry, deserialized);
    }
}
