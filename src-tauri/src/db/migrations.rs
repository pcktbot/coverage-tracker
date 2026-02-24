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
    Ok(())
}
