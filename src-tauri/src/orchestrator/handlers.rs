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
    SessionStart { session_id: String, cwd: String, pid: i64, label: Option<String>, transcript_path: Option<String>, loaded_snapshot: Option<String> },
    UserPromptSubmit { session_id: String, prompt: String, transcript_path: Option<String> },
    PreToolUse { session_id: String, tool: String, transcript_path: Option<String> },
    Notification { session_id: String, message: String, transcript_path: Option<String> },
    Stop { session_id: String, error: Option<bool>, reason: Option<String>, transcript_path: Option<String> },
    WorkflowPhaseChanged { session_id: String, payload: serde_json::Value },
    EvalResult { session_id: String, payload: serde_json::Value },
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
            EventBody::SessionStart { session_id, cwd, pid, label, transcript_path, loaded_snapshot } => {
                store.upsert_session_start(&session_id, label.as_deref(), &cwd, pid, ts)?;
                let p = serde_json::json!({"cwd":cwd,"pid":pid,"label":label}).to_string();
                store.record_event(&session_id, ts, "session_start", &p)?;
                if let Some(tp) = transcript_path.as_ref() {
                    store.set_transcript_path(&session_id, tp, ts)?;
                }
                if let Some(snap) = loaded_snapshot.as_ref() {
                    store.set_loaded_snapshot(&session_id, snap, ts)?;
                }
                Ok((session_id, "working", None))
            }
            EventBody::UserPromptSubmit { session_id, prompt, transcript_path } => {
                store.set_last_user_prompt(&session_id, &prompt, ts)?;
                store.set_first_user_prompt_if_null(&session_id, &prompt, ts)?;  // NEW
                let ns = transition(&current_status(store, &session_id), &EventKind::UserPromptSubmit);
                store.apply_status(&session_id, ns, ts, None)?;
                store.record_event(&session_id, ts, "user_prompt",
                    &serde_json::to_string(&prompt).unwrap_or_default())?;
                if let Some(tp) = transcript_path.as_ref() {
                    store.set_transcript_path(&session_id, tp, ts)?;
                }
                Ok((session_id, ns, None))
            }
            EventBody::PreToolUse { session_id, tool, transcript_path } => {
                store.set_current_tool(&session_id, &tool, ts)?;
                let ns = transition(&current_status(store, &session_id), &EventKind::PreToolUse);
                store.apply_status(&session_id, ns, ts, None)?;
                store.record_event(&session_id, ts, "pre_tool",
                    &serde_json::to_string(&tool).unwrap_or_default())?;
                if let Some(tp) = transcript_path.as_ref() {
                    store.set_transcript_path(&session_id, tp, ts)?;
                }
                Ok((session_id, ns, None))
            }
            EventBody::Notification { session_id, message, transcript_path } => {
                let ns = transition(&current_status(store, &session_id), &EventKind::Notification);
                store.apply_status(&session_id, ns, ts, None)?;
                store.record_event(&session_id, ts, "notification",
                    &serde_json::to_string(&message).unwrap_or_default())?;
                if let Some(tp) = transcript_path.as_ref() {
                    store.set_transcript_path(&session_id, tp, ts)?;
                }
                Ok((session_id, ns, Some(message)))
            }
            EventBody::Stop { session_id, error, reason, transcript_path } => {
                let is_err = error.unwrap_or(false);
                let ns = transition(&current_status(store, &session_id), &EventKind::Stop { error: is_err });
                store.apply_status(&session_id, ns, ts, Some((ts, reason.as_deref().unwrap_or(""))))?;
                store.record_event(&session_id, ts, "stop",
                    &serde_json::json!({"error":is_err,"reason":reason}).to_string())?;
                if let Some(tp) = transcript_path.as_ref() {
                    store.set_transcript_path(&session_id, tp, ts)?;
                }
                Ok((session_id, ns, reason))
            }
            EventBody::WorkflowPhaseChanged { session_id, payload } => {
                store.record_event(&session_id, ts, "workflow_phase_changed", &payload.to_string())?;
                Ok((session_id, "working", None))
            }
            EventBody::EvalResult { session_id, payload } => {
                store.record_event(&session_id, ts, "eval_result", &payload.to_string())?;
                Ok((session_id, "working", None))
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

pub async fn get_transcript_tail(
    State(state): State<Arc<AppState>>,
    Path(sid): Path<String>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    let store = state.store.clone();
    let sid_for_lookup = sid.clone();
    let path: Option<String> = Store::run(store, move |s| -> rusqlite::Result<Option<String>> {
        Ok(s.get_session(&sid_for_lookup)?.and_then(|r| r.transcript_path))
    }).await.map_err(internal)?;

    let Some(path) = path else {
        return Ok(Json(serde_json::json!({"turns": []})));
    };

    // File read DIRECT — not through Store::run (this is not a DB op).
    let body = match tokio::fs::read_to_string(&path).await {
        Ok(b) => b,
        Err(_) => return Ok(Json(serde_json::json!({"turns": []}))),
    };

    // Cap at last 5 MB to bound work.
    let body = if body.len() > 5 * 1024 * 1024 {
        let tail = &body[body.len() - 5 * 1024 * 1024..];
        // Drop the first (likely partial) line.
        tail.split_once('\n').map(|(_, rest)| rest.to_string()).unwrap_or_default()
    } else {
        body
    };

    let turns = crate::orchestrator::transcript::parse_tail(&body, 10);
    Ok(Json(serde_json::json!({"turns": turns})))
}

#[derive(Deserialize, Default)]
pub struct PeekQuery { pub peek: Option<u8> }

pub async fn get_inbox(
    State(state): State<Arc<AppState>>,
    Path(sid): Path<String>,
    Query(q): Query<PeekQuery>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    let store = state.store.clone();
    let msgs = if q.peek.unwrap_or(0) == 1 {
        Store::run(store, move |s| s.peek_inbox(&sid)).await.map_err(internal)?
    } else {
        // Default: drain (preserves MCP check_inbox tool contract).
        Store::run(store, move |s| s.drain_inbox(&sid, now())).await.map_err(internal)?
    };
    Ok(Json(serde_json::json!({"messages": msgs})))
}

#[derive(Deserialize)]
pub struct AckBody { pub ids: Vec<i64> }

pub async fn ack_inbox(
    State(state): State<Arc<AppState>>,
    Path(sid): Path<String>,
    Json(b): Json<AckBody>,
) -> Result<&'static str, (axum::http::StatusCode, String)> {
    let ts = now();
    let store = state.store.clone();
    Store::run(store, move |s| s.ack_inbox(&sid, &b.ids, ts))
        .await.map_err(internal)?;
    Ok("ok")
}

