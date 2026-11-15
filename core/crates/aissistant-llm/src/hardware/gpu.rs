use serde::{Deserialize, Serialize};

/// Supported GPU hardware backends for tensor acceleration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GpuBackend {
    Cuda,
    Metal,
    Vulkan,
    CpuOnly,
}

impl GpuBackend {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Cuda => "CUDA",
            Self::Metal => "Metal",
            Self::Vulkan => "Vulkan",
            Self::CpuOnly => "CPU",
        }
    }
}

impl std::fmt::Display for GpuBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Description of an enumerated GPU graphics/compute adapter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GpuDevice {
    pub name: String,
    pub backend: GpuBackend,
    pub total_vram_bytes: Option<u64>,
    pub is_integrated: bool,
}

/// Aggregated GPU acceleration capabilities of the host system.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GpuProfile {
    pub primary_device: Option<GpuDevice>,
    pub available_devices: Vec<GpuDevice>,
    pub selected_backend: GpuBackend,
    pub supports_gpu_offload: bool,
}

impl GpuProfile {
    /// Detects GPU accelerators on the current platform.
    pub fn detect() -> Self {
        #[cfg(any(target_os = "macos", target_os = "ios"))]
        {
            // Apple Silicon / Metal unified memory architecture
            let device = GpuDevice {
                name: "Apple Integrated GPU (Metal)".to_string(),
                backend: GpuBackend::Metal,
                total_vram_bytes: None, // Unified memory architecture
                is_integrated: true,
            };
            Self {
                primary_device: Some(device.clone()),
                available_devices: vec![device],
                selected_backend: GpuBackend::Metal,
                supports_gpu_offload: true,
            }
        }

        #[cfg(target_os = "windows")]
        {
            Self::detect_windows()
        }

        #[cfg(target_os = "linux")]
        {
            Self::detect_linux()
        }

        #[cfg(not(any(
            target_os = "macos",
            target_os = "ios",
            target_os = "windows",
            target_os = "linux"
        )))]
        {
            Self::cpu_only()
        }
    }

    /// Constructs a CPU-only fallback profile.
    pub fn cpu_only() -> Self {
        Self {
            primary_device: None,
            available_devices: Vec::new(),
            selected_backend: GpuBackend::CpuOnly,
            supports_gpu_offload: false,
        }
    }

    /// Constructs a simulated GPU profile for testing.
    pub fn from_device(device: GpuDevice) -> Self {
        let backend = device.backend;
        Self {
            primary_device: Some(device.clone()),
            available_devices: vec![device],
            selected_backend: backend,
            supports_gpu_offload: backend != GpuBackend::CpuOnly,
        }
    }

    #[cfg(target_os = "windows")]
    fn detect_windows() -> Self {
        // Probe for Vulkan library or NVIDIA CUDA runtime environment
        let has_vulkan = std::path::Path::new("C:\\Windows\\System32\\vulkan-1.dll").exists();
        let has_cuda = std::env::var("CUDA_PATH").is_ok();

        if has_cuda {
            let dev = GpuDevice {
                name: "NVIDIA CUDA Accelerator".to_string(),
                backend: GpuBackend::Cuda,
                total_vram_bytes: None,
                is_integrated: false,
            };
            Self::from_device(dev)
        } else if has_vulkan {
            let dev = GpuDevice {
                name: "Vulkan Hardware Device".to_string(),
                backend: GpuBackend::Vulkan,
                total_vram_bytes: None,
                is_integrated: false,
            };
            Self::from_device(dev)
        } else {
            Self::cpu_only()
        }
    }

    #[cfg(target_os = "linux")]
    fn detect_linux() -> Self {
        let has_vulkan = std::path::Path::new("/usr/lib/libvulkan.so.1").exists()
            || std::path::Path::new("/usr/lib/x86_64-linux-gnu/libvulkan.so.1").exists()
            || std::path::Path::new("/usr/lib64/libvulkan.so.1").exists();

        let has_cuda =
            std::path::Path::new("/usr/local/cuda").exists() || std::env::var("CUDA_HOME").is_ok();

        if has_cuda {
            let dev = GpuDevice {
                name: "NVIDIA CUDA Device".to_string(),
                backend: GpuBackend::Cuda,
                total_vram_bytes: None,
                is_integrated: false,
            };
            Self::from_device(dev)
        } else if has_vulkan {
            let dev = GpuDevice {
                name: "Vulkan Compute Device".to_string(),
                backend: GpuBackend::Vulkan,
                total_vram_bytes: None,
                is_integrated: false,
            };
            Self::from_device(dev)
        } else {
            Self::cpu_only()
        }
    }
}
