use axum::{Router, routing::{get, post}};
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use crate::orchestrator::{state::AppState, handlers};

pub fn build_router(state: Arc<AppState>) -> Router {
    // Permissive CORS — the orchestrator HTTP server is localhost-only, and
    // the Tauri webview loads the Svelte app from a different origin, so we
    // need to allow cross-origin GET/POST from any caller.
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .route("/event", post(handlers::post_event))
        .route("/progress", post(handlers::post_progress))
        .route("/artifact", post(handlers::post_artifact))
        .route("/inbox/{sid}", post(handlers::post_inbox).get(handlers::get_inbox))
        .route("/inbox/{sid}/ack", post(handlers::ack_inbox))
        .route("/sessions", get(handlers::list_sessions))
        .route("/sessions/by-pid/{pid}", get(handlers::lookup_by_pid))
        .route("/sessions/{sid}/link",
               post(handlers::link_session).delete(handlers::unlink_session))
        .route("/sessions/{sid}/transcript-tail", get(handlers::get_transcript_tail))
        .route("/events", get(handlers::list_events))
        .route("/artifacts", get(handlers::list_artifacts_h))
        .with_state(state)
        .layer(cors)
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
