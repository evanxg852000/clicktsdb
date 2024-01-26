use std::sync::Arc;

use axum::{extract::{Query, State}, http::StatusCode, routing::get, Json, Router};
use promql_parser::{label::{MatchOp, Matcher}, parser};
use serde_json::{json, Value};
use storage::Storage;

use super::remote::types::{label_matcher::Type, LabelMatcher, PrometheusRemoteStorageError, PrometheusResult, PrometheusStorage, Query as PromProtoBuffQuery};

use serde::Deserialize;

#[derive(Deserialize)]
pub struct PromReadQuery {
    qs: String,
    start: Option<i64>,
    end: Option<i64>,
}

impl PromReadQuery {
    fn to_prom_pb_query(&self, matchers: Vec<Matcher>) -> PromProtoBuffQuery {
        let label_matchers = matchers.into_iter()
            .map(|m| {
                match m.op {
                    MatchOp::Equal => LabelMatcher{r#type: Type::Eq as i32, name: m.name, value: m.value},
                    MatchOp::NotEqual => LabelMatcher{r#type: Type::Neq as i32, name: m.name, value: m.value},
                    MatchOp::Re(_) => LabelMatcher{r#type: Type::Re as i32, name: m.name, value: m.value},
                    MatchOp::NotRe(_) => LabelMatcher{r#type: Type::Re as i32, name: m.name, value: m.value},
                }
            }).collect();
        PromProtoBuffQuery{
            start_timestamp_ms: self.start.unwrap_or(i64::MIN),
            end_timestamp_ms: self.end.unwrap_or(i64::MAX),
            matchers: label_matchers,
            hints: None,
        }
    }
}


pub(crate) fn prometheus_query_language_router(storage: Arc<Storage>) -> Router {
    let router = Router::new()
        .route("/prometheus/query", get(promql_handler_service));

    let ctx = PrometheusStorage::new(storage);
    let router = router.with_state(ctx);
    router
}


pub async fn promql_handler_service(
    State(storage): State<PrometheusStorage>,
    prom_query: Query<PromReadQuery>,
) -> PrometheusResult<Json<Value>> {
    let query_ast = parser::parse(&prom_query.qs)
        .map_err(PrometheusRemoteStorageError::Other)?;
    let matchers = match query_ast {
        parser::Expr::VectorSelector(selector) => selector.matchers.matchers,
        parser::Expr::MatrixSelector(selector) => selector.vs.matchers.matchers,
        _ => return Ok(Json(json!({
                "error": "Only VectorSelector and MatrixSelector are supported.",
            }),
        ))
    };

    let prom_query  = prom_query.to_prom_pb_query(matchers);
    let timeseries = storage.read_prom_query(prom_query).await?;
    Ok(Json(json!({ "series": timeseries })))
}

