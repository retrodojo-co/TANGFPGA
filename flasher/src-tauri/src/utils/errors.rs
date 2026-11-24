use serde::Serialize;
#[derive(Debug, thiserror::Error, Serialize)]
#[serde(tag = "code", content = "message")]
pub enum Error {
    #[error("{0}")]
    Invalid(String),
    #[error("{0}")]
    Unsafe(String),
    #[error("{0}")]
    Hardware(String),
    #[error("{0}")]
    Io(String),
    #[error("Another operation is running. Wait for it to finish.")]
    Busy,
    #[error("Operation cancelled between files. Completed files were retained.")]
    Cancelled,
}
pub type Result<T> = std::result::Result<T, Error>;
impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e.to_string())
    }
}
impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Self::Invalid(e.to_string())
    }
}
impl From<reqwest::Error> for Error {
    fn from(e: reqwest::Error) -> Self {
        Self::Io(e.to_string())
    }
}
pub fn invalid(s: impl Into<String>) -> Error {
    Error::Invalid(s.into())
}
