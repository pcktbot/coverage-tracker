use crate::commands::repos::{with_db, ApiResult, DbState};
use crate::db::repos as db_repos;
use chrono::{DateTime, Utc};
use serde::Serialize;
use std::fs;
use std::path::{Component, Path, PathBuf};
use tauri::State;

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

#[derive(Debug, Clone, Serialize)]
pub struct RepoDocSummary {
    pub path: String,
    pub title: String,
    pub preview: String,
    pub is_runbook: bool,
    pub modified_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RepoDocContent {
    pub path: String,
    pub title: String,
    pub markdown: String,
    pub is_runbook: bool,
    pub modified_at: Option<String>,
}

#[tauri::command]
pub async fn list_repo_docs(
    state: State<'_, DbState>,
    repo_id: i64,
    query: Option<String>,
    runbooks_only: Option<bool>,
) -> Result<ApiResult<Vec<RepoDocSummary>>, String> {
    let root = match repo_root(&state, repo_id).await? {
        Some(root) => root,
        None => return Ok(ApiResult::err("Repo has not been cloned yet.")),
    };

    let query = query.unwrap_or_default();
    let runbooks_only = runbooks_only.unwrap_or(false);
    let docs = tokio::task::spawn_blocking(move || scan_repo_docs(&root, &query, runbooks_only))
        .await
        .map_err(|e| e.to_string())?;

    match docs {
        Ok(items) => Ok(ApiResult::ok(items)),
        Err(err) => Ok(ApiResult::err(err)),
    }
}

#[tauri::command]
pub async fn read_repo_doc(
    state: State<'_, DbState>,
    repo_id: i64,
    path: String,
) -> Result<ApiResult<RepoDocContent>, String> {
    let root = match repo_root(&state, repo_id).await? {
        Some(root) => root,
        None => return Ok(ApiResult::err("Repo has not been cloned yet.")),
    };

    let doc = tokio::task::spawn_blocking(move || read_doc(&root, &path))
        .await
        .map_err(|e| e.to_string())?;

    match doc {
        Ok(item) => Ok(ApiResult::ok(item)),
        Err(err) => Ok(ApiResult::err(err)),
    }
}

async fn repo_root(state: &State<'_, DbState>, repo_id: i64) -> Result<Option<PathBuf>, String> {
    with_db(&state.0, move |conn| -> Result<Option<PathBuf>, String> {
        match db_repos::list_repos(conn, None) {
            Ok(repos) => repos
                .into_iter()
                .find(|repo| repo.id == repo_id)
                .map(|repo| repo.local_path.map(PathBuf::from))
                .ok_or_else(|| format!("Repo {repo_id} not found")),
            Err(err) => Err(err.to_string()),
        }
    })
    .await
    ?
}

fn scan_repo_docs(root: &Path, query: &str, runbooks_only: bool) -> Result<Vec<RepoDocSummary>, String> {
    let root = root
        .canonicalize()
        .map_err(|e| format!("Failed to open repo: {e}"))?;

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
        let preview = build_preview(&content, &query);

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

        docs.push(RepoDocSummary {
            path: rel,
            title,
            preview,
            is_runbook,
            modified_at,
        });
    }

    docs.sort_by(|a, b| {
        b.is_runbook
            .cmp(&a.is_runbook)
            .then_with(|| a.path.len().cmp(&b.path.len()))
            .then_with(|| a.path.cmp(&b.path))
    });

    Ok(docs)
}

fn read_doc(root: &Path, relative: &str) -> Result<RepoDocContent, String> {
    let root = root
        .canonicalize()
        .map_err(|e| format!("Failed to open repo: {e}"))?;
    let safe_relative = sanitize_relative_path(relative)?;
    let path = root.join(&safe_relative);
    let canonical = path
        .canonicalize()
        .map_err(|e| format!("Failed to open doc: {e}"))?;

    if !canonical.starts_with(&root) {
        return Err("Doc path escapes repo root.".into());
    }

    let markdown = fs::read_to_string(&canonical).map_err(|e| format!("Failed to read doc: {e}"))?;
    let rel = relative_path(&root, &canonical);
    let title = extract_title(&rel, &markdown);
    let is_runbook = classify_runbook(&rel);
    let modified_at = fs::metadata(&canonical)
        .ok()
        .and_then(|meta| meta.modified().ok())
        .map(|time| DateTime::<Utc>::from(time).to_rfc3339());

    Ok(RepoDocContent {
        path: rel,
        title,
        markdown,
        is_runbook,
        modified_at,
    })
}

fn collect_markdown_files(root: &Path, current: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries = fs::read_dir(current).map_err(|e| format!("Failed to read docs directory: {e}"))?;

    for entry in entries {
        let entry = entry.map_err(|e| format!("Failed to inspect docs entry: {e}"))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|e| format!("Failed to inspect docs entry type: {e}"))?;

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

fn sanitize_relative_path(path: &str) -> Result<PathBuf, String> {
    let candidate = PathBuf::from(path);
    if candidate.is_absolute() {
        return Err("Absolute doc paths are not allowed.".into());
    }

    for component in candidate.components() {
        if matches!(component, Component::ParentDir | Component::RootDir | Component::Prefix(_)) {
            return Err("Invalid doc path.".into());
        }
    }

    Ok(candidate)
}
