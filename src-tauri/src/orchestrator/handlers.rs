use axum::{Json, extract::State};
use axum::extract::{Path, Query};
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

#[derive(Deserialize)]
pub struct ProgressBody { pub session_id: String, pub summary: String }

pub async fn post_progress(State(state): State<Arc<AppState>>, Json(b): Json<ProgressBody>)
    -> Result<&'static str, (axum::http::StatusCode, String)> {
    let ts = now();
    let store = state.store.clone();
    Store::run(store, move |s| -> rusqlite::Result<()> {
        s.set_last_progress(&b.session_id, &b.summary, ts)?;
        s.record_event(&b.session_id, ts, "progress",
            &serde_json::to_string(&b.summary).unwrap_or_default())?;
        Ok(())
    }).await.map_err(internal)?;
    Ok("ok")
}

#[derive(Deserialize)]
pub struct ArtifactBody { pub session_id: String, pub path: String, pub label: Option<String> }

pub async fn post_artifact(State(state): State<Arc<AppState>>, Json(b): Json<ArtifactBody>)
    -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    let ts = now();
    let store = state.store.clone();
    let id = Store::run(store, move |s| -> rusqlite::Result<i64> {
        let id = s.insert_artifact(&b.session_id, ts, &b.path, b.label.as_deref())?;
        s.record_event(&b.session_id, ts, "artifact",
            &serde_json::json!({"id":id,"path":b.path,"label":b.label}).to_string())?;
        Ok(id)
    }).await.map_err(internal)?;
    Ok(Json(serde_json::json!({"id": id})))
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FromKind { Human, Session }

impl FromKind {
    fn as_str(&self) -> &'static str {
        match self { FromKind::Human => "human", FromKind::Session => "session" }
    }
}

#[derive(Deserialize)]
pub struct InboxPostBody {
    pub from_kind: FromKind,
    pub from_id: Option<String>,
    pub message: String,
}

pub async fn post_inbox(State(state): State<Arc<AppState>>,
                        Path(sid): Path<String>, Json(b): Json<InboxPostBody>)
    -> Result<&'static str, (axum::http::StatusCode, String)> {
    let ts = now();
    let store = state.store.clone();
    Store::run(store, move |s| -> rusqlite::Result<()> {
        s.enqueue_inbox(&sid, b.from_kind.as_str(), b.from_id.as_deref(), ts, &b.message)?;
        s.record_event(&sid, ts, "inbox_in",
            &serde_json::json!({"from_kind":b.from_kind.as_str(),"message":b.message}).to_string())?;
        Ok(())
    }).await.map_err(internal)?;
    Ok("ok")
}

pub async fn list_sessions(State(state): State<Arc<AppState>>)
    -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    let store = state.store.clone();
    let rows = Store::run(store, |s| s.list_sessions())
        .await.map_err(internal)?;
    Ok(Json(serde_json::json!({"sessions": rows})))
}

#[derive(Deserialize)]
pub struct SessionQuery { pub session: String, pub limit: Option<i64> }

pub async fn list_events(State(state): State<Arc<AppState>>, Query(q): Query<SessionQuery>)
    -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    let limit = q.limit.unwrap_or(200);
    let store = state.store.clone();
    let rows = Store::run(store, move |s| s.events_for(&q.session, limit))
        .await.map_err(internal)?;
    let events: Vec<_> = rows.into_iter().map(|(id,ts,kind,payload)|
        serde_json::json!({"id":id,"ts":ts,"kind":kind,"payload":payload})).collect();
    Ok(Json(serde_json::json!({"events": events})))
}

pub async fn list_artifacts_h(State(state): State<Arc<AppState>>, Query(q): Query<SessionQuery>)
    -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    let store = state.store.clone();
    let rows = Store::run(store, move |s| s.list_artifacts(&q.session))
        .await.map_err(internal)?;
    Ok(Json(serde_json::json!({"artifacts": rows})))
}

pub async fn lookup_by_pid(State(state): State<Arc<AppState>>, Path(pid): Path<i64>)
    -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    let store = state.store.clone();
    match Store::run(store, move |s| s.find_session_by_pid(pid))
        .await.map_err(internal)? {
        Some(id) => Ok(Json(serde_json::json!({"session_id": id}))),
        None => Err((axum::http::StatusCode::NOT_FOUND, "no active session for pid".into())),
    }
}

