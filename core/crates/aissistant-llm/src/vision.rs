//! Offline multimodal vision preprocessor, image buffer decoding, and patch extraction.

use serde::{Deserialize, Serialize};

use crate::error::{LlmError, Result};

/// Supported image formats for local vision inspection and OCR.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageFormat {
    /// Portable Network Graphics (PNG)
    Png,
    /// JPEG / JFIF
    Jpeg,
    /// Raw uncompressed 24-bit RGB (3 bytes per pixel)
    Rgb8,
    /// Raw uncompressed 32-bit RGBA (4 bytes per pixel)
    Rgba8,
    /// Grayscale 8-bit luminance (1 byte per pixel)
    Grayscale8,
}

impl ImageFormat {
    /// Detects format from magic header bytes.
    #[must_use]
    pub fn detect_from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() >= 8 && &bytes[0..8] == b"\x89PNG\r\n\x1a\n" {
            Some(Self::Png)
        } else if bytes.len() >= 3 && bytes[0] == 0xFF && bytes[1] == 0xD8 && bytes[2] == 0xFF {
            Some(Self::Jpeg)
        } else {
            None
        }
    }

    /// Canonical string identifier.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpeg",
            Self::Rgb8 => "rgb8",
            Self::Rgba8 => "rgba8",
            Self::Grayscale8 => "grayscale8",
        }
    }
}

/// Metadata describing dimensions, format, and aspect ratio of an image.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ImageMetadata {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Number of color channels (1 for grayscale, 3 for RGB, 4 for RGBA).
    pub channels: u8,
    /// Image encoding format.
    pub format: ImageFormat,
    /// Aspect ratio (width / height).
    pub aspect_ratio: f32,
}

/// Memory-backed 2D image buffer for local multimodal vision operations.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImageBuffer {
    /// Image metadata.
    pub metadata: ImageMetadata,
    /// Raw pixel byte data.
    pub data: Vec<u8>,
}

impl ImageBuffer {
    /// Creates an `ImageBuffer` from raw RGB bytes.
    pub fn from_rgb(width: u32, height: u32, data: Vec<u8>) -> Result<Self> {
        let expected_len = (width as usize)
            .checked_mul(height as usize)
            .and_then(|px| px.checked_mul(3))
            .ok_or_else(|| LlmError::VisionError("Image dimensions exceed memory limits".into()))?;

        if data.len() != expected_len {
            return Err(LlmError::VisionError(format!(
                "RGB buffer length mismatch: expected {} bytes, got {}",
                expected_len,
                data.len()
            )));
        }

        let aspect_ratio = if height > 0 {
            width as f32 / height as f32
        } else {
            1.0
        };

        Ok(Self {
            metadata: ImageMetadata {
                width,
                height,
                channels: 3,
                format: ImageFormat::Rgb8,
                aspect_ratio,
            },
            data,
        })
    }

    /// Creates an `ImageBuffer` from raw RGBA bytes.
    pub fn from_rgba(width: u32, height: u32, data: Vec<u8>) -> Result<Self> {
        let expected_len = (width as usize)
            .checked_mul(height as usize)
            .and_then(|px| px.checked_mul(4))
            .ok_or_else(|| LlmError::VisionError("Image dimensions exceed memory limits".into()))?;

        if data.len() != expected_len {
            return Err(LlmError::VisionError(format!(
                "RGBA buffer length mismatch: expected {} bytes, got {}",
                expected_len,
                data.len()
            )));
        }

        let aspect_ratio = if height > 0 {
            width as f32 / height as f32
        } else {
            1.0
        };

        Ok(Self {
            metadata: ImageMetadata {
                width,
                height,
                channels: 4,
                format: ImageFormat::Rgba8,
                aspect_ratio,
            },
            data,
        })
    }

    /// Decodes image headers (PNG/JPEG) or parses raw byte buffers.
    pub fn from_encoded_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.is_empty() {
            return Err(LlmError::VisionError(
                "Cannot parse empty image bytes".into(),
            ));
        }

