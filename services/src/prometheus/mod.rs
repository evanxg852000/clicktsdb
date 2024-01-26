pub mod remote;
pub mod promql;

use std::sync::Arc;

use axum::Router;
use storage::Storage;

use self::{promql::prometheus_query_language_router, remote::prometheus_remote_router};

pub fn prometheus_router(storage: Arc<Storage>, can_read: bool, can_write: bool) -> Router {
    Router::new()
        .merge(prometheus_query_language_router(storage.clone()))
        .merge(prometheus_remote_router(
            storage,
            can_read,
            can_write,
        ))
}
    