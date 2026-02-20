use thiserror::Error;

#[derive(Debug, Error)]
pub enum CodeSearchError {
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Codebase not indexed: {0}")]
    CodebaseNotIndexed(String),

    #[error("Failed to load embedding model: {0}")]
    EmbeddingModelLoad(String),

    #[error("Embedding inference error: {0}")]
    EmbeddingInference(String),

    #[error("Failed to read file: {path}")]
    FileRead { path: String },

    #[error("Invalid configuration: {0}")]
    InvalidConfiguration(String),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Git error: {0}")]
    Git(String),

    #[error("Manifest error: {0}")]
    Manifest(String),
}

pub type Result<T> = std::result::Result<T, CodeSearchError>;
