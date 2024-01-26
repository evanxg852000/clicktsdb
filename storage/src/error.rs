use thiserror::Error;

pub type StorageResult<T> = std::result::Result<T, StorageError>;

#[derive(Error, Debug)]
pub enum StorageError {
    #[error("ClickHouse error")]
    ClickHouse(#[from] clickhouse::error::Error),
    #[error("Fts error")]
    Fts(#[from] fts::FtsError),
    #[error("Other error")]
    Other(String),
}
