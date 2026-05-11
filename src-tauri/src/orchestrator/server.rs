use axum::{Router, routing::{get, post}};
use std::sync::Arc;
use crate::orchestrator::{state::AppState, handlers};

pub fn build_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/event", post(handlers::post_event))
        .route("/progress", post(handlers::post_progress))
        .route("/artifact", post(handlers::post_artifact))
        .route("/inbox/{sid}", post(handlers::post_inbox).get(handlers::get_inbox))
        .route("/sessions", get(handlers::list_sessions))
        .route("/sessions/by-pid/{pid}", get(handlers::lookup_by_pid))
        .route("/events", get(handlers::list_events))
        .route("/artifacts", get(handlers::list_artifacts_h))
        .with_state(state)
}
