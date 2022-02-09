use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Soketto: {0}")]
    Soketto(#[from] soketto::connection::Error),
    #[error("Serde: {0}")]
    Serde(#[from] serde_json::error::Error),
    #[error("UTF-8: {0}")]
    FromUtf8(#[from] std::string::FromUtf8Error),
    #[error("Sync-Service: {0}")]
    SyncService(String),
    #[error("The SideCar is not running")]
    SideCar,
    #[error("InfluxDB: {0}")]
    InfluxDB(#[from] influxdb::Error),
}
