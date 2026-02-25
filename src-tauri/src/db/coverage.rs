use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use anyhow::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoverageRun {
    pub id: i64,
    pub repo_id: i64,
    pub started_at: String,
    pub completed_at: Option<String>,
    pub status: String,
    pub error_message: Option<String>,
    pub overall_coverage: Option<f64>,
    pub lines_covered: Option<i64>,
    pub lines_total: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileCoverage {
    pub id: i64,
    pub run_id: i64,
    pub file_path: String,
    pub coverage_percent: Option<f64>,
    pub lines_covered: Option<i64>,
    pub lines_total: Option<i64>,
    pub uncovered_lines: Vec<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoverageTrendPoint {
    pub run_id: i64,
    pub started_at: String,
    pub overall_coverage: Option<f64>,
    pub status: String,
}

pub fn start_run(conn: &Connection, repo_id: i64) -> Result<i64> {
    conn.execute(
        "INSERT INTO coverage_runs (repo_id, started_at, status)
         VALUES (?1, datetime('now'), 'running')",
        params![repo_id],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn finish_run(
    conn: &Connection,
    run_id: i64,
    status: &str,
    error_message: Option<&str>,
    overall_coverage: Option<f64>,
    lines_covered: Option<i64>,
    lines_total: Option<i64>,
) -> Result<()> {
    conn.execute(
        "UPDATE coverage_runs
         SET completed_at = datetime('now'), status = ?1, error_message = ?2,
             overall_coverage = ?3, lines_covered = ?4, lines_total = ?5
         WHERE id = ?6",
        params![status, error_message, overall_coverage, lines_covered, lines_total, run_id],
    )?;
    Ok(())
}

pub fn insert_file_coverage(
    conn: &Connection,
    run_id: i64,
    file_path: &str,
    coverage_percent: Option<f64>,
    lines_covered: Option<i64>,
    lines_total: Option<i64>,
    uncovered_lines: &[usize],
) -> Result<()> {
    let uncovered_json = serde_json::to_string(uncovered_lines).unwrap_or_else(|_| "[]".to_string());
    conn.execute(
        "INSERT INTO file_coverage (run_id, file_path, coverage_percent, lines_covered, lines_total, uncovered_lines)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![run_id, file_path, coverage_percent, lines_covered, lines_total, uncovered_json],
    )?;
    Ok(())
}

pub fn list_runs(conn: &Connection, repo_id: i64) -> Result<Vec<CoverageRun>> {
    let mut stmt = conn.prepare(
        "SELECT id, repo_id, started_at, completed_at, status, error_message,
                overall_coverage, lines_covered, lines_total
         FROM coverage_runs WHERE repo_id = ?1 ORDER BY started_at DESC"
    )?;
    let rows = stmt.query_map(params![repo_id], |row| {
        Ok(CoverageRun {
            id: row.get(0)?,
            repo_id: row.get(1)?,
            started_at: row.get(2)?,
            completed_at: row.get(3)?,
            status: row.get(4)?,
            error_message: row.get(5)?,
            overall_coverage: row.get(6)?,
            lines_covered: row.get(7)?,
            lines_total: row.get(8)?,
        })
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

pub fn get_trend(conn: &Connection, repo_id: i64, limit: i64) -> Result<Vec<CoverageTrendPoint>> {
    let mut stmt = conn.prepare(
        "SELECT id, started_at, overall_coverage, status
         FROM coverage_runs
         WHERE repo_id = ?1 AND status = 'success'
         ORDER BY started_at DESC LIMIT ?2"
    )?;
    let rows = stmt.query_map(params![repo_id, limit], |row| {
        Ok(CoverageTrendPoint {
            run_id: row.get(0)?,
            started_at: row.get(1)?,
            overall_coverage: row.get(2)?,
            status: row.get(3)?,
        })
    })?;
    let mut v: Vec<_> = rows.collect::<Result<Vec<_>, _>>()?;
    v.reverse(); // chronological order for charts
    Ok(v)
}

pub fn get_file_coverage(conn: &Connection, run_id: i64) -> Result<Vec<FileCoverage>> {
    let mut stmt = conn.prepare(
        "SELECT id, run_id, file_path, coverage_percent, lines_covered, lines_total, uncovered_lines
         FROM file_coverage WHERE run_id = ?1 ORDER BY file_path"
    )?;
    let rows = stmt.query_map(params![run_id], |row| {
        let uncovered_json: String = row.get::<_, Option<String>>(6)?.unwrap_or_else(|| "[]".to_string());
        let uncovered_lines: Vec<usize> = serde_json::from_str(&uncovered_json).unwrap_or_default();
        Ok(FileCoverage {
            id: row.get(0)?,
            run_id: row.get(1)?,
            file_path: row.get(2)?,
            coverage_percent: row.get(3)?,
            lines_covered: row.get(4)?,
            lines_total: row.get(5)?,
            uncovered_lines,
        })
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

/// Returns (repo_name, org, run_date, overall_pct, lines_covered, lines_total) for CSV export.
pub fn all_runs_for_export(
    conn: &Connection,
    repo_id: Option<i64>,
) -> Result<Vec<(String, String, String, Option<f64>, Option<i64>, Option<i64>)>> {
    let rows = if let Some(id) = repo_id {
        let mut stmt = conn.prepare(
            "SELECT r.name, r.org, cr.started_at, cr.overall_coverage, cr.lines_covered, cr.lines_total
             FROM coverage_runs cr JOIN repos r ON r.id = cr.repo_id
             WHERE cr.status = 'success' AND cr.repo_id = ?1
             ORDER BY cr.started_at DESC"
        )?;
        let r = stmt.query_map(params![id], map_export_row)?.collect::<Result<Vec<_>, _>>()?;
        r
    } else {
        let mut stmt = conn.prepare(
            "SELECT r.name, r.org, cr.started_at, cr.overall_coverage, cr.lines_covered, cr.lines_total
             FROM coverage_runs cr JOIN repos r ON r.id = cr.repo_id
             WHERE cr.status = 'success'
             ORDER BY r.org, r.name, cr.started_at DESC"
        )?;
        let r = stmt.query_map([], map_export_row)?.collect::<Result<Vec<_>, _>>()?;
        r
    };
    Ok(rows)
}

fn map_export_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<(String, String, String, Option<f64>, Option<i64>, Option<i64>)> {
    Ok((
        row.get(0)?,
        row.get(1)?,
        row.get(2)?,
        row.get(3)?,
        row.get(4)?,
        row.get(5)?,
    ))
}
