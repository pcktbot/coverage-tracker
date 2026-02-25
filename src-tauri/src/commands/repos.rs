use crate::db::repos as db_repos;
use crate::github::GithubClient;
use crate::git as git_ops;
use serde::Serialize;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, State};

pub struct DbState(pub std::sync::Arc<std::sync::Mutex<rusqlite::Connection>>);

#[derive(Serialize)]
pub struct ApiResult<T> {
    pub ok: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

impl<T: Serialize> ApiResult<T> {
    pub fn ok(data: T) -> Self {
        Self { ok: true, data: Some(data), error: None }
    }
    pub fn err(msg: impl ToString) -> Self {
        Self { ok: false, data: None, error: Some(msg.to_string()) }
    }
}

// ── Orgs ──────────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn list_orgs(state: State<DbState>) -> ApiResult<Vec<db_repos::Org>> {
    let conn = state.0.lock().unwrap();
    match db_repos::list_orgs(&conn) {
        Ok(orgs) => ApiResult::ok(orgs),
        Err(e) => ApiResult::err(e),
    }
}

#[tauri::command]
pub fn add_org(state: State<DbState>, name: String) -> ApiResult<()> {
    let conn = state.0.lock().unwrap();
    match db_repos::add_org(&conn, &name) {
        Ok(_) => ApiResult::ok(()),
        Err(e) => ApiResult::err(e),
    }
}

#[tauri::command]
pub fn remove_org(state: State<DbState>, name: String) -> ApiResult<()> {
    let conn = state.0.lock().unwrap();
    match db_repos::remove_org(&conn, &name) {
        Ok(_) => ApiResult::ok(()),
        Err(e) => ApiResult::err(e),
    }
}

#[tauri::command]
pub fn set_active_org(state: State<DbState>, name: String) -> ApiResult<()> {
    let conn = state.0.lock().unwrap();
    match db_repos::set_active_org(&conn, &name) {
        Ok(_) => ApiResult::ok(()),
        Err(e) => ApiResult::err(e),
    }
}

#[tauri::command]
pub fn get_active_org(state: State<DbState>) -> ApiResult<Option<String>> {
    let conn = state.0.lock().unwrap();
    match db_repos::get_active_org(&conn) {
        Ok(org) => ApiResult::ok(org),
        Err(e) => ApiResult::err(e),
    }
}

// ── Settings ──────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn get_settings(state: State<DbState>) -> ApiResult<serde_json::Value> {
    let conn = state.0.lock().unwrap();
    let token = db_repos::get_setting(&conn, "github_token").unwrap_or(None);
    let clone_root = db_repos::get_setting(&conn, "clone_root").unwrap_or(None);
    ApiResult::ok(serde_json::json!({
        "github_token": token.unwrap_or_default(),
        "clone_root": clone_root.unwrap_or_default(),
    }))
}

#[tauri::command]
pub fn save_settings(
    state: State<DbState>,
    github_token: String,
    clone_root: String,
) -> ApiResult<()> {
    let conn = state.0.lock().unwrap();
    if let Err(e) = db_repos::set_setting(&conn, "github_token", &github_token) {
        return ApiResult::err(e);
    }
    if let Err(e) = db_repos::set_setting(&conn, "clone_root", &clone_root) {
        return ApiResult::err(e);
    }
    ApiResult::ok(())
}

// ── Repos ─────────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn list_repos(state: State<DbState>, org: Option<String>) -> ApiResult<Vec<db_repos::Repo>> {
    let conn = state.0.lock().unwrap();
    match db_repos::list_repos(&conn, org.as_deref()) {
        Ok(repos) => ApiResult::ok(repos),
        Err(e) => ApiResult::err(e),
    }
}

#[tauri::command]
pub fn set_repo_enabled(state: State<DbState>, id: i64, enabled: bool) -> ApiResult<()> {
    let conn = state.0.lock().unwrap();
    match db_repos::set_repo_enabled(&conn, id, enabled) {
        Ok(_) => ApiResult::ok(()),
        Err(e) => ApiResult::err(e),
    }
}