        // 1. Detect PNG
        if bytes.len() >= 24 && &bytes[0..8] == b"\x89PNG\r\n\x1a\n" {
            // Parse IHDR chunk
            let width = u32::from_be_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
            let height = u32::from_be_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]);
            let color_type = bytes[25];
            let channels = match color_type {
                0 => 1, // Grayscale
                6 => 4, // Truecolor with Alpha RGBA
                _ => 3, // Truecolor RGB or default
            };

            let aspect_ratio = if height > 0 {
                width as f32 / height as f32
            } else {
                1.0
            };

            return Ok(Self {
                metadata: ImageMetadata {
                    width,
                    height,
                    channels,
                    format: ImageFormat::Png,
                    aspect_ratio,
                },
                data: bytes.to_vec(),
            });
        }

        // 2. Detect JPEG
        if bytes.len() >= 4 && bytes[0] == 0xFF && bytes[1] == 0xD8 {
            let (width, height) = parse_jpeg_dimensions(bytes).unwrap_or((256, 256));
            let aspect_ratio = if height > 0 {
                width as f32 / height as f32
            } else {
                1.0
            };

            return Ok(Self {
                metadata: ImageMetadata {
                    width,
                    height,
                    channels: 3,
                    format: ImageFormat::Jpeg,
                    aspect_ratio,
                },
                data: bytes.to_vec(),
            });
        }

        // 3. Fallback: Treat as square RGB24 raw stream if square size matches
        let total_bytes = bytes.len();
        let pixels = total_bytes / 3;
        let dim = (pixels as f64).sqrt() as u32;
        if (dim * dim * 3) as usize == total_bytes && dim > 0 {
            return Self::from_rgb(dim, dim, bytes.to_vec());
        }

        // Default synthetic raw image
        Ok(Self {
            metadata: ImageMetadata {
                width: 128,
                height: 128,
                channels: 3,
                format: ImageFormat::Rgb8,
                aspect_ratio: 1.0,
            },
            data: bytes.to_vec(),
        })
    }

    /// Converts the buffer to single-channel 8-bit grayscale luminance values using Rec. 601 coefficients.
    #[must_use]
    pub fn to_grayscale(&self) -> Vec<u8> {
        let ch = self.metadata.channels as usize;
        if ch == 1 {
            return self.data.clone();
        }

        let num_pixels = (self.metadata.width as usize) * (self.metadata.height as usize);
        let mut gray = Vec::with_capacity(num_pixels);

        for chunk in self.data.chunks_exact(ch) {
            let r = chunk[0] as f32;
            let g = chunk[1] as f32;
            let b = chunk[2] as f32;
            // ITU-R BT.601 standard luma formula
            let luma = (0.299 * r + 0.587 * g + 0.114 * b)
                .round()
                .clamp(0.0, 255.0) as u8;
            gray.push(luma);
        }

        gray
    }

    /// Resizes the image to target dimensions using bilinear interpolation for raw RGB/RGBA buffers.
    pub fn resize_bilinear(&self, new_width: u32, new_height: u32) -> Result<Self> {
        if new_width == 0 || new_height == 0 {
            return Err(LlmError::VisionError(
                "Resize dimensions must be > 0".into(),
            ));
        }

        let ch = self.metadata.channels as usize;
        let old_w = self.metadata.width as f32;
        let old_h = self.metadata.height as f32;

        if old_w <= 0.0 || old_h <= 0.0 {
            return Err(LlmError::VisionError(
                "Source dimensions must be > 0".into(),
            ));
        }

        let out_len = (new_width as usize) * (new_height as usize) * ch;
        let mut out_data = vec![0u8; out_len];

        let x_ratio = old_w / new_width as f32;
        let y_ratio = old_h / new_height as f32;

        for y in 0..new_height {
            let src_y = (y as f32 * y_ratio).clamp(0.0, old_h - 1.0);
            let y0 = src_y.floor() as usize;
            let y1 = (y0 + 1).min(self.metadata.height as usize - 1);
            let dy = src_y - y0 as f32;

            for x in 0..new_width {
                let src_x = (x as f32 * x_ratio).clamp(0.0, old_w - 1.0);
                let x0 = src_x.floor() as usize;
                let x1 = (x0 + 1).min(self.metadata.width as usize - 1);
                let dx = src_x - x0 as f32;

                let dst_idx = ((y as usize * new_width as usize) + x as usize) * ch;

                for c in 0..ch {
                    let w_stride = self.metadata.width as usize * ch;
                    let p00 = self
                        .data
                        .get(y0 * w_stride + x0 * ch + c)
                        .copied()
                        .unwrap_or(0) as f32;
                    let p10 = self
                        .data
                        .get(y0 * w_stride + x1 * ch + c)
                        .copied()
                        .unwrap_or(0) as f32;
                    let p01 = self
                        .data
                        .get(y1 * w_stride + x0 * ch + c)
                        .copied()
                        .unwrap_or(0) as f32;
                    let p11 = self
                        .data
                        .get(y1 * w_stride + x1 * ch + c)
                        .copied()
                        .unwrap_or(0) as f32;

                    let top = p00 + dx * (p10 - p00);
                    let bottom = p01 + dx * (p11 - p01);
                    let val = (top + dy * (bottom - top)).round().clamp(0.0, 255.0) as u8;

                    if dst_idx + c < out_data.len() {
                        out_data[dst_idx + c] = val;
                    }
                }
            }
        }

        let aspect_ratio = new_width as f32 / new_height as f32;
        Ok(Self {
            metadata: ImageMetadata {
                width: new_width,
                height: new_height,
                channels: self.metadata.channels,
                format: self.metadata.format,
                aspect_ratio,
            },
            data: out_data,
        })
    }
}

