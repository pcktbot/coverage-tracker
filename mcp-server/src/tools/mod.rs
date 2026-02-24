use anyhow::Result;
use rusqlite::Connection;
use serde_json::{json, Value};
use std::path::PathBuf;

pub fn open_db() -> Result<Connection> {
    let path = if let Ok(p) = std::env::var("COVERAGE_DB_PATH") {
        PathBuf::from(p)
    } else {
        dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("coverage-manager")
            .join("coverage.db")
    };
    let conn = Connection::open(&path)?;
    conn.execute_batch("PRAGMA foreign_keys=ON;")?;
    Ok(conn)
}

pub fn list_repos(conn: &Connection, args: &Value) -> Result<Value> {
    let org_filter = args.get("org").and_then(|v| v.as_str());

    let sql = if org_filter.is_some() {
        "SELECT r.id, r.org, r.name, r.ruby_version,
                cr.overall_coverage, cr.started_at, cr.status
         FROM repos r
         LEFT JOIN coverage_runs cr ON cr.id = (
             SELECT id FROM coverage_runs WHERE repo_id = r.id AND status = 'success'
             ORDER BY started_at DESC LIMIT 1
         )
         WHERE r.org = ?1 AND r.enabled = 1
         ORDER BY r.name"
    } else {
        "SELECT r.id, r.org, r.name, r.ruby_version,
                cr.overall_coverage, cr.started_at, cr.status
         FROM repos r
         LEFT JOIN coverage_runs cr ON cr.id = (
             SELECT id FROM coverage_runs WHERE repo_id = r.id AND status = 'success'
             ORDER BY started_at DESC LIMIT 1
         )
         WHERE r.enabled = 1
         ORDER BY r.org, r.name"
    };

    let mut repos = Vec::new();
    if let Some(org) = org_filter {
        let mut stmt = conn.prepare(sql)?;
        let rows = stmt.query_map(rusqlite::params![org], map_repo_row)?;
        for r in rows { repos.push(r?); }
    } else {
        let mut stmt = conn.prepare(sql)?;
        let rows = stmt.query_map([], map_repo_row)?;
        for r in rows { repos.push(r?); }
    }
    Ok(json!(repos))
}

fn map_repo_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Value> {
    Ok(json!({
        "id": row.get::<_, i64>(0)?,
        "org": row.get::<_, String>(1)?,
        "name": row.get::<_, String>(2)?,
        "ruby_version": row.get::<_, Option<String>>(3)?,
        "latest_coverage": row.get::<_, Option<f64>>(4)?,
        "last_run_at": row.get::<_, Option<String>>(5)?,
        "last_run_status": row.get::<_, Option<String>>(6)?,
    }))
}

pub fn get_coverage_summary(conn: &Connection, args: &Value) -> Result<Value> {
    let repo_name = args.get("repo_name").and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("repo_name is required"))?;
    let org = args.get("org").and_then(|v| v.as_str());

    let repo_id: i64 = if let Some(org) = org {
        conn.query_row(
            "SELECT id FROM repos WHERE name = ?1 AND org = ?2",
            rusqlite::params![repo_name, org],
            |r| r.get(0),
        )?
    } else {
        conn.query_row(
            "SELECT id FROM repos WHERE name = ?1 LIMIT 1",
            rusqlite::params![repo_name],
            |r| r.get(0),
        )?
    };

    let result = conn.query_row(
        "SELECT id, started_at, completed_at, status, overall_coverage, lines_covered, lines_total, error_message
         FROM coverage_runs WHERE repo_id = ?1 ORDER BY started_at DESC LIMIT 1",
        rusqlite::params![repo_id],
        |row| Ok(json!({
            "run_id": row.get::<_, i64>(0)?,
            "started_at": row.get::<_, String>(1)?,
            "completed_at": row.get::<_, Option<String>>(2)?,
            "status": row.get::<_, String>(3)?,
            "overall_coverage": row.get::<_, Option<f64>>(4)?,
            "lines_covered": row.get::<_, Option<i64>>(5)?,
            "lines_total": row.get::<_, Option<i64>>(6)?,
            "error_message": row.get::<_, Option<String>>(7)?,
        })),
    )?;
    Ok(result)
}

