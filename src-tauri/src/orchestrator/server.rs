use axum::{Router, routing::{get, post}};
use std::sync::Arc;
use crate::orchestrator::{state::AppState, handlers};

pub fn build_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/event", post(handlers::post_event))
        .route("/progress", post(handlers::post_progress))
        .route("/artifact", post(handlers::post_artifact))
        .route("/inbox/{sid}", post(handlers::post_inbox).get(handlers::get_inbox))
        .with_state(state)
}