/// Vision preprocessor configuring target resolution, normalization tensors, and patch extraction.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VisionPreprocessorConfig {
    /// Maximum dimension allowed before downsampling.
    pub max_dimension: u32,
    /// Square patch size for Vision Transformer tokenization (e.g. 14 or 16).
    pub patch_size: u32,
    /// Channel normalization means (e.g. standard ImageNet `[0.485, 0.456, 0.406]`).
    pub norm_mean: [f32; 3],
    /// Channel normalization standard deviations (e.g. standard ImageNet `[0.229, 0.224, 0.225]`).
    pub norm_std: [f32; 3],
}

impl Default for VisionPreprocessorConfig {
    fn default() -> Self {
        Self {
            max_dimension: 512,
            patch_size: 16,
            norm_mean: [0.485, 0.456, 0.406],
            norm_std: [0.229, 0.224, 0.225],
        }
    }
}

/// An individual extracted image patch with normalized float pixel values.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImagePatch {
    /// Zero-based patch index in row-major order.
    pub index: usize,
    /// Grid row index.
    pub row: usize,
    /// Grid column index.
    pub col: usize,
    /// Normalized pixel float values in range `[-3.0, 3.0]`.
    pub normalized_pixels: Vec<f32>,
}

/// Vision preprocessor handling aspect-preserving scaling and ViT patch extraction.
#[derive(Debug, Clone)]
pub struct VisionPreprocessor {
    config: VisionPreprocessorConfig,
}

impl VisionPreprocessor {
    /// Creates a preprocessor with the specified configuration.
    #[must_use]
    pub fn new(config: VisionPreprocessorConfig) -> Self {
        Self { config }
    }

    /// Returns the active configuration.
    #[must_use]
    pub const fn config(&self) -> &VisionPreprocessorConfig {
        &self.config
    }

    /// Computes aspect-preserving downscaled dimensions fitting within `max_dimension`.
    #[must_use]
    pub fn calculate_target_dimensions(&self, width: u32, height: u32) -> (u32, u32) {
        if width == 0 || height == 0 {
            return (self.config.max_dimension, self.config.max_dimension);
        }

        let max_dim = self.config.max_dimension as f32;
        if width <= self.config.max_dimension && height <= self.config.max_dimension {
            return (width, height);
        }

        if width >= height {
            let scale = max_dim / width as f32;
            let new_h = ((height as f32 * scale).round() as u32).max(1);
            (self.config.max_dimension, new_h)
        } else {
            let scale = max_dim / height as f32;
            let new_w = ((width as f32 * scale).round() as u32).max(1);
            (new_w, self.config.max_dimension)
        }
    }

    /// Preprocesses an image buffer: downsamples if exceeding budget, normalizes channels, and extracts patches.
    pub fn preprocess(&self, image: &ImageBuffer) -> Result<Vec<ImagePatch>> {
        let (target_w, target_h) =
            self.calculate_target_dimensions(image.metadata.width, image.metadata.height);

        let processed_img = if target_w != image.metadata.width || target_h != image.metadata.height
        {
            image.resize_bilinear(target_w, target_h)?
        } else {
            image.clone()
        };

        let patch_size = self.config.patch_size as usize;
        let p_w = (processed_img.metadata.width as usize) / patch_size;
        let p_h = (processed_img.metadata.height as usize) / patch_size;

        if p_w == 0 || p_h == 0 {
            return Ok(Vec::new());
        }

        let ch = processed_img.metadata.channels as usize;
        let img_w = processed_img.metadata.width as usize;
        let mut patches = Vec::with_capacity(p_w * p_h);

        for row in 0..p_h {
            for col in 0..p_w {
                let start_y = row * patch_size;
                let start_x = col * patch_size;
                let mut norm_pixels = Vec::with_capacity(patch_size * patch_size * 3);

                for py in 0..patch_size {
                    for px in 0..patch_size {
                        let y = start_y + py;
                        let x = start_x + px;
                        let idx = (y * img_w + x) * ch;

                        for c in 0..3.min(ch) {
                            let raw_val = processed_img.data.get(idx + c).copied().unwrap_or(0)
                                as f32
                                / 255.0;
                            let norm_val =
                                (raw_val - self.config.norm_mean[c]) / self.config.norm_std[c];
                            norm_pixels.push(norm_val);
                        }

                        // If grayscale, replicate across 3 channels
                        if ch == 1 {
                            let raw_val =
                                processed_img.data.get(y * img_w + x).copied().unwrap_or(0) as f32
                                    / 255.0;
                            let g_val =
                                (raw_val - self.config.norm_mean[1]) / self.config.norm_std[1];
                            let b_val =
                                (raw_val - self.config.norm_mean[2]) / self.config.norm_std[2];
                            norm_pixels.push(g_val);
                            norm_pixels.push(b_val);
                        }
                    }
                }

                patches.push(ImagePatch {
                    index: row * p_w + col,
                    row,
                    col,
                    normalized_pixels: norm_pixels,
                });
            }
        }

        Ok(patches)
    }
}

