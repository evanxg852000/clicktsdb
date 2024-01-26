mod core;

use std::sync::Arc;

use axum::{extract::State, Router, routing::post};
use storage::Storage;

use self::core::{InfluxDbResult, InfluxDbStorage, decode_influx_lines_request};

async fn write_handler_service(
    State(storage): State<InfluxDbStorage>,
    body: String,
) -> InfluxDbResult<()> {
    let write_request = decode_influx_lines_request(body)?;
    storage.write(write_request).await
}

pub fn influxdb_router(storage: Arc<Storage>) -> Router {
    let ctx = InfluxDbStorage::new(storage);
    Router::new()
        .route("/influxdb", post(write_handler_service))
        .with_state(ctx)
}
