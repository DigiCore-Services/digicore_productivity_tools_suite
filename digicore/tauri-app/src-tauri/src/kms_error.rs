use thiserror::Error;

#[derive(Error, Debug)]
#[allow(dead_code)]
pub enum KmsError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("KMS Repository not initialized")]
    NotInitialized,

    #[error("Config error: {0}")]
    Config(String),

    #[error("Path error: {0}")]
    Path(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Note not found: {0}")]
    NotFound(String),

    #[error("Embedding error: {0}")]
    Embedding(String),

    #[error("Security error: {0}")]
    Security(String),

    #[error("Operation failed: {0}")]
    General(String),
}

impl From<String> for KmsError {
    fn from(s: String) -> Self {
        KmsError::General(s)
    }
}

pub type KmsResult<T> = Result<T, KmsError>;
