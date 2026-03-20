pub mod routes;

use axum::Router;
use std::sync::Arc;

use crate::state::AppState;

pub fn build_router(state: Arc<AppState>) -> Router {
    Router::new()
        .nest("/api/v1", routes::api_routes(state.clone()))
        .with_state(state)
}
