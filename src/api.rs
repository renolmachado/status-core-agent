use crate::db::{self, DbPool};
use crate::models::ServerMetrics;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::Json;
use axum::routing::get;
use axum::Router;
use serde::Deserialize;
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_http::cors::CorsLayer;

#[derive(Clone)]
pub struct AppState {
    pub current: Arc<RwLock<ServerMetrics>>,
    pub db: DbPool,
}

#[derive(Deserialize)]
pub struct HistoryParams {
    #[serde(default = "default_hours")]
    pub hours: u64,
}

fn default_hours() -> u64 {
    24
}

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/api/v1/current", get(get_current))
        .route("/api/v1/history", get(get_history))
        .layer(CorsLayer::permissive())
        .with_state(state)
}

async fn get_current(State(state): State<AppState>) -> Json<ServerMetrics> {
    let metrics = state.current.read().await.clone();
    Json(metrics)
}

async fn get_history(
    State(state): State<AppState>,
    Query(params): Query<HistoryParams>,
) -> Result<Json<Vec<ServerMetrics>>, StatusCode> {
    let db = state.db.clone();
    let hours = params.hours;
    let history = tokio::task::spawn_blocking(move || db::get_history(&db, hours))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(history))
}