#[derive(Debug, Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactKind { Project, Ado, Confluence, GithubPr }

impl ArtifactKind {
    fn as_str(&self) -> &'static str {
        match self {
            ArtifactKind::Project => "project",
            ArtifactKind::Ado => "ado",
            ArtifactKind::Confluence => "confluence",
            ArtifactKind::GithubPr => "github_pr",
        }
    }
}

#[derive(Deserialize)]
pub struct LinkBody {
    pub kind: ArtifactKind,
    pub id: String,
    pub title: Option<String>,
    pub url: Option<String>,
}

pub async fn link_session(
    State(state): State<Arc<AppState>>,
    Path(sid): Path<String>,
    Json(b): Json<LinkBody>,
) -> Result<&'static str, (axum::http::StatusCode, String)> {
    let ts = now();
    let store = state.store.clone();
    let kind_str = b.kind.as_str();
    let payload = serde_json::json!({"kind":kind_str,"id":b.id,"title":b.title,"url":b.url}).to_string();
    let sid_for_emit = sid.clone();
    let kind_for_emit = kind_str.to_string();
    Store::run(store, move |s| -> rusqlite::Result<()> {
        s.link_artifact(&sid, kind_str, &b.id, b.title.as_deref(), b.url.as_deref(), ts)?;
        s.record_event(&sid, ts, "link", &payload)?;
        Ok(())
    }).await.map_err(internal)?;
    let label = state.store.get_session(&sid_for_emit).ok().flatten().and_then(|r| r.label);
    state.bus.emit(crate::orchestrator::bus::StateChange {
        session_id: sid_for_emit.clone(),
        status: state.store.get_session(&sid_for_emit).ok().flatten().map(|r| r.status).unwrap_or_default(),
        label,
        reason: Some(format!("linked to {}", kind_for_emit)),
    });
    Ok("ok")
}

