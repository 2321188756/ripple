//! store 层错误类型。

use ripple_core::RippleError;

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("database error: {0}")]
    Database(String),

    #[error("connection pool error: {0}")]
    Pool(String),

    #[error("migration v{version} failed: {details}")]
    Migration { version: u32, details: String },

    #[error("not found: {0}")]
    NotFound(String),

    #[error("invalid data: {0}")]
    InvalidData(String),
}

impl From<StoreError> for RippleError {
    fn from(e: StoreError) -> Self {
        match e {
            StoreError::NotFound(s) => RippleError::NotFound(s),
            StoreError::InvalidData(s) => RippleError::InvalidInput(s),
            _ => RippleError::Database(e.to_string()),
        }
    }
}

pub type StoreResult<T> = Result<T, StoreError>;
