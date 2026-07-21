use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Ollama error: {0}")]
    OllamaError(String),

    #[error("Cloud error: {0}")]
    CloudError(String),

    #[error("Cache error: {0}")]
    CacheError(String),

    #[error("Hardware error: {0}")]
    HardwareError(String),

    #[error("DAG error: {0}")]
    DagError(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Task error: {0}")]
    TaskError(String),

    #[error("Unknown error: {0}")]
    Unknown(String),
}

pub type Result<T> = std::result::Result<T, Error>;

#[cfg(feature = "python")]
impl From<pyo3::PyErr> for Error {
    fn from(err: pyo3::PyErr) -> Self {
        Error::CloudError(err.to_string())
    }
}