pub async fn get_inbox(State(state): State<Arc<AppState>>, Path(sid): Path<String>)
    -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    let store = state.store.clone();
    let msgs = Store::run(store, move |s| s.drain_inbox(&sid, now()))
        .await.map_err(internal)?;
    Ok(Json(serde_json::json!({"messages": msgs})))
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

    #[tokio::test]
    async fn progress_updates_last_progress() {
        let (app, state) = test_app();
        seed(&state, "s1");
        let resp = app.oneshot(
            Request::builder().method("POST").uri("/progress")
                .header("content-type","application/json")
                .body(Body::from(r#"{"session_id":"s1","summary":"deployed"}"#))
                .unwrap()
        ).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(state.store.get_session("s1").unwrap().unwrap().last_progress.as_deref(),
                   Some("deployed"));
    }

    #[tokio::test]
    async fn artifact_returns_id() {
        let (app, state) = test_app();
        seed(&state, "s1");
        let resp = app.oneshot(
            Request::builder().method("POST").uri("/artifact")
                .header("content-type","application/json")
                .body(Body::from(r#"{"session_id":"s1","path":"https://example.com/pr/1","label":"PR 1"}"#))
                .unwrap()
        ).await.unwrap();
        let body = axum::body::to_bytes(resp.into_body(), 1024).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(v["id"].as_i64().unwrap() > 0);
    }

    #[tokio::test]
    async fn inbox_post_then_get_marks_delivered() {
        let (app, state) = test_app();
        seed(&state, "s1");
        app.clone().oneshot(
            Request::builder().method("POST").uri("/inbox/s1")
                .header("content-type","application/json")
                .body(Body::from(r#"{"from_kind":"human","message":"hi"}"#))
                .unwrap()
        ).await.unwrap();
        let r1 = app.clone().oneshot(Request::builder().method("GET").uri("/inbox/s1")
            .body(Body::empty()).unwrap()).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(
            &axum::body::to_bytes(r1.into_body(), 4096).await.unwrap()).unwrap();
        assert_eq!(v["messages"][0]["message"], "hi");
        let r2 = app.oneshot(Request::builder().method("GET").uri("/inbox/s1")
            .body(Body::empty()).unwrap()).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(
            &axum::body::to_bytes(r2.into_body(), 4096).await.unwrap()).unwrap();
        assert_eq!(v["messages"].as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn list_sessions_returns_all() {
        let (app, state) = test_app();
        seed(&state, "a"); seed(&state, "b");
        let resp = app.oneshot(Request::builder().method("GET").uri("/sessions")
            .body(Body::empty()).unwrap()).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(
            &axum::body::to_bytes(resp.into_body(), 4096).await.unwrap()).unwrap();
        assert_eq!(v["sessions"].as_array().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn events_for_session() {
        let (app, state) = test_app();
        seed(&state, "s1");
        state.store.record_event("s1", 1, "progress", r#""hi""#).unwrap();
        let resp = app.oneshot(Request::builder().method("GET").uri("/events?session=s1")
            .body(Body::empty()).unwrap()).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(
            &axum::body::to_bytes(resp.into_body(), 4096).await.unwrap()).unwrap();
        assert!(v["events"].as_array().unwrap().len() >= 1);
    }

    #[tokio::test]
    async fn lookup_session_by_pid() {
        let (app, state) = test_app();
        state.store.upsert_session_start("s1", None, "/tmp", 4242, 1).unwrap();
        let resp = app.oneshot(Request::builder().method("GET").uri("/sessions/by-pid/4242")
            .body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let v: serde_json::Value = serde_json::from_slice(
            &axum::body::to_bytes(resp.into_body(), 1024).await.unwrap()).unwrap();
        assert_eq!(v["session_id"], "s1");
    }

    #[tokio::test]
    async fn artifacts_endpoint_returns_list() {
        let (app, state) = test_app();
        seed(&state, "s1");
        state.store.insert_artifact("s1", 1, "/tmp/a", Some("A")).unwrap();
        let resp = app.oneshot(Request::builder().method("GET").uri("/artifacts?session=s1")
            .body(Body::empty()).unwrap()).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(
            &axum::body::to_bytes(resp.into_body(), 4096).await.unwrap()).unwrap();
        assert_eq!(v["artifacts"].as_array().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn inbox_rejects_unknown_from_kind() {
        let (app, state) = test_app();
        seed(&state, "s1");
        let resp = app.oneshot(
            Request::builder().method("POST").uri("/inbox/s1")
                .header("content-type","application/json")
                .body(Body::from(r#"{"from_kind":"bogus","message":"x"}"#))
                .unwrap()
        ).await.unwrap();
        assert!(resp.status().is_client_error());
    }
}