/// Fetch repos for an org from GitHub and upsert into the DB.
/// Runs the blocking HTTP work on a background thread so the UI stays responsive.
/// Emits `sync-progress` events: `{ done: usize, total: usize, name: String }`.
#[tauri::command]
pub async fn sync_org_repos(
    app: AppHandle,
    state: State<'_, DbState>,
    org: String,
) -> Result<ApiResult<usize>, String> {
    let token = {
        let conn = state.0.lock().unwrap();
        match db_repos::get_setting(&conn, "github_token") {
            Ok(Some(t)) if !t.is_empty() => t,
            _ => return Ok(ApiResult::err("GitHub token not configured. Go to Settings.")),
        }
    };

    let org2 = org.clone();
    let token2 = token.clone();
    let repos = match tokio::task::spawn_blocking(move || {
        GithubClient::new(&token2).list_all_repos(&org2)
    })
    .await
    {
        Ok(Ok(r)) => r,
        Ok(Err(e)) => return Ok(ApiResult::err(e)),
        Err(e) => return Ok(ApiResult::err(format!("Task error: {}", e))),
    };

    let total = repos.len();
    // Hold the DB lock only for the batch insert, wrapped in a transaction.
    {
        let conn = state.0.lock().unwrap();
        let _ = conn.execute_batch("BEGIN");
        for r in repos.iter() {
            let _ = db_repos::upsert_repo(&conn, &org, &r.name, &r.clone_url);
        }
        let _ = conn.execute_batch("COMMIT");
    }
    // Emit progress events after releasing the lock.
    for (i, r) in repos.iter().enumerate() {
        let _ = app.emit(
            "sync-progress",
            serde_json::json!({ "done": i + 1, "total": total, "name": r.name }),
        );
    }
    Ok(ApiResult::ok(total))
}

/// Read a repo's .env file contents. Returns empty string if the file doesn't exist.
#[tauri::command]
pub fn read_env_file(state: State<DbState>, repo_id: i64) -> ApiResult<String> {
    let conn = state.0.lock().unwrap();
    let repos = match db_repos::list_repos(&conn, None) {
        Ok(r) => r,
        Err(e) => return ApiResult::err(e),
    };
    let repo = match repos.into_iter().find(|r| r.id == repo_id) {
        Some(r) => r,
        None => return ApiResult::err(format!("Repo {} not found", repo_id)),
    };
    let local_path = match &repo.local_path {
        Some(p) => PathBuf::from(p),
        None => return ApiResult::err("Repo has not been cloned yet."),
    };
    let env_path = local_path.join(".env.test");
    let content = std::fs::read_to_string(&env_path).unwrap_or_default();
    ApiResult::ok(content)
}

/// Write content to a repo's .env.test file.
#[tauri::command]
pub fn write_env_file(state: State<DbState>, repo_id: i64, content: String) -> ApiResult<()> {
    let conn = state.0.lock().unwrap();
    let repos = match db_repos::list_repos(&conn, None) {
        Ok(r) => r,
        Err(e) => return ApiResult::err(e),
    };
    let repo = match repos.into_iter().find(|r| r.id == repo_id) {
        Some(r) => r,
        None => return ApiResult::err(format!("Repo {} not found", repo_id)),
    };
    let local_path = match &repo.local_path {
        Some(p) => PathBuf::from(p),
        None => return ApiResult::err("Repo has not been cloned yet."),
    };
    let env_path = local_path.join(".env.test");
    match std::fs::write(&env_path, &content) {
        Ok(_) => ApiResult::ok(()),
        Err(e) => ApiResult::err(format!("Failed to write .env.test: {}", e)),
    }
}

/// Clone or pull a single repo by its DB id.
#[tauri::command]
pub async fn clone_or_pull_repo(state: State<'_, DbState>, repo_id: i64) -> Result<ApiResult<String>, String> {
    let (token, clone_root, repo) = {
        let conn = state.0.lock().unwrap();
        let token = match db_repos::get_setting(&conn, "github_token") {
            Ok(Some(t)) if !t.is_empty() => t,
            _ => return Ok(ApiResult::err("GitHub token not configured.")),
        };
        let clone_root = match db_repos::get_setting(&conn, "clone_root") {
            Ok(Some(p)) if !p.is_empty() => p,
            _ => return Ok(ApiResult::err("Clone root path not configured. Go to Settings.")),
        };
        let repos = match db_repos::list_repos(&conn, None) {
            Ok(r) => r,
            Err(e) => return Ok(ApiResult::err(e)),
        };
        let repo = match repos.into_iter().find(|r| r.id == repo_id) {
            Some(r) => r,
            None => return Ok(ApiResult::err(format!("Repo {} not found", repo_id))),
        };
        (token, clone_root, repo)
    };

    let dest = PathBuf::from(&clone_root).join(&repo.org).join(&repo.name);
    let github_url = repo.github_url.clone();
    let dest2 = dest.clone();
    let result = tokio::task::spawn_blocking(move || {
        git_ops::clone_or_pull(&github_url, &dest2, &token)
    })
    .await;

    match result {
        Ok(Ok(())) => {}
        Ok(Err(e)) => return Ok(ApiResult::err(format!("Git error: {}", e))),
        Err(e) => return Ok(ApiResult::err(format!("Task error: {}", e))),
    }

    let ruby_version = git_ops::read_ruby_version(&dest);
    let conn = state.0.lock().unwrap();
    if let Err(e) = db_repos::update_repo_local_path(
        &conn,
        repo_id,
        &dest.to_string_lossy(),
        ruby_version.as_deref(),
    ) {
        return Ok(ApiResult::err(e));
    }
    Ok(ApiResult::ok(dest.to_string_lossy().to_string()))
}
