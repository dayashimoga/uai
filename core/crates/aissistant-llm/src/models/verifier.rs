use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

use sha2::{Digest, Sha256};

use crate::error::{LlmError, Result};

/// Incremental and file-based SHA-256 cryptographic integrity verifier.
#[derive(Debug, Clone, Default)]
pub struct Sha256Verifier {
    hasher: Sha256,
    bytes_hashed: u64,
}

impl Sha256Verifier {
    /// Creates a fresh streaming SHA-256 verifier.
    pub fn new() -> Self {
        Self {
            hasher: Sha256::new(),
            bytes_hashed: 0,
        }
    }

    /// Ingests a slice of bytes into the running hash calculation.
    pub fn update(&mut self, data: &[u8]) {
        self.hasher.update(data);
        self.bytes_hashed += data.len() as u64;
    }

    /// Total bytes hashed so far.
    pub fn bytes_hashed(&self) -> u64 {
        self.bytes_hashed
    }

    /// Finalizes the hash and returns the 64-character lowercase hexadecimal digest.
    pub fn finalize_hex(self) -> String {
        let result = self.hasher.finalize();
        format!("{:x}", result)
    }

    /// Verifies if the accumulated bytes match an expected 64-character hex string.
    pub fn verify(self, expected_hex: &str) -> bool {
        let computed = self.finalize_hex();
        computed.eq_ignore_ascii_case(expected_hex.trim())
    }

    /// Computes the SHA-256 hex digest of a file using buffered streaming.
    ///
    /// Reads in 64 KB chunks to maintain minimal memory footprint on multi-GB model files.
    pub fn compute_file_hash<P: AsRef<Path>>(path: P) -> Result<String> {
        let file = File::open(path)?;
        let mut reader = BufReader::with_capacity(64 * 1024, file);
        let mut verifier = Self::new();
        let mut buffer = [0u8; 64 * 1024];

        loop {
            let bytes_read = reader.read(&mut buffer)?;
            if bytes_read == 0 {
                break;
            }
            verifier.update(&buffer[..bytes_read]);
        }

        Ok(verifier.finalize_hex())
    }

    /// Verifies a file against an expected SHA-256 hash. Returns `Ok(true)` if valid,
    /// or returns `Err(LlmError::ChecksumMismatch)` if corrupted or tampered.
    pub fn verify_file<P: AsRef<Path>>(
        path: P,
        model_id: &str,
        expected_hex: &str,
    ) -> Result<bool> {
        let computed = Self::compute_file_hash(&path)?;
        let expected_trimmed = expected_hex.trim().to_ascii_lowercase();

        if computed == expected_trimmed {
            Ok(true)
        } else {
            Err(LlmError::ChecksumMismatch {
                model_id: model_id.to_string(),
                expected: expected_trimmed,
                actual: computed,
            })
        }
    }
}
