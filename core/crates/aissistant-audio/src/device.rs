use cpal::traits::{DeviceTrait, HostTrait};
use serde::{Deserialize, Serialize};

use crate::error::AudioError;

/// Discovered audio hardware device descriptor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioDeviceInfo {
    /// Device identifier or human-readable system name.
    pub name: String,
    /// True if this device is currently the OS system default.
    pub is_default: bool,
    /// Supports audio capture (microphone).
    pub is_input: bool,
    /// Supports audio playback (speaker/headphones).
    pub is_output: bool,
    /// Minimum supported sample rate in Hz.
    pub min_sample_rate: u32,
    /// Maximum supported sample rate in Hz.
    pub max_sample_rate: u32,
    /// Maximum supported channel count.
    pub max_channels: u16,
}

/// Host audio hardware discovery and configuration negotiator.
#[derive(Debug, Default)]
pub struct AudioDeviceManager;

impl AudioDeviceManager {
    /// List all available audio input devices (microphones).
    pub fn list_input_devices() -> Result<Vec<AudioDeviceInfo>, AudioError> {
        let host = cpal::default_host();
        let default_name = host.default_input_device().and_then(|d| d.name().ok());

        let devices = match host.input_devices() {
            Ok(d) => d,
            Err(e) => {
                return Err(AudioError::BackendError(format!(
                    "Failed to query input devices: {e}"
                )))
            }
        };

        let mut result = Vec::new();
        for device in devices {
            if let Ok(name) = device.name() {
                let is_default = default_name.as_ref().map(|n| n == &name).unwrap_or(false);
                let mut min_rate = u32::MAX;
                let mut max_rate = 0u32;
                let mut max_channels = 0u16;

                if let Ok(configs) = device.supported_input_configs() {
                    for cfg in configs {
                        min_rate = min_rate.min(cfg.min_sample_rate().0);
                        max_rate = max_rate.max(cfg.max_sample_rate().0);
                        max_channels = max_channels.max(cfg.channels());
                    }
                }

                if max_rate > 0 {
                    result.push(AudioDeviceInfo {
                        name,
                        is_default,
                        is_input: true,
                        is_output: false,
                        min_sample_rate: min_rate,
                        max_sample_rate: max_rate,
                        max_channels,
                    });
                }
            }
        }

        Ok(result)
    }

    /// List all available audio output devices (speakers/headphones).
    pub fn list_output_devices() -> Result<Vec<AudioDeviceInfo>, AudioError> {
        let host = cpal::default_host();
        let default_name = host.default_output_device().and_then(|d| d.name().ok());

        let devices = match host.output_devices() {
            Ok(d) => d,
            Err(e) => {
                return Err(AudioError::BackendError(format!(
                    "Failed to query output devices: {e}"
                )))
            }
        };

        let mut result = Vec::new();
        for device in devices {
            if let Ok(name) = device.name() {
                let is_default = default_name.as_ref().map(|n| n == &name).unwrap_or(false);
                let mut min_rate = u32::MAX;
                let mut max_rate = 0u32;
                let mut max_channels = 0u16;

                if let Ok(configs) = device.supported_output_configs() {
                    for cfg in configs {
                        min_rate = min_rate.min(cfg.min_sample_rate().0);
                        max_rate = max_rate.max(cfg.max_sample_rate().0);
                        max_channels = max_channels.max(cfg.channels());
                    }
                }

                if max_rate > 0 {
                    result.push(AudioDeviceInfo {
                        name,
                        is_default,
                        is_input: false,
                        is_output: true,
                        min_sample_rate: min_rate,
                        max_sample_rate: max_rate,
                        max_channels,
                    });
                }
            }
        }

        Ok(result)
    }

    /// Get the system default audio input device info if available.
    pub fn default_input_device() -> Result<Option<AudioDeviceInfo>, AudioError> {
        let inputs = Self::list_input_devices()?;
        Ok(inputs.into_iter().find(|d| d.is_default))
    }

    /// Get the system default audio output device info if available.
    pub fn default_output_device() -> Result<Option<AudioDeviceInfo>, AudioError> {
        let outputs = Self::list_output_devices()?;
        Ok(outputs.into_iter().find(|d| d.is_default))
    }

    /// Resolve a `cpal::Device` for input capture by name or default.
    pub fn find_input_cpal_device(name: Option<&str>) -> Result<cpal::Device, AudioError> {
        let host = cpal::default_host();
        if let Some(target_name) = name {
            let devices = host.input_devices().map_err(|e| {
                AudioError::BackendError(format!("Failed to list input devices: {e}"))
            })?;
            for dev in devices {
                if let Ok(dev_name) = dev.name() {
                    if dev_name == target_name {
                        return Ok(dev);
                    }
                }
            }
            Err(AudioError::DeviceNotFound(target_name.to_string()))
        } else {
            host.default_input_device().ok_or_else(|| {
                AudioError::DeviceNotFound("No default input device found".to_string())
            })
        }
    }

    /// Resolve a `cpal::Device` for output playback by name or default.
    pub fn find_output_cpal_device(name: Option<&str>) -> Result<cpal::Device, AudioError> {
        let host = cpal::default_host();
        if let Some(target_name) = name {
            let devices = host.output_devices().map_err(|e| {
                AudioError::BackendError(format!("Failed to list output devices: {e}"))
            })?;
            for dev in devices {
                if let Ok(dev_name) = dev.name() {
                    if dev_name == target_name {
                        return Ok(dev);
                    }
                }
            }
            Err(AudioError::DeviceNotFound(target_name.to_string()))
        } else {
            host.default_output_device().ok_or_else(|| {
                AudioError::DeviceNotFound("No default output device found".to_string())
            })
        }
    }
}
