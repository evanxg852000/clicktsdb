use std::sync::Arc;

// use async_trait::async_trait;
use serde_json::{json, Value};
// use std::{
//     future::{ready, Ready},
//     sync::Arc,
// };
// use tower_http::trace::TraceLayer;

use anyhow::{Context, Result};
use axum::{http::StatusCode, routing::get, Json, Router};
// use serde::{Deserialize, Serialize};
use services::{prometheus::prometheus_router, influxdb::influxdb_router};
use storage::StorageFactory;

use crate::settings::Settings;

pub async fn serve(settings: Settings) -> Result<()> {
    let storage = Arc::new(StorageFactory::open(&settings.storage)
        .map_err(anyhow::Error::msg)?);

    let app = Router::new()
        .route("/", get(welcome))
        .merge(influxdb_router(storage.clone()))
        .merge(prometheus_router(
            storage,
            settings.prometheus.read,
            settings.prometheus.write,
        ));

    let addr = format!("{}:{}", settings.web.host, settings.web.port);
    println!("Listening on `http://{}`.", addr);

    let listener = tokio::net::TcpListener::bind(addr.as_str())
        .await
        .context(format!("Failed to bind to address: `{}`.", addr))?;
    axum::serve(listener, app)
        .await
        .context("Failed to start the web server.")
}

async fn welcome() -> (StatusCode, Json<Value>) {
    //TODO: pull version from cargo.toml & add build hash
    (
        StatusCode::OK,
        Json(json!({
            "message": "Welcome to ClickTSDB",
            "version": "0.1.0",
        })),
    )
}
