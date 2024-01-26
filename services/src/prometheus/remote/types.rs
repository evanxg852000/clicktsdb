use axum::{http::StatusCode, response::IntoResponse};
use std::{fmt::Display, sync::Arc};
use storage::{Label as NativeLabel, Sample as NativeSample, Storage, TimeSeries as NativeSeries};
use fts::query::Query as NativeQuery;
use thiserror::Error;

mod prompb {
    include!(concat!(env!("OUT_DIR"), "/prometheus.rs"));
}
pub use prompb::*;

pub type PrometheusResult<T> = Result<T, PrometheusRemoteStorageError>;

#[derive(Error, Debug)]
pub enum PrometheusRemoteStorageError {
    Storage(#[from] storage::StorageError),
    Snappy(#[from] snap::Error),
    ProtocolBuffer(#[from] prost::DecodeError),
    Other(String),
}

impl Display for PrometheusRemoteStorageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Storage(err) => f.write_fmt(format_args!("StorageError {}", err)),
            Self::Snappy(err) => f.write_fmt(format_args!("SnappyError {}", err)),
            Self::ProtocolBuffer(err) => f.write_fmt(format_args!("ProtocolBufferError {}", err)),
            Self::Other(err) => f.write_fmt(format_args!("OtherError {}", err)),
        }
    }
}

impl IntoResponse for PrometheusRemoteStorageError {
    fn into_response(self) -> axum::response::Response {
        let error_message = match self {
            PrometheusRemoteStorageError::Storage(err) => format!("Internal server error: {}", err),
            PrometheusRemoteStorageError::Snappy(err) => format!("Internal server error: {}", err),
            PrometheusRemoteStorageError::ProtocolBuffer(err) => {
                format!("Internal server error: {}", err)
            },
            PrometheusRemoteStorageError::Other(err) => format!("Internal server error: {}", err),
        };
        (StatusCode::INTERNAL_SERVER_ERROR, error_message).into_response()
    }
}

#[derive(Debug, Clone)]
pub struct PrometheusStorage {
    storage: Arc<Storage>,
}

impl PrometheusStorage {
    pub fn new(storage: Arc<Storage>) -> Self {
        PrometheusStorage { storage }
    }

    /// Write samples to remote storage.
    pub async fn write(&self, request: WriteRequest) -> Result<(), PrometheusRemoteStorageError> {
        println!( "Received WriteRequest: {} records", request.timeseries.len());
        let native_series = request
            .timeseries
            .into_iter()
            .map(|series| {
                let labels = series
                    .labels
                    .into_iter()
                    .map(|label| NativeLabel {
                        name: label.name,
                        value: label.value,
                    })
                    .collect();

                let samples = series
                    .samples
                    .into_iter()
                    .map(|sample| NativeSample {
                        timestamp: sample.timestamp,
                        value: sample.value,
                    })
                    .collect();

                NativeSeries::new(labels, samples)
            })
            .collect();
        self.storage.write(native_series).await?;
        Ok(())
    }

    /// Serves HTTP read request from remote storage.
    ///
    /// [ReadRequest](crate::types::ReadRequest) may contain multiple [queries](crate::types::Query).
    pub async fn read(
        &self,
        // ctx: Self::Context,
        request: ReadRequest,
    ) -> Result<ReadResponse, PrometheusRemoteStorageError> {
        println!("Received ReadRequest: {:?} queries", request.queries.len());
        let results = futures::future::join_all(
            request
                .queries
                .into_iter()
                .map(|q| async { self.process_query(q).await }),
        )
        .await
        .into_iter()
        .collect::<Result<Vec<_>, PrometheusRemoteStorageError>>()?;
        Ok(ReadResponse { results })
    }

    pub async fn read_prom_query(
        &self,
        prom_query: Query,
    ) -> Result<Vec<NativeSeries>, PrometheusRemoteStorageError> {
        let start_timestamp = prom_query.start_timestamp_ms;
        let end_timestamp = prom_query.end_timestamp_ms;
        let query = convert_prom_query_to_native_query(prom_query)?;
        let native_series = self.storage.read(query, start_timestamp, end_timestamp).await?;
        Ok(native_series)
    }

    /// Process a single read [Query](crate::types::Query) query.
    async fn process_query(
        &self,
        prom_query: Query,
    ) -> Result<QueryResult, PrometheusRemoteStorageError> {
        let start_timestamp = prom_query.start_timestamp_ms;
        let end_timestamp = prom_query.end_timestamp_ms;
        let query = convert_prom_query_to_native_query(prom_query)?;

        let native_series = self.storage.read(query, start_timestamp, end_timestamp).await?;
        let mut timeseries = Vec::with_capacity(native_series.len());
        for series in native_series {
            let(native_labels, native_samples) = series.into_raw();
            let labels = native_labels.into_iter()
                .map(|l| Label{name: l.name, value: l.value})
                .collect();
            let samples = native_samples.into_iter()
                .map(|s| Sample{value: s.value, timestamp: s.timestamp})
                .collect();
            timeseries.push(TimeSeries{
                labels,
                samples,
                ..Default::default()
            });
        }

        Ok(QueryResult { timeseries })
    }
}

fn convert_prom_query_to_native_query(_prom_query: Query) -> Result<NativeQuery, PrometheusRemoteStorageError>{
    //TODO: convert prom_query to native query
    // for matcher in prom_query.matchers {
    //     match matcher.r#type() {
    //         label_matcher::Type::Eq => todo!(),
    //         label_matcher::Type::Neq => todo!(),
    //         label_matcher::Type::Re => todo!(),
    //         label_matcher::Type::Nre => todo!(),
    //     }
    // }
    Ok(NativeQuery::All)
}
