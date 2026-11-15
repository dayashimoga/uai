pub mod catalog;
pub mod downloader;
pub mod gguf_parser;
pub mod verifier;

pub use catalog::{ModelCatalog, ModelDescriptor, QuantizationType};
pub use downloader::{DownloadProgress, DownloadStatus, LocalModelDownloader, ModelDownloader};
pub use gguf_parser::{GgufInspector, GgufMetadata, GgufType, GGUF_MAGIC};
pub use verifier::Sha256Verifier;
