use rusqlite::Connection;

pub const SCHEMA_VERSION: i64 = 2;

pub fn migrate(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_version (version INTEGER NOT NULL);"
    )?;
    let current: i64 = conn
        .query_row("SELECT version FROM schema_version LIMIT 1", [], |r| r.get(0))
        .unwrap_or(0);
    let mut version = current;
    loop {
        match version {
            0 => { create_v1(conn)?; version = 1; }
            1 => { upgrade_v1_to_v2(conn)?; version = 2; }
            2 => break,
            other => panic!("unknown schema version: {other} — upgrade orchestrator binary"),
        }
    }
    if current == 0 {
        conn.execute("INSERT INTO schema_version(version) VALUES (?)", [SCHEMA_VERSION])?;
    } else if current != SCHEMA_VERSION {
        conn.execute("UPDATE schema_version SET version=?", [SCHEMA_VERSION])?;
    }
    Ok(())
}

fn upgrade_v1_to_v2(conn: &Connection) -> rusqlite::Result<()> {
    for col in &["transcript_path", "artifact_kind", "artifact_id", "artifact_title", "artifact_url"] {
        if !column_exists(conn, "sessions", col)? {
            conn.execute(&format!("ALTER TABLE sessions ADD COLUMN {} TEXT", col), [])?;
        }
    }
    Ok(())
}

fn column_exists(conn: &Connection, table: &str, col: &str) -> rusqlite::Result<bool> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({})", table))?;
    let exists = stmt.query_map([], |r| r.get::<_, String>(1))?
        .filter_map(|r| r.ok())
        .any(|name| name == col);
    Ok(exists)
}

fn create_v1(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE sessions (
            id TEXT PRIMARY KEY,
            label TEXT,
            cwd TEXT NOT NULL,
            pid INTEGER NOT NULL,
            status TEXT NOT NULL,
            current_tool TEXT,
            last_progress TEXT,
            last_user_prompt TEXT,
            started_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            ended_at INTEGER,
            end_reason TEXT
         );
         CREATE INDEX idx_sessions_pid ON sessions(pid);
         CREATE TABLE events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
            ts INTEGER NOT NULL,
            kind TEXT NOT NULL,
            payload TEXT NOT NULL
         );
         CREATE INDEX idx_events_session_ts ON events(session_id, ts);
         CREATE TABLE artifacts (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
            ts INTEGER NOT NULL,
            path TEXT NOT NULL,
            label TEXT,
            kind TEXT NOT NULL
         );
         CREATE TABLE inbox (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
            from_kind TEXT NOT NULL CHECK (from_kind IN ('human','session')),
            from_id TEXT,
            ts INTEGER NOT NULL,
            message TEXT NOT NULL,
            delivered_at INTEGER
         );
         CREATE INDEX idx_inbox_session_undelivered
            ON inbox(session_id) WHERE delivered_at IS NULL;"
    )
}
