use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Path;

use byteorder::{LittleEndian, ReadBytesExt};
use serde::{Deserialize, Serialize};

use crate::error::{LlmError, Result};

pub const GGUF_MAGIC: [u8; 4] = *b"GGUF";
pub const MAX_KEY_LENGTH: u64 = 1024;
pub const MAX_STRING_LENGTH: u64 = 65536;
pub const MAX_ARRAY_LENGTH: u64 = 65536;

/// GGUF value types according to the specification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum GgufType {
    Uint8 = 0,
    Int8 = 1,
    Uint16 = 2,
    Int16 = 3,
    Uint32 = 4,
    Int32 = 5,
    Float32 = 6,
    Bool = 7,
    String = 8,
    Array = 9,
    Uint64 = 10,
    Int64 = 11,
    Float64 = 12,
}

impl GgufType {
    pub fn from_u32(val: u32) -> Option<Self> {
        match val {
            0 => Some(Self::Uint8),
            1 => Some(Self::Int8),
            2 => Some(Self::Uint16),
            3 => Some(Self::Int16),
            4 => Some(Self::Uint32),
            5 => Some(Self::Int32),
            6 => Some(Self::Float32),
            7 => Some(Self::Bool),
            8 => Some(Self::String),
            9 => Some(Self::Array),
            10 => Some(Self::Uint64),
            11 => Some(Self::Int64),
            12 => Some(Self::Float64),
            _ => None,
        }
    }
}

/// Parsed structural metadata extracted from a GGUF binary header.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GgufMetadata {
    pub version: u32,
    pub tensor_count: u64,
    pub kv_count: u64,
    pub architecture: Option<String>,
    pub model_name: Option<String>,
    pub context_length: Option<u64>,
    pub embedding_length: Option<u64>,
    pub block_count: Option<u64>,
    pub head_count: Option<u64>,
    pub head_count_kv: Option<u64>,
    pub file_size_bytes: u64,
    pub raw_properties: HashMap<String, String>,
}

/// Binary inspector for GGUF model containers.
#[derive(Debug, Default, Clone)]
pub struct GgufInspector;

impl GgufInspector {
    /// Inspects and parses metadata from a GGUF file on disk.
    pub fn inspect_file<P: AsRef<Path>>(path: P) -> Result<GgufMetadata> {
        let file = File::open(&path)?;
        let file_size_bytes = file.metadata()?.len();
        let mut reader = BufReader::new(file);

        Self::inspect_reader(&mut reader, file_size_bytes)
    }

    /// Inspects and parses metadata from an in-memory byte slice.
    pub fn inspect_bytes(bytes: &[u8]) -> Result<GgufMetadata> {
        let mut cursor = std::io::Cursor::new(bytes);
        Self::inspect_reader(&mut cursor, bytes.len() as u64)
    }

    fn inspect_reader<R: Read + Seek>(
        reader: &mut R,
        file_size_bytes: u64,
    ) -> Result<GgufMetadata> {
        // 1. Validate 4-byte GGUF magic header
        let mut magic = [0u8; 4];
        reader.read_exact(&mut magic).map_err(|e| {
            LlmError::GgufParseError(format!("Truncated file; unable to read magic header: {e}"))
        })?;

        if magic != GGUF_MAGIC {
            return Err(LlmError::GgufParseError(format!(
                "Invalid GGUF magic bytes: expected {:?}, got {:?}",
                GGUF_MAGIC, magic
            )));
        }

        // 2. Read GGUF version (support v2 and v3)
        let version = reader
            .read_u32::<LittleEndian>()
            .map_err(|e| LlmError::GgufParseError(format!("Failed to read GGUF version: {e}")))?;

        if !(2..=3).contains(&version) {
            return Err(LlmError::GgufParseError(format!(
                "Unsupported GGUF version {version}; engine supports v2 and v3"
            )));
        }

        // 3. Read tensor count and metadata key-value count
        let tensor_count = reader
            .read_u64::<LittleEndian>()
            .map_err(|e| LlmError::GgufParseError(format!("Failed to read tensor count: {e}")))?;

        let kv_count = reader.read_u64::<LittleEndian>().map_err(|e| {
            LlmError::GgufParseError(format!("Failed to read metadata key-value count: {e}"))
        })?;

        if kv_count > 100_000 {
            return Err(LlmError::GgufParseError(format!(
                "Suspiciously high KV metadata count: {kv_count}"
            )));
        }

        // 4. Iterate over key-value metadata pairs
        let mut raw_properties = HashMap::new();

        for i in 0..kv_count {
            let key = Self::read_string(reader).map_err(|e| {
                LlmError::GgufParseError(format!("Failed reading metadata key #{i}: {e}"))
            })?;

            let type_val = reader.read_u32::<LittleEndian>().map_err(|e| {
                LlmError::GgufParseError(format!("Failed reading type for key '{key}': {e}"))
            })?;

            let val_type = GgufType::from_u32(type_val).ok_or_else(|| {
                LlmError::GgufParseError(format!("Unknown GGUF type ID {type_val} for key '{key}'"))
            })?;

            let value_str = Self::read_value(reader, val_type).map_err(|e| {
                LlmError::GgufParseError(format!("Failed reading value for key '{key}': {e}"))
            })?;

            raw_properties.insert(key, value_str);
        }

        // 5. Extract core architectural parameters
        let architecture = raw_properties.get("general.architecture").cloned();
        let model_name = raw_properties
            .get("general.name")
            .cloned()
            .or_else(|| architecture.clone());

        let arch_prefix = architecture.as_deref().unwrap_or("llama");

        let context_length = Self::lookup_u64(&raw_properties, arch_prefix, "context_length");
        let embedding_length = Self::lookup_u64(&raw_properties, arch_prefix, "embedding_length");
        let block_count = Self::lookup_u64(&raw_properties, arch_prefix, "block_count");
        let head_count = Self::lookup_u64(&raw_properties, arch_prefix, "attention.head_count");
        let head_count_kv =
            Self::lookup_u64(&raw_properties, arch_prefix, "attention.head_count_kv")
                .or(head_count);

        Ok(GgufMetadata {
            version,
            tensor_count,
            kv_count,
            architecture,
            model_name,
            context_length,
            embedding_length,
            block_count,
            head_count,
            head_count_kv,
            file_size_bytes,
            raw_properties,
        })
    }

