use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::Connection;
use serde_json::{json, Value};
use std::fs;
use std::path::{Component, Path, PathBuf};

const MAX_DOC_SIZE_BYTES: u64 = 512 * 1024;
const IGNORED_DIRS: &[&str] = &[
    ".git",
    ".next",
    ".svelte-kit",
    "build",
    "coverage",
    "dist",
    "node_modules",
    "target",
    "tmp",
    "vendor",
];

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

pub fn list_repo_docs(conn: &Connection, args: &Value) -> Result<Value> {
    let root = resolve_repo_root(
        conn,
        required_str(args, "repo_name")?,
        args.get("org").and_then(|v| v.as_str()),
    )?;
    let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("");
    let runbooks_only = args.get("runbooks_only").and_then(|v| v.as_bool()).unwrap_or(false);
    let docs = scan_repo_docs(&root, query, runbooks_only)?;
    Ok(json!(docs))
}

pub fn read_repo_doc(conn: &Connection, args: &Value) -> Result<Value> {
    let root = resolve_repo_root(
        conn,
        required_str(args, "repo_name")?,
        args.get("org").and_then(|v| v.as_str()),
    )?;
    let path = required_str(args, "path")?;
    Ok(read_doc(&root, path)?)
}

pub fn search_repo_docs(conn: &Connection, args: &Value) -> Result<Value> {
    let pattern = required_str(args, "pattern")?;
    let runbooks_only = args.get("runbooks_only").and_then(|v| v.as_bool()).unwrap_or(false);
    let repo_name = args.get("repo_name").and_then(|v| v.as_str());
    let org = args.get("org").and_then(|v| v.as_str());

    if let Some(repo_name) = repo_name {
        let root = resolve_repo_root(conn, repo_name, org)?;
        let docs = scan_repo_docs(&root, pattern, runbooks_only)?;
        return Ok(json!(docs));
    }

    let mut stmt = conn.prepare(
        "SELECT org, name, local_path
         FROM repos
         WHERE local_path IS NOT NULL
         ORDER BY org, name"
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
        ))
    })?;

    let mut results = Vec::new();
    for row in rows {
        let (repo_org, repo_name, local_path) = row?;
        let root = PathBuf::from(local_path);
        for doc in scan_repo_docs(&root, pattern, runbooks_only)? {
            results.push(json!({
                "org": repo_org,
                "repo": repo_name,
                "path": doc.get("path").cloned().unwrap_or(Value::Null),
                "title": doc.get("title").cloned().unwrap_or(Value::Null),
                "preview": doc.get("preview").cloned().unwrap_or(Value::Null),
                "is_runbook": doc.get("is_runbook").cloned().unwrap_or(Value::Bool(false)),
                "modified_at": doc.get("modified_at").cloned().unwrap_or(Value::Null),
            }));
        }
    }

    Ok(json!(results))
}

fn required_str<'a>(args: &'a Value, key: &str) -> Result<&'a str> {
    args.get(key)
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("{key} is required"))
}

fn resolve_repo_root(conn: &Connection, repo_name: &str, org: Option<&str>) -> Result<PathBuf> {
    let local_path: String = if let Some(org) = org {
        conn.query_row(
            "SELECT local_path FROM repos WHERE name = ?1 AND org = ?2 AND local_path IS NOT NULL LIMIT 1",
            rusqlite::params![repo_name, org],
            |r| r.get(0),
        )?
    } else {
        conn.query_row(
            "SELECT local_path FROM repos WHERE name = ?1 AND local_path IS NOT NULL LIMIT 1",
            rusqlite::params![repo_name],
            |r| r.get(0),
        )?
    };
    Ok(PathBuf::from(local_path))
}

