use thiserror::Error;

#[derive(Error, Debug)]
pub enum SendError {
    #[error("Soketto: {0}")]
    Soketto(#[from] soketto::connection::Error),
    #[error("Serde: {0}")]
    Serde(#[from] serde_json::error::Error),
}

#[derive(Error, Debug)]
pub enum ReceiveError {
    #[error("Soketto: {0}")]
    Soketto(#[from] soketto::connection::Error),
    #[error("Serde: {0}")]
    Serde(#[from] serde_json::error::Error),
    #[error("{0}")]
    FromUtf8(#[from] std::string::FromUtf8Error),
}