pub fn get_coverage_trend(conn: &Connection, args: &Value) -> Result<Value> {
    let repo_name = args.get("repo_name").and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("repo_name is required"))?;
    let limit = args.get("limit").and_then(|v| v.as_i64()).unwrap_or(20);

    let repo_id: i64 = conn.query_row(
        "SELECT id FROM repos WHERE name = ?1 LIMIT 1",
        rusqlite::params![repo_name],
        |r| r.get(0),
    )?;

    let mut stmt = conn.prepare(
        "SELECT started_at, overall_coverage
         FROM coverage_runs WHERE repo_id = ?1 AND status = 'success'
         ORDER BY started_at DESC LIMIT ?2"
    )?;
    let mut points = Vec::new();
    let rows = stmt.query_map(rusqlite::params![repo_id, limit], |row| {
        Ok(json!({
            "date": row.get::<_, String>(0)?,
            "coverage": row.get::<_, Option<f64>>(1)?,
        }))
    })?;
    for r in rows { points.push(r?); }
    points.reverse(); // chronological
    Ok(json!(points))
}

pub fn search_file_coverage(conn: &Connection, args: &Value) -> Result<Value> {
    let pattern = args.get("pattern").and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("pattern is required"))?;
    let repo_filter = args.get("repo_name").and_then(|v| v.as_str());

    let like_pattern = format!("%{}%", pattern);
    let mut results = Vec::new();

    if let Some(repo_name) = repo_filter {
        let repo_id: i64 = conn.query_row(
            "SELECT id FROM repos WHERE name = ?1 LIMIT 1",
            rusqlite::params![repo_name],
            |r| r.get(0),
        )?;
        let mut stmt = conn.prepare(
            "SELECT fc.file_path, fc.coverage_percent, fc.lines_covered, fc.lines_total,
                    cr.started_at, r.name, r.org
             FROM file_coverage fc
             JOIN coverage_runs cr ON cr.id = fc.run_id
             JOIN repos r ON r.id = cr.repo_id
             WHERE cr.repo_id = ?1 AND fc.file_path LIKE ?2
               AND cr.id = (SELECT id FROM coverage_runs WHERE repo_id = ?1 AND status='success'
                            ORDER BY started_at DESC LIMIT 1)
             ORDER BY fc.file_path"
        )?;
        let rows = stmt.query_map(rusqlite::params![repo_id, like_pattern], map_file_row)?;
        let r: Vec<_> = rows.collect::<Result<Vec<_>, _>>()?;
        results.extend(r);
    } else {
        let mut stmt = conn.prepare(
            "SELECT fc.file_path, fc.coverage_percent, fc.lines_covered, fc.lines_total,
                    cr.started_at, r.name, r.org
             FROM file_coverage fc
             JOIN coverage_runs cr ON cr.id = fc.run_id
             JOIN repos r ON r.id = cr.repo_id
             WHERE fc.file_path LIKE ?1
               AND cr.id = (SELECT id FROM coverage_runs WHERE repo_id = r.id AND status='success'
                            ORDER BY started_at DESC LIMIT 1)
             ORDER BY r.org, r.name, fc.file_path"
        )?;
        let rows = stmt.query_map(rusqlite::params![like_pattern], map_file_row)?;
        let r: Vec<_> = rows.collect::<Result<Vec<_>, _>>()?;
        results.extend(r);
    }

    Ok(json!(results))
}

fn map_file_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Value> {
    Ok(json!({
        "file_path": row.get::<_, String>(0)?,
        "coverage_percent": row.get::<_, Option<f64>>(1)?,
        "lines_covered": row.get::<_, Option<i64>>(2)?,
        "lines_total": row.get::<_, Option<i64>>(3)?,
        "run_date": row.get::<_, String>(4)?,
        "repo": row.get::<_, String>(5)?,
        "org": row.get::<_, String>(6)?,
    }))
}
