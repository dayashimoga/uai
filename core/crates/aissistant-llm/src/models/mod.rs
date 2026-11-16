pub mod catalog;
pub mod downloader;
pub mod gguf_parser;
pub mod verifier;

pub use catalog::{
    register_custom_gguf, ModelCatalog, ModelDescriptor, ModelLifecycleState, QuantizationType,
};
pub use downloader::{
    query_model_lifecycle, DownloadProgress, DownloadStatus, LocalModelDownloader,
    ModelDownloadManager, ModelDownloader,
};
pub use gguf_parser::{GgufInspector, GgufMetadata, GgufType, GGUF_MAGIC};
pub use verifier::Sha256Verifier;
