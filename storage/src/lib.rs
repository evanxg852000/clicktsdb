mod clickhouse;
mod core;
mod error;
mod native;
mod settings;

pub use core::*;

use fts::query::Query;
pub use settings::StorageSettings;

use clickhouse::ClickHouseStorage;
pub use error::{StorageResult, StorageError};
use native::NativeStorage;

#[derive(Debug)]
pub enum Storage {
    Native(NativeStorage),
    ClickHouse(ClickHouseStorage),
}

impl Storage {
    pub async fn write(&self, series: Vec<TimeSeries>) -> StorageResult<()> {
        match self {
            Storage::Native(storage) => storage.write(series).await,
            Storage::ClickHouse(storage) => storage.write(series).await,
        }
    }

    pub async fn read(
        &self,
        query: Query,
        start_timestamp: i64,
        end_timestamp: i64,
    ) -> StorageResult<Vec<TimeSeries>> {
        match self {
            Storage::Native(storage) => storage.read(query, start_timestamp, end_timestamp).await,
            Storage::ClickHouse(storage) => {
                storage.read(query, start_timestamp, end_timestamp).await
            }
        }
    }

    pub async fn truncate(&self, timestamp: i64) -> StorageResult<()> {
        match self {
            Storage::Native(storage) => storage.truncate(timestamp).await,
            Storage::ClickHouse(storage) => storage.truncate(timestamp).await,
        }
    }
}

#[derive(Debug)]
pub struct StorageFactory;

impl StorageFactory {
    pub fn open(settings: &StorageSettings) -> StorageResult<Storage> {
        match settings {
            StorageSettings::Native(path) => Ok(Storage::Native(NativeStorage::new(path))),
            StorageSettings::ClickHouse {
                url,
                db,
                username,
                password,
                index_path, 
                memory_budget,
                sample_budget,
            } => {
                let store = ClickHouseStorage::new(
                    url,
                    db,
                    username,
                    password,
                    index_path,
                    *memory_budget * 1024 * 1024, // convert to MB
                    *sample_budget,
                )?;
                Ok(Storage::ClickHouse(store))
            },
        }
    }
}