    fn lookup_u64(map: &HashMap<String, String>, arch: &str, field: &str) -> Option<u64> {
        let arch_key = format!("{arch}.{field}");
        if let Some(val) = map.get(&arch_key) {
            return val.parse::<u64>().ok();
        }
        let general_key = format!("general.{field}");
        if let Some(val) = map.get(&general_key) {
            return val.parse::<u64>().ok();
        }
        None
    }

    fn read_string<R: Read>(reader: &mut R) -> Result<String> {
        let len = reader
            .read_u64::<LittleEndian>()
            .map_err(|e| LlmError::GgufParseError(format!("Failed to read string length: {e}")))?;

        if len > MAX_STRING_LENGTH {
            return Err(LlmError::GgufParseError(format!(
                "String length ({len}) exceeds security limit ({MAX_STRING_LENGTH})"
            )));
        }

        let mut buf = vec![0u8; len as usize];
        reader.read_exact(&mut buf).map_err(|e| {
            LlmError::GgufParseError(format!("Failed to read {len} string bytes: {e}"))
        })?;

        String::from_utf8(buf).map_err(|e| {
            LlmError::GgufParseError(format!("GGUF string contains invalid UTF-8: {e}"))
        })
    }

    fn read_value<R: Read + Seek>(reader: &mut R, val_type: GgufType) -> Result<String> {
        match val_type {
            GgufType::Uint8 => {
                let v = reader.read_u8()?;
                Ok(v.to_string())
            }
            GgufType::Int8 => {
                let v = reader.read_i8()?;
                Ok(v.to_string())
            }
            GgufType::Uint16 => {
                let v = reader.read_u16::<LittleEndian>()?;
                Ok(v.to_string())
            }
            GgufType::Int16 => {
                let v = reader.read_i16::<LittleEndian>()?;
                Ok(v.to_string())
            }
            GgufType::Uint32 => {
                let v = reader.read_u32::<LittleEndian>()?;
                Ok(v.to_string())
            }
            GgufType::Int32 => {
                let v = reader.read_i32::<LittleEndian>()?;
                Ok(v.to_string())
            }
            GgufType::Float32 => {
                let v = reader.read_f32::<LittleEndian>()?;
                Ok(v.to_string())
            }
            GgufType::Bool => {
                let b = reader.read_u8()?;
                Ok((b != 0).to_string())
            }
            GgufType::String => Self::read_string(reader),
            GgufType::Uint64 => {
                let v = reader.read_u64::<LittleEndian>()?;
                Ok(v.to_string())
            }
            GgufType::Int64 => {
                let v = reader.read_i64::<LittleEndian>()?;
                Ok(v.to_string())
            }
            GgufType::Float64 => {
                let v = reader.read_f64::<LittleEndian>()?;
                Ok(v.to_string())
            }
            GgufType::Array => {
                let elem_type_id = reader.read_u32::<LittleEndian>()?;
                let elem_type = GgufType::from_u32(elem_type_id).ok_or_else(|| {
                    LlmError::GgufParseError(format!("Invalid array element type {elem_type_id}"))
                })?;

                let count = reader.read_u64::<LittleEndian>()?;
                if count > MAX_ARRAY_LENGTH {
                    return Err(LlmError::GgufParseError(format!(
                        "Array count {count} exceeds safety limit"
                    )));
                }

                // Skip or read array elements
                let mut elements = Vec::with_capacity(count.min(16) as usize);
                for i in 0..count {
                    if i < 16 {
                        elements.push(Self::read_value(reader, elem_type)?);
                    } else {
                        // Skip remaining to prevent large memory spikes on tokenizer arrays
                        Self::skip_value(reader, elem_type)?;
                    }
                }
                if count > 16 {
                    elements.push(format!("... ({} more elements)", count - 16));
                }
                Ok(format!("[{}]", elements.join(", ")))
            }
        }
    }

    fn skip_value<R: Read + Seek>(reader: &mut R, val_type: GgufType) -> Result<()> {
        match val_type {
            GgufType::Uint8 | GgufType::Int8 | GgufType::Bool => {
                reader.seek(SeekFrom::Current(1))?;
            }
            GgufType::Uint16 | GgufType::Int16 => {
                reader.seek(SeekFrom::Current(2))?;
            }
            GgufType::Uint32 | GgufType::Int32 | GgufType::Float32 => {
                reader.seek(SeekFrom::Current(4))?;
            }
            GgufType::Uint64 | GgufType::Int64 | GgufType::Float64 => {
                reader.seek(SeekFrom::Current(8))?;
            }
            GgufType::String => {
                let len = reader.read_u64::<LittleEndian>()?;
                reader.seek(SeekFrom::Current(len as i64))?;
            }
            GgufType::Array => {
                let elem_type_id = reader.read_u32::<LittleEndian>()?;
                let elem_type = GgufType::from_u32(elem_type_id).ok_or_else(|| {
                    LlmError::GgufParseError(format!("Invalid array element type {elem_type_id}"))
                })?;
                let count = reader.read_u64::<LittleEndian>()?;
                for _ in 0..count {
                    Self::skip_value(reader, elem_type)?;
                }
            }
        }
        Ok(())
    }
}
