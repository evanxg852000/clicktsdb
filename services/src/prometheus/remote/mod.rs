pub mod types;
mod utils;

use std::sync::Arc;

use axum::{
    body::Bytes,
    // routing::{get, post},
    // http::StatusCode,
    // Json,
    extract::State,
    http::{
        header::{CONTENT_ENCODING, CONTENT_TYPE},
        HeaderMap, HeaderValue,
    },
    routing::{get, post},
    Router,
};

use prost::Message;
use storage::Storage;

use self::types::{
    PrometheusRemoteStorageError, PrometheusResult, PrometheusStorage, ReadRequest, WriteRequest,
};

use super::promql::promql_handler_service;

fn decode_request<T: Message + Default>(compressed_bytes: &[u8]) -> PrometheusResult<T> {
    utils::decode_snappy(compressed_bytes).and_then(|uncompressed_bytes| {
        T::decode(uncompressed_bytes.as_slice())
            .map_err(PrometheusRemoteStorageError::ProtocolBuffer)
    })
}

async fn read_handler_service(
    State(storage): State<PrometheusStorage>,
    body: Bytes,
) -> PrometheusResult<(HeaderMap, Vec<u8>)> {
    let read_request = decode_request::<ReadRequest>(&body)?;
    let read_response = storage.read(read_request).await?;
    let response_body = utils::encode_snappy(read_response.encode_to_vec().as_slice())?;
    let mut headers = HeaderMap::new();
    headers.insert(
        CONTENT_TYPE,
        HeaderValue::from_static("application/x-protobuf"),
    );
    headers.insert(CONTENT_ENCODING, HeaderValue::from_static("snappy"));
    Ok((headers, response_body))
}

async fn write_handler_service(
    State(storage): State<PrometheusStorage>,
    body: Bytes,
) -> PrometheusResult<()> {
    let write_request = decode_request::<WriteRequest>(&body)?;
    storage.write(write_request).await
}

pub(crate) fn prometheus_remote_router(storage: Arc<Storage>, can_read: bool, can_write: bool) -> Router {
    let router = Router::new()
        .route("/prometheus", get(promql_handler_service));

    let router = if can_read {
        router.route("/prometheus/read", post(read_handler_service))
    } else {
        router
    };

    let router = if can_write {
        router.route("/prometheus/write", post(write_handler_service))
    } else {
        router
    };

    let ctx = PrometheusStorage::new(storage);
    let router = router.with_state(ctx);
    router
}
