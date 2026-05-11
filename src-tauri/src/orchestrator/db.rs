use rusqlite::{Connection, OptionalExtension, params};
use std::path::Path;
use std::sync::{Arc, Mutex};
use crate::db::orchestrator_migrations;

pub struct Store { conn: Mutex<Connection> }

#[derive(Debug, Clone, serde::Serialize)]
pub struct SessionRow {
    pub id: String,
    pub label: Option<String>,
    pub cwd: String,
    pub pid: i64,
    pub status: String,
    pub current_tool: Option<String>,
    pub last_progress: Option<String>,
    pub last_user_prompt: Option<String>,
    pub started_at: i64,
    pub updated_at: i64,
    pub ended_at: Option<i64>,
    pub end_reason: Option<String>,
}

impl Store {
    pub fn open_at(path: &Path) -> rusqlite::Result<Self> {
        if let Some(parent) = path.parent() { let _ = std::fs::create_dir_all(parent); }
        let conn = Connection::open(path)?;
        orchestrator_migrations::migrate(&conn)?;
        Ok(Self { conn: Mutex::new(conn) })
    }

    pub fn open_in_memory() -> rusqlite::Result<Self> {
        let conn = Connection::open_in_memory()?;
        orchestrator_migrations::migrate(&conn)?;
        Ok(Self { conn: Mutex::new(conn) })
    }

    /// Run a blocking DB closure off the async runtime. Use this from axum handlers.
    ///
    /// Contract: a `JoinError` (DB closure panicked) is fatal — we panic the
    /// handler thread so the failure is visible. Callers map the inner
    /// `Result<_, rusqlite::Error>` returned by their own closure as usual.
    pub async fn run<F, T>(store: Arc<Self>, f: F) -> T
    where F: FnOnce(&Self) -> T + Send + 'static, T: Send + 'static
    {
        tokio::task::spawn_blocking(move || f(&store))
            .await
            .expect("orchestrator DB task panicked")
    }

    pub fn schema_version(&self) -> rusqlite::Result<i64> {
        let conn = self.conn.lock().unwrap();
        conn.query_row("SELECT version FROM schema_version LIMIT 1", [], |r| r.get(0))
    }

    pub fn upsert_session_start(
        &self, id: &str, label: Option<&str>, cwd: &str, pid: i64, now: i64,
    ) -> rusqlite::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO sessions(id,label,cwd,pid,status,started_at,updated_at)
             VALUES(?,?,?,?, 'working', ?, ?)
             ON CONFLICT(id) DO UPDATE SET label=excluded.label, cwd=excluded.cwd,
                pid=excluded.pid, status='working', updated_at=excluded.updated_at,
                ended_at=NULL, end_reason=NULL",
            params![id, label, cwd, pid, now, now],
        )?;
        Ok(())
    }

    pub fn get_session(&self, id: &str) -> rusqlite::Result<Option<SessionRow>> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT id,label,cwd,pid,status,current_tool,last_progress,last_user_prompt,
                    started_at,updated_at,ended_at,end_reason FROM sessions WHERE id=?",
            params![id],
            |r| Ok(SessionRow {
                id: r.get(0)?, label: r.get(1)?, cwd: r.get(2)?, pid: r.get(3)?,
                status: r.get(4)?, current_tool: r.get(5)?, last_progress: r.get(6)?,
                last_user_prompt: r.get(7)?, started_at: r.get(8)?, updated_at: r.get(9)?,
                ended_at: r.get(10)?, end_reason: r.get(11)?,
            }),
        ).optional()
    }

    pub fn enqueue_inbox(&self, sid: &str, from_kind: &str, from_id: Option<&str>,
                        ts: i64, message: &str) -> rusqlite::Result<i64> {
        let conn = self.conn.lock().unwrap();
        conn.execute("INSERT INTO inbox(session_id,from_kind,from_id,ts,message) VALUES(?,?,?,?,?)",
            params![sid, from_kind, from_id, ts, message])?;
        Ok(conn.last_insert_rowid())
    }

    pub fn lock_conn(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().unwrap()
    }

    pub fn record_event(&self, sid: &str, ts: i64, kind: &str, payload: &str) -> rusqlite::Result<i64> {
        let conn = self.conn.lock().unwrap();
        conn.execute("INSERT INTO events(session_id,ts,kind,payload) VALUES(?,?,?,?)",
            params![sid, ts, kind, payload])?;
        Ok(conn.last_insert_rowid())
    }
    pub fn apply_status(&self, sid: &str, new_status: &str, now: i64,
                        ended: Option<(i64, &str)>) -> rusqlite::Result<()> {
        let conn = self.conn.lock().unwrap();
        match ended {
            Some((ended_at, reason)) => conn.execute(
                "UPDATE sessions SET status=?, updated_at=?, ended_at=?, end_reason=? WHERE id=?",
                params![new_status, now, ended_at, reason, sid])?,
            None => conn.execute(
                "UPDATE sessions SET status=?, updated_at=? WHERE id=?",
                params![new_status, now, sid])?,
        };
        Ok(())
    }
    pub fn set_current_tool(&self, sid: &str, tool: &str, now: i64) -> rusqlite::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("UPDATE sessions SET current_tool=?, updated_at=? WHERE id=?",
            params![tool, now, sid])?;
        Ok(())
    }
    pub fn set_last_user_prompt(&self, sid: &str, prompt: &str, now: i64) -> rusqlite::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("UPDATE sessions SET last_user_prompt=?, updated_at=? WHERE id=?",
            params![prompt, now, sid])?;
        Ok(())
    }
}