fn scan_repo_docs(root: &Path, query: &str, runbooks_only: bool) -> Result<Vec<Value>> {
    let root = root.canonicalize()?;
    let mut files = Vec::new();
    collect_markdown_files(&root, &root, &mut files)?;
    let query = query.trim().to_lowercase();
    let mut docs = Vec::new();

    for path in files {
        let rel = relative_path(&root, &path);
        let is_runbook = classify_runbook(&rel);
        if runbooks_only && !is_runbook {
            continue;
        }

        let content = match fs::read_to_string(&path) {
            Ok(content) => content,
            Err(_) => continue,
        };
        let title = extract_title(&rel, &content);
        if !query.is_empty() {
            let haystack = format!(
                "{}\n{}\n{}",
                rel.to_lowercase(),
                title.to_lowercase(),
                content.to_lowercase()
            );
            if !haystack.contains(&query) {
                continue;
            }
        }

        let modified_at = fs::metadata(&path)
            .ok()
            .and_then(|meta| meta.modified().ok())
            .map(|time| DateTime::<Utc>::from(time).to_rfc3339());

        docs.push(json!({
            "path": rel,
            "title": title,
            "preview": build_preview(&content, &query),
            "is_runbook": is_runbook,
            "modified_at": modified_at,
        }));
    }

    docs.sort_by(|a, b| {
        let a_runbook = a.get("is_runbook").and_then(|v| v.as_bool()).unwrap_or(false);
        let b_runbook = b.get("is_runbook").and_then(|v| v.as_bool()).unwrap_or(false);
        let a_path = a.get("path").and_then(|v| v.as_str()).unwrap_or("");
        let b_path = b.get("path").and_then(|v| v.as_str()).unwrap_or("");
        b_runbook.cmp(&a_runbook)
            .then_with(|| a_path.len().cmp(&b_path.len()))
            .then_with(|| a_path.cmp(b_path))
    });

    Ok(docs)
}

fn read_doc(root: &Path, relative: &str) -> Result<Value> {
    let root = root.canonicalize()?;
    let safe_relative = sanitize_relative_path(relative)?;
    let path = root.join(&safe_relative);
    let canonical = path.canonicalize()?;
    if !canonical.starts_with(&root) {
        anyhow::bail!("Doc path escapes repo root");
    }

    let markdown = fs::read_to_string(&canonical)?;
    let rel = relative_path(&root, &canonical);
    let modified_at = fs::metadata(&canonical)
        .ok()
        .and_then(|meta| meta.modified().ok())
        .map(|time| DateTime::<Utc>::from(time).to_rfc3339());

    Ok(json!({
        "path": rel,
        "title": extract_title(&rel, &markdown),
        "markdown": markdown,
        "is_runbook": classify_runbook(&rel),
        "modified_at": modified_at,
    }))
}

fn collect_markdown_files(root: &Path, current: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;

        if file_type.is_dir() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if IGNORED_DIRS.iter().any(|ignored| ignored.eq_ignore_ascii_case(&name)) {
                continue;
            }
            collect_markdown_files(root, &path, files)?;
            continue;
        }

        if !file_type.is_file() || !is_markdown_file(&path) {
            continue;
        }

        let metadata = match entry.metadata() {
            Ok(meta) => meta,
            Err(_) => continue,
        };
        if metadata.len() > MAX_DOC_SIZE_BYTES {
            continue;
        }
        if path.strip_prefix(root).is_ok() {
            files.push(path);
        }
    }
    Ok(())
}

fn is_markdown_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| matches!(ext.to_ascii_lowercase().as_str(), "md" | "mdx" | "markdown"))
        .unwrap_or(false)
}

fn classify_runbook(relative: &str) -> bool {
    let lower = relative.to_lowercase();
    lower.contains("runbook") || lower.contains("/runbooks/") || lower.starts_with("runbooks/")
}

fn extract_title(relative: &str, content: &str) -> String {
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(title) = trimmed.strip_prefix("# ") {
            if !title.trim().is_empty() {
                return title.trim().to_string();
            }
        }
    }
    Path::new(relative)
        .file_stem()
        .and_then(|name| name.to_str())
        .map(|name| name.replace(['-', '_'], " "))
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| relative.to_string())
}

fn build_preview(content: &str, query: &str) -> String {
    if !query.is_empty() {
        for line in content.lines() {
            let trimmed = line.trim();
            if !trimmed.is_empty() && trimmed.to_lowercase().contains(query) {
                return trimmed.chars().take(180).collect();
            }
        }
    }
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        return trimmed.chars().take(180).collect();
    }
    "No preview available.".into()
}

fn relative_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .components()
        .filter_map(|component| match component {
            Component::Normal(part) => Some(part.to_string_lossy().to_string()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn sanitize_relative_path(path: &str) -> Result<PathBuf> {
    let candidate = PathBuf::from(path);
    if candidate.is_absolute() {
        anyhow::bail!("Absolute doc paths are not allowed");
    }
    for component in candidate.components() {
        if matches!(component, Component::ParentDir | Component::RootDir | Component::Prefix(_)) {
            anyhow::bail!("Invalid doc path");
        }
    }
    Ok(candidate)
}
