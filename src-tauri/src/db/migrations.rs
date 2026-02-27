use rusqlite::Connection;
use anyhow::Result;

pub fn run(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS settings (
            key   TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS orgs (
            id         INTEGER PRIMARY KEY AUTOINCREMENT,
            name       TEXT NOT NULL UNIQUE,
            is_active  INTEGER NOT NULL DEFAULT 0
        );

        CREATE TABLE IF NOT EXISTS repos (
            id             INTEGER PRIMARY KEY AUTOINCREMENT,
            org            TEXT NOT NULL,
            name           TEXT NOT NULL,
            github_url     TEXT NOT NULL,
            local_path     TEXT,
            ruby_version   TEXT,
            enabled        INTEGER NOT NULL DEFAULT 1,
            last_synced_at TEXT,
            UNIQUE(org, name)
        );

        CREATE TABLE IF NOT EXISTS coverage_runs (
            id               INTEGER PRIMARY KEY AUTOINCREMENT,
            repo_id          INTEGER NOT NULL REFERENCES repos(id) ON DELETE CASCADE,
            started_at       TEXT NOT NULL,
            completed_at     TEXT,
            status           TEXT NOT NULL DEFAULT 'running',
            error_message    TEXT,
            overall_coverage REAL,
            lines_covered    INTEGER,
            lines_total      INTEGER
        );

        CREATE TABLE IF NOT EXISTS file_coverage (
            id               INTEGER PRIMARY KEY AUTOINCREMENT,
            run_id           INTEGER NOT NULL REFERENCES coverage_runs(id) ON DELETE CASCADE,
            file_path        TEXT NOT NULL,
            coverage_percent REAL,
            lines_covered    INTEGER,
            lines_total      INTEGER
        );

        CREATE INDEX IF NOT EXISTS idx_coverage_runs_repo
            ON coverage_runs(repo_id, started_at DESC);
        CREATE INDEX IF NOT EXISTS idx_file_coverage_run
            ON file_coverage(run_id);
        ",
    )?;

    // Migration: add node_version column if not present
    let has_node_version: bool = conn
        .prepare("PRAGMA table_info(repos)")
        .and_then(|mut stmt| {
            let names: Vec<String> = stmt
                .query_map([], |row| row.get::<_, String>(1))?
                .filter_map(|r| r.ok())
                .collect();
            Ok(names.contains(&"node_version".to_string()))
        })
        .unwrap_or(false);

    if !has_node_version {
        conn.execute_batch(
            "ALTER TABLE repos ADD COLUMN node_version TEXT;"
        )?;
    }

    // Migration: add uncovered_lines column if not present
    let has_col: bool = conn
        .prepare("PRAGMA table_info(file_coverage)")
        .and_then(|mut stmt| {
            let names: Vec<String> = stmt
                .query_map([], |row| row.get::<_, String>(1))?
                .filter_map(|r| r.ok())
                .collect();
            Ok(names.contains(&"uncovered_lines".to_string()))
        })
        .unwrap_or(false);

    if !has_col {
        conn.execute_batch(
            "ALTER TABLE file_coverage ADD COLUMN uncovered_lines TEXT DEFAULT '[]';"
        )?;
    }

    // Migration: create runtime_eol table for caching end-of-life data
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS runtime_eol (
            id           INTEGER PRIMARY KEY AUTOINCREMENT,
            runtime      TEXT NOT NULL,          -- 'nodejs' or 'ruby'
            cycle        TEXT NOT NULL,           -- e.g. '18', '20', '3.1'
            release_date TEXT,                    -- YYYY-MM-DD
            eol_date     TEXT,                    -- YYYY-MM-DD or NULL if not yet EOL
            lts_date     TEXT,                    -- YYYY-MM-DD when LTS started, NULL if never
            latest       TEXT,                    -- latest patch version in this cycle
            is_eol       INTEGER NOT NULL DEFAULT 0,
            UNIQUE(runtime, cycle)
        );

        CREATE TABLE IF NOT EXISTS runtime_eol_meta (
            runtime      TEXT PRIMARY KEY,
            last_fetched TEXT NOT NULL             -- ISO-8601 timestamp of last API fetch
        );
        ",
    )?;

    Ok(())
}
