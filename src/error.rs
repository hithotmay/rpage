//! Error types for rpage

use thiserror::Error;

/// Unified error type for all rpage operations
#[derive(Error, Debug)]
pub enum Error {
    /// Browser launch or CDP connection failure
    #[error("Browser error: {0}")]
    Browser(String),

    /// Network / HTTP request failure
    #[error("Network error: {0}")]
    Network(String),

    /// Element not found on the page
    #[error("Element not found: {0}")]
    ElementNotFound(String),

    /// Invalid locator string
    #[error("Invalid locator: {0}")]
    InvalidLocator(String),

    /// Operation timed out
    #[error("Timeout: {0}")]
    Timeout(String),

    /// Cookie synchronization failure
    #[error("Cookie sync error: {0}")]
    CookieSync(String),

    /// Configuration error
    #[error("Config error: {0}")]
    Config(String),

    /// I/O error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// reqwest HTTP error
    #[error("HTTP error: {0}")]
    Reqwest(#[from] reqwest::Error),

    /// URL parsing error
    #[error("URL error: {0}")]
    Url(#[from] url::ParseError),

    /// JSON serialization / deserialization error
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// CDP error from chromiumoxide
    #[error("CDP error: {0}")]
    Cdp(String),

    /// Generic error from anyhow
    #[error("{0}")]
    Other(#[from] anyhow::Error),
}

/// Convenience result alias
pub type Result<T> = std::result::Result<T, Error>;

impl From<chromiumoxide::error::CdpError> for Error {
    fn from(e: chromiumoxide::error::CdpError) -> Self {
        Error::Cdp(e.to_string())
    }
}