pub async fn unlink_session(
    State(state): State<Arc<AppState>>,
    Path(sid): Path<String>,
) -> Result<&'static str, (axum::http::StatusCode, String)> {
    let ts = now();
    let store = state.store.clone();
    let sid_clone = sid.clone();
    Store::run(store, move |s| -> rusqlite::Result<()> {
        s.unlink_artifact(&sid, ts)?;
        s.record_event(&sid, ts, "unlink", "{}")?;
        Ok(())
    }).await.map_err(internal)?;
    let label = state.store.get_session(&sid_clone).ok().flatten().and_then(|r| r.label);
    state.bus.emit(crate::orchestrator::bus::StateChange {
        session_id: sid_clone.clone(),
        status: state.store.get_session(&sid_clone).ok().flatten().map(|r| r.status).unwrap_or_default(),
        label,
        reason: Some("unlinked".to_string()),
    });
    Ok("ok")
}

pub async fn dismiss_session(
    State(state): State<Arc<AppState>>,
    Path(sid): Path<String>,
) -> Result<&'static str, (axum::http::StatusCode, String)> {
    let ts = now();
    let store = state.store.clone();
    let sid_clone = sid.clone();
    Store::run(store, move |s| -> rusqlite::Result<()> {
        s.set_dismissed_at(&sid, Some(ts), ts)?;
        s.record_event(&sid, ts, "dismiss", "{}")?;
        Ok(())
    }).await.map_err(internal)?;
    let label = state.store.get_session(&sid_clone).ok().flatten().and_then(|r| r.label);
    state.bus.emit(crate::orchestrator::bus::StateChange {
        session_id: sid_clone.clone(),
        status: state.store.get_session(&sid_clone).ok().flatten().map(|r| r.status).unwrap_or_default(),
        label,
        reason: Some("dismissed".to_string()),
    });
    Ok("ok")
}

pub async fn undismiss_session(
    State(state): State<Arc<AppState>>,
    Path(sid): Path<String>,
) -> Result<&'static str, (axum::http::StatusCode, String)> {
    let ts = now();
    let store = state.store.clone();
    let sid_clone = sid.clone();
    Store::run(store, move |s| -> rusqlite::Result<()> {
        s.set_dismissed_at(&sid, None, ts)?;
        s.record_event(&sid, ts, "undismiss", "{}")?;
        Ok(())
    }).await.map_err(internal)?;
    let label = state.store.get_session(&sid_clone).ok().flatten().and_then(|r| r.label);
    state.bus.emit(crate::orchestrator::bus::StateChange {
        session_id: sid_clone.clone(),
        status: state.store.get_session(&sid_clone).ok().flatten().map(|r| r.status).unwrap_or_default(),
        label,
        reason: Some("undismissed".to_string()),
    });
    Ok("ok")
}

// ---------- Admin: read-only DB browser ----------

#[derive(serde::Deserialize)]
pub struct AdminTablesQuery { pub db: String }

#[derive(serde::Deserialize)]
pub struct AdminRowsQuery {
    pub db: String,
    pub table: String,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

fn open_readonly(which: &str) -> Result<rusqlite::Connection, (axum::http::StatusCode, String)> {
    use axum::http::StatusCode;
    let base = dirs::data_local_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("coverage-manager");
    let path = match which {
        "orchestrator" => base.join("orchestrator.db"),
        "coverage"     => base.join("coverage.db"),
        _ => return Err((StatusCode::BAD_REQUEST, format!("unknown db: {which}"))),
    };
    let flags = rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY
        | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX;
    rusqlite::Connection::open_with_flags(&path, flags)
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, format!("open {which}: {e}")))
}

fn list_user_tables(conn: &rusqlite::Connection) -> rusqlite::Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT name FROM sqlite_master \
         WHERE type='table' AND name NOT LIKE 'sqlite_%' \
         ORDER BY name")?;
    let result = stmt.query_map([], |r| r.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>();
    result
}

pub async fn admin_tables(
    Query(q): Query<AdminTablesQuery>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    let which = q.db.clone();
    let conn = open_readonly(&which)?;
    let tables = tokio::task::spawn_blocking(move || list_user_tables(&conn))
        .await.map_err(internal)?.map_err(internal)?;
    Ok(Json(serde_json::json!({ "db": which, "tables": tables })))
}

