pub mod routes;

use axum::Router;
use sqlx::SqlitePool;
use std::sync::Arc;
use tokio::sync::broadcast;

use crate::state::AppState;

pub fn build_router(state: Arc<AppState>) -> Router {
    Router::new()
        .nest("/api/v1", routes::api_routes())
        .with_state(state)
}
