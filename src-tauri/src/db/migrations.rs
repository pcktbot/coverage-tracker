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

        CREATE TABLE IF NOT EXISTS repo_sources (
            repo_id                   INTEGER PRIMARY KEY REFERENCES repos(id) ON DELETE CASCADE,
            platform_name             TEXT,
            tfs_project               TEXT,
            tfs_area_path             TEXT,
            tfs_team                  TEXT,
            tfs_release_definition    TEXT,
            confluence_space_key      TEXT,
            confluence_parent_page_id TEXT,
            confluence_site_label     TEXT,
            notes                     TEXT
        );

        CREATE TABLE IF NOT EXISTS projects (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            name            TEXT NOT NULL,
            slug            TEXT NOT NULL UNIQUE,
            status          TEXT NOT NULL DEFAULT 'incoming',
            platform_name   TEXT,
            manual_priority INTEGER NOT NULL DEFAULT 3,
            notes           TEXT,
            ado_iteration_path TEXT,
            ado_team        TEXT,
            ado_states      TEXT NOT NULL DEFAULT '[]',
            ado_tag         TEXT,
            repo_links      TEXT NOT NULL DEFAULT '[]',
            teams_team_id   TEXT,
            teams_channel_id TEXT,
            teams_members   TEXT NOT NULL DEFAULT '[]',
            loop_workspace_id TEXT,
            loop_page_id    TEXT,
            is_active       INTEGER NOT NULL DEFAULT 1,
            created_at      TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at      TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );

        CREATE TABLE IF NOT EXISTS project_repos (
            project_id      INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
            repo_id         INTEGER NOT NULL REFERENCES repos(id) ON DELETE CASCADE,
            PRIMARY KEY (project_id, repo_id)
        );

        CREATE TABLE IF NOT EXISTS agent_profiles (
            id                        INTEGER PRIMARY KEY AUTOINCREMENT,
            name                      TEXT NOT NULL,
            goal                      TEXT NOT NULL DEFAULT '',
            instructions              TEXT NOT NULL DEFAULT '',
            source_types              TEXT NOT NULL DEFAULT '[]',
            project_scope             TEXT NOT NULL DEFAULT 'all_active_projects',
            weight_manual_priority    INTEGER NOT NULL DEFAULT 5,
            weight_release_risk       INTEGER NOT NULL DEFAULT 4,
            weight_doc_gap            INTEGER NOT NULL DEFAULT 2,
            weight_meeting_followup   INTEGER NOT NULL DEFAULT 3,
            is_active                 INTEGER NOT NULL DEFAULT 1,
            created_at                TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at                TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );

        CREATE TABLE IF NOT EXISTS confluence_pages (
            page_id          TEXT PRIMARY KEY,
            space_key        TEXT,
            title            TEXT NOT NULL,
            web_url          TEXT NOT NULL,
            version_number   INTEGER,
            last_updated_at  TEXT,
            last_fetched_at  TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            raw_html         TEXT NOT NULL,
            plain_text       TEXT NOT NULL,
            excerpt          TEXT NOT NULL
        );
        ",
    )?;

    let has_project_repo_links: bool = conn
        .prepare("PRAGMA table_info(projects)")
        .and_then(|mut stmt| {
            let names: Vec<String> = stmt
                .query_map([], |row| row.get::<_, String>(1))?
                .filter_map(|r| r.ok())
                .collect();
            Ok(names.contains(&"repo_links".to_string()))
        })
        .unwrap_or(false);

    if !has_project_repo_links {
        conn.execute_batch(
            "ALTER TABLE projects ADD COLUMN repo_links TEXT NOT NULL DEFAULT '[]';"
        )?;
    }

    for (column, sql) in [
        ("ado_iteration_path", "ALTER TABLE projects ADD COLUMN ado_iteration_path TEXT;"),
        ("ado_team", "ALTER TABLE projects ADD COLUMN ado_team TEXT;"),
        ("ado_states", "ALTER TABLE projects ADD COLUMN ado_states TEXT NOT NULL DEFAULT '[]';"),
        ("ado_tag", "ALTER TABLE projects ADD COLUMN ado_tag TEXT;"),
        ("teams_team_id", "ALTER TABLE projects ADD COLUMN teams_team_id TEXT;"),
        ("teams_channel_id", "ALTER TABLE projects ADD COLUMN teams_channel_id TEXT;"),
        ("teams_members", "ALTER TABLE projects ADD COLUMN teams_members TEXT NOT NULL DEFAULT '[]';"),
        ("loop_workspace_id", "ALTER TABLE projects ADD COLUMN loop_workspace_id TEXT;"),
        ("loop_page_id", "ALTER TABLE projects ADD COLUMN loop_page_id TEXT;"),
    ] {
        let has_column: bool = conn
            .prepare("PRAGMA table_info(projects)")
            .and_then(|mut stmt| {
                let names: Vec<String> = stmt
                    .query_map([], |row| row.get::<_, String>(1))?
                    .filter_map(|r| r.ok())
                    .collect();
                Ok(names.contains(&column.to_string()))
            })
            .unwrap_or(false);

        if !has_column {
            conn.execute_batch(sql)?;
        }
    }

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
