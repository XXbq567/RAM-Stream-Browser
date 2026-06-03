/// Unified error types for RAM-Stream-Browser.
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Cookie extraction failed: {0}")]
    CookieExtraction(String),

    #[error("Stream extraction failed: {0}")]
    StreamExtraction(String),

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("IPC error: {0}")]
    Ipc(#[from] serde_json::Error),
}

impl From<AppError> for String {
    fn from(e: AppError) -> Self {
        e.to_string()
    }
}
