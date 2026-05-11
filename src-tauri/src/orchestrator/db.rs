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
