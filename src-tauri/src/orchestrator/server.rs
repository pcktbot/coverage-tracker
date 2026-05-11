use axum::{Router, routing::post};
use std::sync::Arc;
use crate::orchestrator::{state::AppState, handlers};

pub fn build_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/event", post(handlers::post_event))
        .with_state(state)
}
