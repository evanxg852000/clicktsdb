use std::path::PathBuf;

use fts::query::Query;

use crate::{error::StorageResult, TimeSeries};

/// A Prometheus/VictoriaMetrics storage
/// engine like implementation.
#[derive(Debug)]
pub struct NativeStorage {
    path: PathBuf,
}

impl NativeStorage {
    pub fn new(path: &str) -> Self {
        Self {
            path: PathBuf::from(path),
        }
    }

    pub async fn write(&self, _series: Vec<TimeSeries>) -> StorageResult<()> {
        Ok(())
    }

    pub async fn read(
        &self,
        _query: Query,
        _start_timestamp: i64,
        _end_timestamp: i64,
    ) -> StorageResult<Vec<TimeSeries>> {
        unimplemented!()
    }

    pub async fn truncate(&self, _timestamp: i64) -> StorageResult<()> {
        unimplemented!()
    }
}
