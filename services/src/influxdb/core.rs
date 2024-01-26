use std::{sync::Arc, fmt::Display, collections::HashMap};

use axum::{response::IntoResponse, http::StatusCode};
use influxdb_line_protocol::{parse_lines, ParsedLine, FieldValue};
use serde::{Serialize, Deserialize};
use storage::{Label, TimeSeries, SERIES_NAME_LABEL, Sample, Storage, TimeSeriesInfo};
use thiserror::Error;

pub type InfluxDbResult<T> = Result<T, InfluxDbError>;

#[derive(Error, Debug)]
pub enum InfluxDbError {
    Storage(#[from] storage::StorageError),
    LineProtocol(#[from] influxdb_line_protocol::Error), 
    Other(String),
}

impl Display for InfluxDbError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Storage(err) => f.write_fmt(format_args!("StorageError {}", err)),
            Self::LineProtocol(err) => f.write_fmt(format_args!("LineProtocol {}", err)),
            Self::Other(err) => f.write_fmt(format_args!("Other {}", err)),
        }
    }
}

impl IntoResponse for InfluxDbError {
    fn into_response(self) -> axum::response::Response {
        let error_message = match self {
            InfluxDbError::Storage(err) => format!("Internal server error: {}", err),
            InfluxDbError::LineProtocol(err) => format!("Internal server error: {}", err),
            InfluxDbError::Other(err) => {
                format!("Internal server error: {}", err)
            }
        };
        (StatusCode::INTERNAL_SERVER_ERROR, error_message).into_response()
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WriteRequest {
    timeseries: Vec<TimeSeries>,
}

#[derive(Debug, Clone)]
pub struct InfluxDbStorage {
    storage: Arc<Storage>,
}

impl InfluxDbStorage {
    pub fn new(storage: Arc<Storage>) -> Self {
        InfluxDbStorage { storage }
    }

    /// Write samples to remote storage.
    pub async fn write(&self, request: WriteRequest) -> Result<(), InfluxDbError> {
        self.storage.write(request.timeseries).await?;
        Ok(())
    }
} 

pub fn decode_influx_lines_request(body: String) -> InfluxDbResult<WriteRequest> {
    let mut timeseries_map: HashMap<u64, TimeSeries> = HashMap::new();
    let mut parsed_lines = parse_lines(&body);
    while let Some(line_result) = parsed_lines.next() {
        let ParsedLine {
            series,
            field_set,
            timestamp,
        } = line_result?;

        let Some(timestamp) = timestamp  else {
            continue;
        };

        let tags = series.tag_set.map_or(vec![], |tags| {
            tags.into_iter()
                .map(|(name, value)| Label { name: name.to_string(), value: value.to_string() })
                .collect::<Vec<_>>()
        });
        for (field_name, field_value) in field_set {
            let Some(value) = convert_value(field_value) else {
                continue;
            };

            let mut labels = tags.clone();
            labels.push(Label{ 
                name: SERIES_NAME_LABEL.to_string(), 
                value: format!("{}_{}", series.measurement, field_name)
            });

            let series_info = TimeSeriesInfo::new(labels);
            if let Some(entry) = timeseries_map.get_mut(&series_info.id) {
                entry.push(Sample{timestamp, value});
            } else {
                timeseries_map.insert(series_info.id, TimeSeries::new(series_info.labels, vec![Sample{timestamp, value}]));
            }
        }
    }

    let timeseries = timeseries_map
        .into_iter()
        .map(|(_, ts)| ts)
        .collect();
    Ok(WriteRequest{timeseries})
}

fn convert_value(value: FieldValue) -> Option<f64> {
    match value {
        FieldValue::I64(v) => Some(v as f64),
        FieldValue::U64(v) => Some(v as f64),
        FieldValue::F64(v) => Some(v),
        FieldValue::String(_) => None,
        FieldValue::Boolean(_) => None,
    }
}


