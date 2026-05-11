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

pub async fn spawn_on_random_port(state: Arc<AppState>)
    -> std::io::Result<(u16, tokio::task::JoinHandle<()>)>
{
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let port = listener.local_addr()?.port();
    let app = build_router(state);
    let handle = tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    Ok((port, handle))
}

pub async fn serve_on(state: Arc<AppState>, addr: &str) -> std::io::Result<()> {
    let listener = tokio::net::TcpListener::bind(addr).await?;
    let app = build_router(state);
    axum::serve(listener, app).await
}