#[derive(Debug, serde::Serialize)]
pub struct ArtifactRow {
    pub id: i64, pub session_id: String, pub ts: i64,
    pub path: String, pub label: Option<String>, pub kind: String,
}

#[derive(Debug, serde::Serialize)]
pub struct InboxRow {
    pub id: i64, pub from_kind: String, pub from_id: Option<String>,
    pub ts: i64, pub message: String,
}

impl Store {
    pub fn set_last_progress(&self, sid: &str, summary: &str, now: i64) -> rusqlite::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("UPDATE sessions SET last_progress=?, updated_at=? WHERE id=?",
            params![summary, now, sid])?;
        Ok(())
    }
    pub fn insert_artifact(&self, sid: &str, ts: i64, path: &str, label: Option<&str>)
        -> rusqlite::Result<i64> {
        let kind = if path.contains("/pull/") || path.contains("/pulls/") { "pr" }
                   else if path.starts_with("http") { "url" } else { "file" };
        let conn = self.conn.lock().unwrap();
        conn.execute("INSERT INTO artifacts(session_id,ts,path,label,kind) VALUES(?,?,?,?,?)",
            params![sid, ts, path, label, kind])?;
        Ok(conn.last_insert_rowid())
    }
    pub fn list_artifacts(&self, sid: &str) -> rusqlite::Result<Vec<ArtifactRow>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id,session_id,ts,path,label,kind FROM artifacts
             WHERE session_id=? ORDER BY ts DESC")?;
        let rows: rusqlite::Result<Vec<ArtifactRow>> = stmt.query_map(params![sid], |r| Ok(ArtifactRow {
            id: r.get(0)?, session_id: r.get(1)?, ts: r.get(2)?,
            path: r.get(3)?, label: r.get(4)?, kind: r.get(5)?,
        }))?.collect();
        rows
    }
    pub fn list_sessions(&self) -> rusqlite::Result<Vec<SessionRow>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id,label,cwd,pid,status,current_tool,last_progress,last_user_prompt,
                    started_at,updated_at,ended_at,end_reason
             FROM sessions ORDER BY updated_at DESC")?;
        let rows: Vec<SessionRow> = stmt.query_map([], |r| Ok(SessionRow {
            id: r.get(0)?, label: r.get(1)?, cwd: r.get(2)?, pid: r.get(3)?,
            status: r.get(4)?, current_tool: r.get(5)?, last_progress: r.get(6)?,
            last_user_prompt: r.get(7)?, started_at: r.get(8)?, updated_at: r.get(9)?,
            ended_at: r.get(10)?, end_reason: r.get(11)?,
        }))?.collect::<Result<Vec<_>,_>>()?;
        Ok(rows)
    }
    pub fn events_for(&self, sid: &str, limit: i64)
        -> rusqlite::Result<Vec<(i64,i64,String,String)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id,ts,kind,payload FROM events WHERE session_id=?
             ORDER BY ts DESC LIMIT ?")?;
        let rows: Vec<(i64,i64,String,String)> = stmt.query_map(params![sid, limit], |r|
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?.collect::<Result<Vec<_>,_>>()?;
        Ok(rows)
    }
    pub fn find_session_by_pid(&self, pid: i64) -> rusqlite::Result<Option<String>> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT id FROM sessions WHERE pid=? AND ended_at IS NULL
             ORDER BY started_at DESC LIMIT 1",
            params![pid], |r| r.get::<_, String>(0)).optional()
    }
    pub fn drain_inbox(&self, sid: &str, now: i64) -> rusqlite::Result<Vec<InboxRow>> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        let rows: Vec<InboxRow> = {
            let mut stmt = tx.prepare(
                "SELECT id,from_kind,from_id,ts,message FROM inbox
                 WHERE session_id=? AND delivered_at IS NULL ORDER BY ts ASC")?;
            let collected: Result<Vec<InboxRow>, rusqlite::Error> = stmt.query_map(params![sid], |r| Ok(InboxRow {
                id: r.get(0)?, from_kind: r.get(1)?, from_id: r.get(2)?,
                ts: r.get(3)?, message: r.get(4)?,
            }))?.collect();
            collected?
        };
        tx.execute("UPDATE inbox SET delivered_at=? WHERE session_id=? AND delivered_at IS NULL",
            params![now, sid])?;
        tx.commit()?;
        Ok(rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn mem_store() -> Arc<Store> {
        Arc::new(Store::open_in_memory().expect("open in-memory store"))
    }

    #[test]
    fn upsert_then_get_round_trips() {
        let store = mem_store();
        store.upsert_session_start("sess-1", Some("feature"), "/tmp/x", 42, 1).unwrap();
        let row = store.get_session("sess-1").unwrap().expect("row");
        assert_eq!(row.id, "sess-1");
        assert_eq!(row.status, "working");
    }

    #[test]
    fn schema_version_is_one() {
        let store = mem_store();
        assert_eq!(store.schema_version().unwrap(), 1);
    }

    #[test]
    fn foreign_keys_are_enforced() {
        let store = mem_store();
        let err = store.enqueue_inbox("missing", "human", None, 1, "x").unwrap_err();
        let msg = err.to_string().to_lowercase();
        assert!(msg.contains("foreign key") || msg.contains("constraint"), "got: {msg}");
    }

    #[tokio::test]
    async fn run_async_executes_off_runtime() {
        let store = mem_store();
        store.upsert_session_start("s1", None, "/", 1, 1).unwrap();
        let s = store.clone();
        let row = Store::run(s, |st| st.get_session("s1")).await.unwrap().unwrap();
        assert_eq!(row.id, "s1");
    }
}