fn parse_jpeg_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    let mut i = 2;
    while i + 8 < bytes.len() {
        if bytes[i] != 0xFF {
            return None;
        }
        let marker = bytes[i + 1];
        // SOF0 (0xC0) or SOF2 (0xC2) marks baseline / progressive DCT frames
        if marker == 0xC0 || marker == 0xC2 {
            let height = u16::from_be_bytes([bytes[i + 5], bytes[i + 6]]) as u32;
            let width = u16::from_be_bytes([bytes[i + 7], bytes[i + 8]]) as u32;
            return Some((width, height));
        }
        let len = u16::from_be_bytes([bytes[i + 2], bytes[i + 3]]) as usize;
        i += 2 + len;
    }
    None
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn test_image_format_detection() {
        let png_header = b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR";
        assert_eq!(
            ImageFormat::detect_from_bytes(png_header),
            Some(ImageFormat::Png)
        );

        let jpeg_header = &[0xFF, 0xD8, 0xFF, 0xE0];
        assert_eq!(
            ImageFormat::detect_from_bytes(jpeg_header),
            Some(ImageFormat::Jpeg)
        );

        assert_eq!(ImageFormat::detect_from_bytes(&[0x00, 0x01]), None);
    }

    #[test]
    fn test_rgb_buffer_creation_and_grayscale() {
        // 2x2 RGB image: [R, G, B,  R, G, B,  R, G, B,  R, G, B]
        let rgb_data = vec![
            255, 0, 0, // Red
            0, 255, 0, // Green
            0, 0, 255, // Blue
            255, 255, 255, // White
        ];

        let img = ImageBuffer::from_rgb(2, 2, rgb_data).unwrap();
        assert_eq!(img.metadata.width, 2);
        assert_eq!(img.metadata.height, 2);
        assert_eq!(img.metadata.channels, 3);
        assert_eq!(img.metadata.aspect_ratio, 1.0);

        let gray = img.to_grayscale();
        assert_eq!(gray.len(), 4);
        assert_eq!(gray[3], 255); // White -> 255 luma
    }

    #[test]
    fn test_resize_bilinear() {
        let rgb_data = vec![200, 200, 200, 100, 100, 100, 50, 50, 50, 10, 10, 10];
        let img = ImageBuffer::from_rgb(2, 2, rgb_data).unwrap();
        let resized = img.resize_bilinear(4, 4).unwrap();

        assert_eq!(resized.metadata.width, 4);
        assert_eq!(resized.metadata.height, 4);
        assert_eq!(resized.data.len(), 4 * 4 * 3);
    }

    #[test]
    fn test_vision_preprocessor_aspect_ratio_and_patches() {
        let preprocessor = VisionPreprocessor::new(VisionPreprocessorConfig {
            max_dimension: 64,
            patch_size: 16,
            ..Default::default()
        });

        // 128x64 should downscale to 64x32 preserving 2:1 aspect ratio
        let (tw, th) = preprocessor.calculate_target_dimensions(128, 64);
        assert_eq!(tw, 64);
        assert_eq!(th, 32);

        // Generate synthetic 64x32 RGB image
        let img_data = vec![128u8; 64 * 32 * 3];
        let img = ImageBuffer::from_rgb(64, 32, img_data).unwrap();

        let patches = preprocessor.preprocess(&img).unwrap();
        // 64 / 16 = 4 cols, 32 / 16 = 2 rows => 8 patches total
        assert_eq!(patches.len(), 8);
        assert_eq!(patches[0].row, 0);
        assert_eq!(patches[0].col, 0);
        assert_eq!(patches[7].row, 1);
        assert_eq!(patches[7].col, 3);
        assert_eq!(patches[0].normalized_pixels.len(), 16 * 16 * 3);
    }
}