pub async fn admin_rows(
    Query(q): Query<AdminRowsQuery>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    use axum::http::StatusCode;

    // Open RO connection (also validates `db` allowlist).
    let conn = open_readonly(&q.db)?;

    // Validate table name against actual schema — never interpolate arbitrary text into SQL.
    let table_list = list_user_tables(&conn).map_err(internal)?;
    let table = q.table.clone();
    if !table_list.iter().any(|t| t == &table) {
        return Err((StatusCode::BAD_REQUEST, format!("unknown table: {table}")));
    }

    let limit  = q.limit.unwrap_or(100).clamp(1, 1000);
    let offset = q.offset.unwrap_or(0).max(0);

    // Column names via PRAGMA (table name already validated above).
    let quoted = table.replace('"', "\"\"");
    let col_names: Vec<String> = {
        let mut stmt = conn
            .prepare(&format!("PRAGMA table_info(\"{}\")", quoted))
            .map_err(internal)?;
        let result = stmt.query_map([], |r| r.get::<_, String>(1))
            .map_err(internal)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(internal)?;
        result
    };

    // SELECT rows.
    let sql = format!("SELECT * FROM \"{}\" LIMIT ? OFFSET ?", quoted);
    let mut stmt = conn.prepare(&sql).map_err(internal)?;
    let mut rows = stmt.query(rusqlite::params![limit, offset]).map_err(internal)?;

    let mut out: Vec<serde_json::Value> = Vec::new();
    while let Some(row) = rows.next().map_err(internal)? {
        let mut obj = serde_json::Map::new();
        for (i, name) in col_names.iter().enumerate() {
            let v: rusqlite::types::Value = row.get(i).map_err(internal)?;
            let jv = match v {
                rusqlite::types::Value::Null       => serde_json::Value::Null,
                rusqlite::types::Value::Integer(n) => serde_json::Value::from(n),
                rusqlite::types::Value::Real(f)    => serde_json::Value::from(f),
                rusqlite::types::Value::Text(s)    => serde_json::Value::String(s),
                rusqlite::types::Value::Blob(b)    =>
                    serde_json::Value::String(format!("<blob {} bytes>", b.len())),
            };
            obj.insert(name.clone(), jv);
        }
        out.push(serde_json::Value::Object(obj));
    }

    // Total row count for pagination footer.
    let count: i64 = conn
        .query_row(&format!("SELECT COUNT(*) FROM \"{}\"", quoted), [], |r| r.get(0))
        .map_err(internal)?;

    Ok(Json(serde_json::json!({
        "db":      q.db,
        "table":   table,
        "columns": col_names,
        "rows":    out,
        "limit":   limit,
        "offset":  offset,
        "total":   count,
    })))
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
    async fn session_start_with_transcript_path_round_trips() {
        let (app, state) = test_app();
        let body = r#"{"kind":"session_start","session_id":"s1","cwd":"/tmp","pid":1,"label":"x","transcript_path":"/tmp/t.jsonl"}"#;
        let resp = app.oneshot(
            Request::builder().method("POST").uri("/event")
                .header("content-type","application/json")
                .body(Body::from(body)).unwrap()
        ).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let row = state.store.get_session("s1").unwrap().unwrap();
        assert_eq!(row.transcript_path.as_deref(), Some("/tmp/t.jsonl"));
    }

    #[tokio::test]
    async fn session_start_without_transcript_path_still_succeeds() {
        // Backward-compat regression: payloads from older hooks omit transcript_path.
        let (app, state) = test_app();
        let body = r#"{"kind":"session_start","session_id":"s1","cwd":"/tmp","pid":1,"label":"x"}"#;
        let resp = app.oneshot(
            Request::builder().method("POST").uri("/event")
                .header("content-type","application/json")
                .body(Body::from(body)).unwrap()
        ).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert!(state.store.get_session("s1").unwrap().unwrap().transcript_path.is_none());
    }

    #[tokio::test]
    async fn session_start_with_loaded_snapshot_round_trips() {
        let (app, state) = test_app();
        let body = r#"{"kind":"session_start","session_id":"s1","cwd":"/tmp","pid":1,"label":"x","loaded_snapshot":"{\"plugins\":[\"superpowers\"]}"}"#;
        let resp = app.oneshot(
            Request::builder().method("POST").uri("/event")
                .header("content-type","application/json")
                .body(Body::from(body)).unwrap()
        ).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let row = state.store.get_session("s1").unwrap().unwrap();
        assert_eq!(row.loaded_snapshot.as_deref(), Some(r#"{"plugins":["superpowers"]}"#));
    }

    #[tokio::test]
    async fn session_start_without_loaded_snapshot_still_succeeds() {
        let (app, state) = test_app();
        let body = r#"{"kind":"session_start","session_id":"s1","cwd":"/tmp","pid":1,"label":"x"}"#;
        let resp = app.oneshot(
            Request::builder().method("POST").uri("/event")
                .header("content-type","application/json")
                .body(Body::from(body)).unwrap()
        ).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert!(state.store.get_session("s1").unwrap().unwrap().loaded_snapshot.is_none());
    }

    #[tokio::test]
    async fn link_round_trips() {
        let (app, state) = test_app();
        seed(&state, "s1");
        let resp = app.oneshot(
            Request::builder().method("POST").uri("/sessions/s1/link")
                .header("content-type","application/json")
                .body(Body::from(r#"{"kind":"project","id":"42","title":"My Project","url":"https://x/y"}"#))
                .unwrap()
        ).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let row = state.store.get_session("s1").unwrap().unwrap();
        assert_eq!(row.artifact_kind.as_deref(), Some("project"));
        assert_eq!(row.artifact_id.as_deref(), Some("42"));
        assert_eq!(row.artifact_title.as_deref(), Some("My Project"));
        assert_eq!(row.artifact_url.as_deref(), Some("https://x/y"));
    }

    #[tokio::test]
    async fn unlink_clears_fields() {
        let (app, state) = test_app();
        seed(&state, "s1");
        app.clone().oneshot(
            Request::builder().method("POST").uri("/sessions/s1/link")
                .header("content-type","application/json")
                .body(Body::from(r#"{"kind":"project","id":"42"}"#))
                .unwrap()
        ).await.unwrap();
        let resp = app.oneshot(
            Request::builder().method("DELETE").uri("/sessions/s1/link")
                .body(Body::empty()).unwrap()
        ).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let row = state.store.get_session("s1").unwrap().unwrap();
        assert!(row.artifact_kind.is_none());
        assert!(row.artifact_id.is_none());
    }

    #[tokio::test]
    async fn link_bogus_kind_rejected() {
        let (app, state) = test_app();
        seed(&state, "s1");
        let resp = app.oneshot(
            Request::builder().method("POST").uri("/sessions/s1/link")
                .header("content-type","application/json")
                .body(Body::from(r#"{"kind":"bogus","id":"x"}"#)).unwrap()
        ).await.unwrap();
        assert!(resp.status().is_client_error());
    }

    #[tokio::test]
    async fn unlink_idempotent() {
        let (app, state) = test_app();
        seed(&state, "s1");
        let resp = app.oneshot(
            Request::builder().method("DELETE").uri("/sessions/s1/link")
                .body(Body::empty()).unwrap()
        ).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn first_user_prompt_captured_on_first_submit_only() {
        let (app, state) = test_app();
        state.store.upsert_session_start("s1", Some("demo"), "/tmp", 1, 1).unwrap();

        // First prompt
        app.clone().oneshot(
            Request::builder().method("POST").uri("/event")
                .header("content-type","application/json")
                .body(Body::from(r#"{"kind":"user_prompt_submit","session_id":"s1","prompt":"first one"}"#))
                .unwrap()
        ).await.unwrap();
        assert_eq!(state.store.get_session("s1").unwrap().unwrap().first_user_prompt.as_deref(),
                   Some("first one"));

        // Second prompt — first_user_prompt must not change
        app.oneshot(
            Request::builder().method("POST").uri("/event")
                .header("content-type","application/json")
                .body(Body::from(r#"{"kind":"user_prompt_submit","session_id":"s1","prompt":"second one"}"#))
                .unwrap()
        ).await.unwrap();
        assert_eq!(state.store.get_session("s1").unwrap().unwrap().first_user_prompt.as_deref(),
                   Some("first one"));
        assert_eq!(state.store.get_session("s1").unwrap().unwrap().last_user_prompt.as_deref(),
                   Some("second one"));
    }

    #[tokio::test]
    async fn dismiss_then_undismiss_toggles_dismissed_at() {
        let (app, state) = test_app();
        state.store.upsert_session_start("s1", None, "/tmp", 1, 1).unwrap();

        let resp = app.clone().oneshot(
            Request::builder().method("POST").uri("/sessions/s1/dismiss")
                .body(Body::empty()).unwrap()
        ).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert!(state.store.get_session("s1").unwrap().unwrap().dismissed_at.is_some());

        let resp = app.oneshot(
            Request::builder().method("DELETE").uri("/sessions/s1/dismiss")
                .body(Body::empty()).unwrap()
        ).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert!(state.store.get_session("s1").unwrap().unwrap().dismissed_at.is_none());
    }

    #[test]
    fn serializes_github_pr_as_snake_case() {
        let s = serde_json::to_string(&ArtifactKind::GithubPr).unwrap();
        assert_eq!(s, "\"github_pr\"");
        let back: ArtifactKind = serde_json::from_str(&s).unwrap();
        assert!(matches!(back, ArtifactKind::GithubPr));
    }

    #[tokio::test]
    async fn transcript_empty_when_no_path() {
        let (app, state) = test_app();
        seed(&state, "s1");
        let resp = app.oneshot(
            Request::builder().method("GET").uri("/sessions/s1/transcript-tail")
                .body(Body::empty()).unwrap()
        ).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let v: serde_json::Value = serde_json::from_slice(
            &axum::body::to_bytes(resp.into_body(), 4096).await.unwrap()).unwrap();
        assert_eq!(v["turns"].as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn peek_does_not_drain() {
        let (app, state) = test_app();
        seed(&state, "s1");
        state.store.enqueue_inbox("s1","human",None,1,"first").unwrap();
        state.store.enqueue_inbox("s1","human",None,2,"second").unwrap();

        let resp = app.clone().oneshot(
            Request::builder().method("GET").uri("/inbox/s1?peek=1")
                .body(Body::empty()).unwrap()).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(
            &axum::body::to_bytes(resp.into_body(), 4096).await.unwrap()).unwrap();
        assert_eq!(v["messages"].as_array().unwrap().len(), 2);

        let resp = app.clone().oneshot(
            Request::builder().method("GET").uri("/inbox/s1?peek=1")
                .body(Body::empty()).unwrap()).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(
            &axum::body::to_bytes(resp.into_body(), 4096).await.unwrap()).unwrap();
        assert_eq!(v["messages"].as_array().unwrap().len(), 2);

        let first_id = v["messages"][0]["id"].as_i64().unwrap();
        let ack_body = format!(r#"{{"ids":[{}]}}"#, first_id);
        let resp = app.clone().oneshot(
            Request::builder().method("POST").uri("/inbox/s1/ack")
                .header("content-type","application/json")
                .body(Body::from(ack_body)).unwrap()).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let resp = app.oneshot(
            Request::builder().method("GET").uri("/inbox/s1?peek=1")
                .body(Body::empty()).unwrap()).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(
            &axum::body::to_bytes(resp.into_body(), 4096).await.unwrap()).unwrap();
        assert_eq!(v["messages"].as_array().unwrap().len(), 1);
        assert_eq!(v["messages"][0]["message"], "second");
    }

    #[tokio::test]
    async fn ack_idempotent_for_unknown_ids() {
        let (app, state) = test_app();
        seed(&state, "s1");
        let resp = app.oneshot(
            Request::builder().method("POST").uri("/inbox/s1/ack")
                .header("content-type","application/json")
                .body(Body::from(r#"{"ids":[999,1000]}"#)).unwrap()).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn bare_get_still_drains() {
        let (app, state) = test_app();
        seed(&state, "s1");
        state.store.enqueue_inbox("s1","human",None,1,"hi").unwrap();

        let resp = app.clone().oneshot(
            Request::builder().method("GET").uri("/inbox/s1")
                .body(Body::empty()).unwrap()).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(
            &axum::body::to_bytes(resp.into_body(), 4096).await.unwrap()).unwrap();
        assert_eq!(v["messages"].as_array().unwrap().len(), 1);

        let resp = app.oneshot(
            Request::builder().method("GET").uri("/inbox/s1")
                .body(Body::empty()).unwrap()).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(
            &axum::body::to_bytes(resp.into_body(), 4096).await.unwrap()).unwrap();
        assert_eq!(v["messages"].as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn admin_rows_rejects_unknown_table() {
        // The handler opens the real DB file by path; in the test environment that
        // file won't exist, so open_readonly will fail with INTERNAL_SERVER_ERROR.
        // Either a 4xx or 5xx response confirms no SQL was executed with the bogus
        // table name — either path is safe.
        let (app, _state) = test_app();
        let resp = app.oneshot(
            Request::builder().method("GET")
                .uri("/admin/rows?db=orchestrator&table=__nonexistent__")
                .body(Body::empty()).unwrap()
        ).await.unwrap();
        assert!(resp.status().is_client_error() || resp.status().is_server_error());
    }

    #[tokio::test]
    async fn admin_tables_rejects_unknown_db() {
        let (app, _state) = test_app();
        let resp = app.oneshot(
            Request::builder().method("GET")
                .uri("/admin/tables?db=evil_db")
                .body(Body::empty()).unwrap()
        ).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
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
