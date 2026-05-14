use rusqlite::Connection;

pub const SCHEMA_VERSION: i64 = 3;

pub fn migrate(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_version (version INTEGER NOT NULL);"
    )?;
    // Always run every upgrade step. Each is idempotent (column_exists / IF NOT EXISTS
    // guards), so re-running is cheap and lets us self-heal when the schema_version
    // row drifts from the actual schema (e.g. partial migration, killed mid-run).
    create_v1(conn)?;
    upgrade_v1_to_v2(conn)?;
    upgrade_v2_to_v3(conn)?;

    // Reconcile the version marker after the upgrades have taken effect.
    let current: i64 = conn
        .query_row("SELECT version FROM schema_version LIMIT 1", [], |r| r.get(0))
        .unwrap_or(-1);
    if current == -1 {
        conn.execute("INSERT INTO schema_version(version) VALUES (?)", [SCHEMA_VERSION])?;
    } else if current != SCHEMA_VERSION {
        conn.execute("UPDATE schema_version SET version=?", [SCHEMA_VERSION])?;
    }
    Ok(())
}

fn upgrade_v2_to_v3(conn: &Connection) -> rusqlite::Result<()> {
    if !column_exists(conn, "sessions", "loaded_snapshot")? {
        conn.execute("ALTER TABLE sessions ADD COLUMN loaded_snapshot TEXT", [])?;
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

#[cfg(test)]
mod migration_tests {
    use super::*;
    use rusqlite::Connection;

    #[test]
    fn fresh_db_has_loaded_snapshot_column() {
        let conn = Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();
        assert!(column_exists(&conn, "sessions", "loaded_snapshot").unwrap());
    }

    #[test]
    fn migrate_repairs_drifted_schema() {
        // Reproduce the production bug: schema_version says 3 but the column is missing.
        let conn = Connection::open_in_memory().unwrap();
        create_v1(&conn).unwrap();
        upgrade_v1_to_v2(&conn).unwrap();
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS schema_version (version INTEGER NOT NULL);"
        ).unwrap();
        conn.execute("INSERT INTO schema_version(version) VALUES (3)", []).unwrap();
        // Note: NO loaded_snapshot column yet.

        assert!(!column_exists(&conn, "sessions", "loaded_snapshot").unwrap(),
            "test precondition: column should not yet exist");

        migrate(&conn).unwrap();

        assert!(column_exists(&conn, "sessions", "loaded_snapshot").unwrap(),
            "migrate() must self-heal when schema_version drifts from real schema");
    }

    #[test]
    fn v2_db_upgrades_to_v3_idempotently() {
        let conn = Connection::open_in_memory().unwrap();
        create_v1(&conn).unwrap();
        upgrade_v1_to_v2(&conn).unwrap();
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS schema_version (version INTEGER NOT NULL);"
        ).unwrap();
        conn.execute("INSERT INTO schema_version(version) VALUES (2)", []).unwrap();

        migrate(&conn).unwrap();
        migrate(&conn).unwrap(); // idempotent

        assert!(column_exists(&conn, "sessions", "loaded_snapshot").unwrap());
        let v: i64 = conn
            .query_row("SELECT version FROM schema_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(v, 3);
    }
}

fn create_v1(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS sessions (
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
         CREATE INDEX IF NOT EXISTS idx_sessions_pid ON sessions(pid);
         CREATE TABLE IF NOT EXISTS events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
            ts INTEGER NOT NULL,
            kind TEXT NOT NULL,
            payload TEXT NOT NULL
         );
         CREATE INDEX IF NOT EXISTS idx_events_session_ts ON events(session_id, ts);
         CREATE TABLE IF NOT EXISTS artifacts (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
            ts INTEGER NOT NULL,
            path TEXT NOT NULL,
            label TEXT,
            kind TEXT NOT NULL
         );
         CREATE TABLE IF NOT EXISTS inbox (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
            from_kind TEXT NOT NULL CHECK (from_kind IN ('human','session')),
            from_id TEXT,
            ts INTEGER NOT NULL,
            message TEXT NOT NULL,
            delivered_at INTEGER
         );
         CREATE INDEX IF NOT EXISTS idx_inbox_session_undelivered
            ON inbox(session_id) WHERE delivered_at IS NULL;"
    )
}
