use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("Storage error: {0}")]
    Storage(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Not found: {0}")]
    #[allow(dead_code)]
    NotFound(String),
}
