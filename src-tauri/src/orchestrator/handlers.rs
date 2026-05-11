use axum::{Json, extract::State};
use serde::Deserialize;
use std::sync::Arc;
use crate::orchestrator::{
    db::Store, state::AppState, bus::StateChange,
    status::{EventKind, transition},
};

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EventBody {
    SessionStart { session_id: String, cwd: String, pid: i64, label: Option<String> },
    UserPromptSubmit { session_id: String, prompt: String },
    PreToolUse { session_id: String, tool: String },
    Notification { session_id: String, message: String },
    Stop { session_id: String, error: Option<bool>, reason: Option<String> },
}

pub(crate) fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64
}

pub(crate) fn internal<E: std::fmt::Display>(e: E) -> (axum::http::StatusCode, String) {
    (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
}

fn current_status(store: &Store, id: &str) -> String {
    store.get_session(id).ok().flatten().map(|r| r.status).unwrap_or_else(|| "unknown".into())
}

fn emit_change(state: &Arc<AppState>, sid: &str, status: &str, reason: Option<String>) {
    let label = state.store.get_session(sid).ok().flatten().and_then(|r| r.label);
    state.bus.emit(StateChange {
        session_id: sid.to_string(),
        status: status.to_string(),
        label, reason,
    });
}

pub async fn post_event(
    State(state): State<Arc<AppState>>,
    Json(body): Json<EventBody>,
) -> Result<&'static str, (axum::http::StatusCode, String)> {
    let ts = now();
    let store = state.store.clone();
    let (sid, new_status, reason) = Store::run(store, move |store| -> Result<(String, &'static str, Option<String>), rusqlite::Error> {
        match body {
            EventBody::SessionStart { session_id, cwd, pid, label } => {
                store.upsert_session_start(&session_id, label.as_deref(), &cwd, pid, ts)?;
                let p = serde_json::json!({"cwd":cwd,"pid":pid,"label":label}).to_string();
                store.record_event(&session_id, ts, "session_start", &p)?;
                Ok((session_id, "working", None))
            }
            EventBody::UserPromptSubmit { session_id, prompt } => {
                store.set_last_user_prompt(&session_id, &prompt, ts)?;
                let ns = transition(&current_status(store, &session_id), &EventKind::UserPromptSubmit);
                store.apply_status(&session_id, ns, ts, None)?;
                store.record_event(&session_id, ts, "user_prompt",
                    &serde_json::to_string(&prompt).unwrap_or_default())?;
                Ok((session_id, ns, None))
            }
            EventBody::PreToolUse { session_id, tool } => {
                store.set_current_tool(&session_id, &tool, ts)?;
                let ns = transition(&current_status(store, &session_id), &EventKind::PreToolUse);
                store.apply_status(&session_id, ns, ts, None)?;
                store.record_event(&session_id, ts, "pre_tool",
                    &serde_json::to_string(&tool).unwrap_or_default())?;
                Ok((session_id, ns, None))
            }
            EventBody::Notification { session_id, message } => {
                let ns = transition(&current_status(store, &session_id), &EventKind::Notification);
                store.apply_status(&session_id, ns, ts, None)?;
                store.record_event(&session_id, ts, "notification",
                    &serde_json::to_string(&message).unwrap_or_default())?;
                Ok((session_id, ns, Some(message)))
            }
            EventBody::Stop { session_id, error, reason } => {
                let is_err = error.unwrap_or(false);
                let ns = transition(&current_status(store, &session_id), &EventKind::Stop { error: is_err });
                store.apply_status(&session_id, ns, ts, Some((ts, reason.as_deref().unwrap_or(""))))?;
                store.record_event(&session_id, ts, "stop",
                    &serde_json::json!({"error":is_err,"reason":reason}).to_string())?;
                Ok((session_id, ns, reason))
            }
        }
    }).await.map_err(internal)?;

    emit_change(&state, &sid, new_status, reason);
    Ok("ok")
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use std::sync::Arc;
    use tower::ServiceExt;
    use crate::orchestrator::{db::Store, state::AppState};

    fn test_app() -> (axum::Router, Arc<AppState>) {
        let store = Arc::new(Store::open_in_memory().unwrap());
        let state = AppState::new(store);
        let app = crate::orchestrator::server::build_router(state.clone());
        (app, state)
    }

    #[allow(dead_code)]
    fn seed(state: &AppState, id: &str) {
        state.store.upsert_session_start(id, None, "/tmp", 1, 1).unwrap();
    }

    #[tokio::test]
    async fn session_start_then_notification_transitions_to_needs_input() {
        let (app, state) = test_app();

        let resp = app.clone().oneshot(
            Request::builder().method("POST").uri("/event")
                .header("content-type","application/json")
                .body(Body::from(r#"{"session_id":"s1","kind":"session_start","cwd":"/tmp/x","pid":1234,"label":"feature"}"#))
                .unwrap()
        ).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(state.store.get_session("s1").unwrap().unwrap().status, "working");

        let resp = app.oneshot(
            Request::builder().method("POST").uri("/event")
                .header("content-type","application/json")
                .body(Body::from(r#"{"session_id":"s1","kind":"notification","message":"awaiting confirmation"}"#))
                .unwrap()
        ).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(state.store.get_session("s1").unwrap().unwrap().status, "needs_input");
    }

    #[tokio::test]
    async fn notification_event_emits_state_change_on_bus() {
        let (app, state) = test_app();
        state.store.upsert_session_start("s1", Some("demo"), "/tmp", 1, 1).unwrap();
        let mut rx = state.bus.subscribe();
        app.oneshot(
            Request::builder().method("POST").uri("/event")
                .header("content-type","application/json")
                .body(Body::from(r#"{"session_id":"s1","kind":"notification","message":"x"}"#))
                .unwrap()
        ).await.unwrap();
        let c = tokio::time::timeout(std::time::Duration::from_millis(500), rx.recv())
            .await.unwrap().unwrap();
        assert_eq!(c.session_id, "s1");
        assert_eq!(c.status, "needs_input");
        assert_eq!(c.label.as_deref(), Some("demo"));
    }
}
