use crate::db::repos as db_repos;
use crate::github::GithubClient;
use crate::git as git_ops;
use serde::Serialize;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, State};
use std::process::Command as StdCommand;

pub struct DbState(pub std::sync::Arc<std::sync::Mutex<rusqlite::Connection>>);

#[derive(Serialize)]
pub struct ApiResult<T> {
    pub ok: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

#[derive(Serialize)]
pub struct AuthCheck {
    pub ok: bool,
    pub status: String,
    pub message: String,
    pub hint: Option<String>,
}

#[derive(Serialize)]
pub struct GithubAuthDiagnostics {
    pub token_present: bool,
    pub org: Option<String>,
    pub repo: Option<String>,
    pub api: AuthCheck,
    pub git: AuthCheck,
}

#[derive(Serialize)]
pub struct SettingsPayload {
    pub github_token: String,
    pub clone_root: String,
    pub tfs_base_url: String,
    pub tfs_pat: String,
    pub tfs_collection: String,
    pub confluence_base_url: String,
    pub confluence_username: String,
    pub confluence_token: String,
}

#[derive(Serialize)]
pub struct RepoBranchState {
    pub current_branch: String,
    pub branches: Vec<git_ops::RepoBranch>,
}

impl<T: Serialize> ApiResult<T> {
    pub fn ok(data: T) -> Self {
        Self { ok: true, data: Some(data), error: None }
    }
    pub fn err(msg: impl ToString) -> Self {
        Self { ok: false, data: None, error: Some(msg.to_string()) }
    }
}

/// Run a closure with the DB connection on a blocking thread.
/// Keeps heavy / mutex work off the Tauri main thread so the window never freezes.
pub async fn with_db<T, F>(db: &std::sync::Arc<std::sync::Mutex<rusqlite::Connection>>, f: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce(&rusqlite::Connection) -> T + Send + 'static,
{
    let db = db.clone();
    tokio::task::spawn_blocking(move || {
        let conn = db.lock().unwrap();
        f(&conn)
    })
    .await
    .map_err(|e| e.to_string())
}

// ── Orgs ──────────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn list_orgs(state: State<'_, DbState>) -> Result<ApiResult<Vec<db_repos::Org>>, String> {
    with_db(&state.0, |conn| {
        match db_repos::list_orgs(conn) {
            Ok(orgs) => ApiResult::ok(orgs),
            Err(e) => ApiResult::err(e),
        }
    }).await
}

#[tauri::command]
pub async fn add_org(state: State<'_, DbState>, name: String) -> Result<ApiResult<()>, String> {
    with_db(&state.0, move |conn| {
        match db_repos::add_org(conn, &name) {
            Ok(_) => ApiResult::ok(()),
            Err(e) => ApiResult::err(e),
        }
    }).await
}

#[tauri::command]
pub async fn remove_org(state: State<'_, DbState>, name: String) -> Result<ApiResult<()>, String> {
    with_db(&state.0, move |conn| {
        match db_repos::remove_org(conn, &name) {
            Ok(_) => ApiResult::ok(()),
            Err(e) => ApiResult::err(e),
        }
    }).await
}

#[tauri::command]
pub async fn set_active_org(state: State<'_, DbState>, name: String) -> Result<ApiResult<()>, String> {
    with_db(&state.0, move |conn| {
        match db_repos::set_active_org(conn, &name) {
            Ok(_) => ApiResult::ok(()),
            Err(e) => ApiResult::err(e),
        }
    }).await
}

#[tauri::command]
pub async fn get_active_org(state: State<'_, DbState>) -> Result<ApiResult<Option<String>>, String> {
    with_db(&state.0, |conn| {
        match db_repos::get_active_org(conn) {
            Ok(org) => ApiResult::ok(org),
            Err(e) => ApiResult::err(e),
        }
    }).await
}

// ── Settings ──────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn get_settings(state: State<'_, DbState>) -> Result<ApiResult<SettingsPayload>, String> {
    with_db(&state.0, |conn| {
        let token = db_repos::get_setting(conn, "github_token").unwrap_or(None);
        let clone_root = db_repos::get_setting(conn, "clone_root").unwrap_or(None);
        let tfs_base_url = db_repos::get_setting(conn, "tfs_base_url").unwrap_or(None);
        let tfs_pat = db_repos::get_setting(conn, "tfs_pat").unwrap_or(None);
        let tfs_collection = db_repos::get_setting(conn, "tfs_collection").unwrap_or(None);
        let confluence_base_url = db_repos::get_setting(conn, "confluence_base_url").unwrap_or(None);
        let confluence_username = db_repos::get_setting(conn, "confluence_username").unwrap_or(None);
        let confluence_token = db_repos::get_setting(conn, "confluence_token").unwrap_or(None);
        ApiResult::ok(SettingsPayload {
            github_token: token.unwrap_or_default(),
            clone_root: clone_root.unwrap_or_default(),
            tfs_base_url: tfs_base_url.unwrap_or_default(),
            tfs_pat: tfs_pat.unwrap_or_default(),
            tfs_collection: tfs_collection.unwrap_or_default(),
            confluence_base_url: confluence_base_url.unwrap_or_default(),
            confluence_username: confluence_username.unwrap_or_default(),
            confluence_token: confluence_token.unwrap_or_default(),
        })
    }).await
}

#[tauri::command]
pub async fn save_settings(
    state: State<'_, DbState>,
    github_token: String,
    clone_root: String,
    tfs_base_url: String,
    tfs_pat: String,
    tfs_collection: String,
    confluence_base_url: String,
    confluence_username: String,
    confluence_token: String,
) -> Result<ApiResult<()>, String> {
    with_db(&state.0, move |conn| {
        if let Err(e) = db_repos::set_setting(conn, "github_token", &github_token) {
            return ApiResult::err(e);
        }
        if let Err(e) = db_repos::set_setting(conn, "clone_root", &clone_root) {
            return ApiResult::err(e);
        }
        if let Err(e) = db_repos::set_setting(conn, "tfs_base_url", &tfs_base_url) {
            return ApiResult::err(e);
        }
        if let Err(e) = db_repos::set_setting(conn, "tfs_pat", &tfs_pat) {
            return ApiResult::err(e);
        }
        if let Err(e) = db_repos::set_setting(conn, "tfs_collection", &tfs_collection) {
            return ApiResult::err(e);
        }
        if let Err(e) = db_repos::set_setting(conn, "confluence_base_url", &confluence_base_url) {
            return ApiResult::err(e);
        }
        if let Err(e) = db_repos::set_setting(conn, "confluence_username", &confluence_username) {
            return ApiResult::err(e);
        }
        if let Err(e) = db_repos::set_setting(conn, "confluence_token", &confluence_token) {
            return ApiResult::err(e);
        }
        ApiResult::ok(())
    }).await
}

#[tauri::command]
pub async fn diagnose_github_auth(
    state: State<'_, DbState>,
    org: Option<String>,
) -> Result<ApiResult<GithubAuthDiagnostics>, String> {
    let snapshot = with_db(&state.0, move |conn| {
        let token = db_repos::get_setting(conn, "github_token").unwrap_or(None).unwrap_or_default();
        let active_org = db_repos::get_active_org(conn).unwrap_or(None);
        let selected_org = org.or(active_org);
        let repos = db_repos::list_repos(conn, None).unwrap_or_default();
        let candidate_repo = repos
            .iter()
            .filter(|repo| selected_org.as_ref().map(|org| repo.org == *org).unwrap_or(true))
            .find(|repo| !repo.github_url.is_empty())
            .or_else(|| repos.iter().find(|repo| !repo.github_url.is_empty()))
            .map(|repo| (repo.org.clone(), repo.name.clone(), repo.github_url.clone()));
        (token, selected_org, candidate_repo)
    }).await?;

    let (token, selected_org, candidate_repo) = snapshot;

    if token.is_empty() {
        return Ok(ApiResult::ok(GithubAuthDiagnostics {
            token_present: false,
            org: selected_org,
            repo: candidate_repo.as_ref().map(|(_, name, _)| name.clone()),
            api: AuthCheck {
                ok: false,
                status: "missing_token".into(),
                message: "No GitHub token is configured.".into(),
                hint: Some("Add a PAT in Settings, then authorize it for the org if SSO is enforced.".into()),
            },
            git: AuthCheck {
                ok: false,
                status: "missing_token".into(),
                message: "Git auth was not tested because no token is configured.".into(),
                hint: Some("Git HTTPS auth in this app uses the same token as the GitHub API.".into()),
            },
        }));
    }

    let token_for_api = token.clone();
    let api_org = selected_org.clone();
    let api_result = tokio::task::spawn_blocking(move || {
        let client = GithubClient::new(&token_for_api);
        let viewer = client.get_viewer_login().map_err(|e| e.to_string())?;
        if let Some(org) = api_org.as_deref() {
            let repos = client.list_all_repos(org).map_err(|e| e.to_string())?;
            Ok::<AuthCheck, String>(AuthCheck {
                ok: true,
                status: "ok".into(),
                message: format!("GitHub API auth worked as {viewer}. Org access to {org} returned {} repos.", repos.len()),
                hint: None,
            })
        } else {
            Ok::<AuthCheck, String>(AuthCheck {
                ok: true,
                status: "ok".into(),
                message: format!("GitHub API auth worked as {viewer}."),
                hint: None,
            })
        }
    }).await.map_err(|e| e.to_string())?;

    let api = match api_result {
        Ok(check) => check,
        Err(err) => AuthCheck {
            ok: false,
            status: "failed".into(),
            message: err,
            hint: Some("If the org now enforces SSO, confirm the PAT is explicitly authorized for that org.".into()),
        },
    };

    let git = if let Some((repo_org, repo_name, repo_url)) = candidate_repo.as_ref() {
        let token_for_git = token.clone();
        let repo_url = repo_url.clone();
        let repo_org = repo_org.clone();
        let repo_name = repo_name.clone();
        let git_result = tokio::task::spawn_blocking(move || {
            git_ops::probe_auth(&repo_url, &token_for_git).map_err(|e| e.to_string())
        }).await.map_err(|e| e.to_string())?;

        match git_result {
            Ok(_) => AuthCheck {
                ok: true,
                status: "ok".into(),
                message: format!("Git HTTPS auth succeeded against {repo_org}/{repo_name}."),
                hint: None,
            },
            Err(err) => AuthCheck {
                ok: false,
                status: "failed".into(),
                message: format!("Git HTTPS auth failed against {repo_org}/{repo_name}: {err}"),
                hint: Some("This app clones over HTTPS with username `x-access-token` and your PAT as the password. SSO authorization failures usually break this path too.".into()),
            },
        }
    } else {
        AuthCheck {
            ok: false,
            status: "skipped".into(),
            message: "Git auth was not tested because no tracked repo with a GitHub URL was available.".into(),
            hint: Some("Sync an org first so the app has a repo URL it can probe.".into()),
        }
    };

    Ok(ApiResult::ok(GithubAuthDiagnostics {
        token_present: true,
        org: selected_org,
        repo: candidate_repo.map(|(_, name, _)| name),
        api,
        git,
    }))
}

// ── Repos ─────────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn list_repos(state: State<'_, DbState>, org: Option<String>) -> Result<ApiResult<Vec<db_repos::Repo>>, String> {
    with_db(&state.0, move |conn| {
        match db_repos::list_repos(conn, org.as_deref()) {
            Ok(repos) => ApiResult::ok(repos),
            Err(e) => ApiResult::err(e),
        }
    }).await
}

#[tauri::command]
pub async fn list_repo_branches(
    state: State<'_, DbState>,
    repo_id: i64,
) -> Result<ApiResult<RepoBranchState>, String> {
    let (local_path, token) = with_db(&state.0, move |conn| {
        let repos = match db_repos::list_repos(conn, None) {
            Ok(r) => r,
            Err(e) => return Err(e.to_string()),
        };
        let repo = repos
            .into_iter()
            .find(|repo| repo.id == repo_id)
            .ok_or_else(|| format!("Repo {repo_id} not found"))?;
        let local_path = repo
            .local_path
            .ok_or_else(|| "Repo has not been cloned yet.".to_string())?;
        let token = db_repos::get_setting(conn, "github_token")
            .unwrap_or(None)
            .unwrap_or_default();
        Ok::<(String, String), String>((local_path, token))
    }).await??;

    let result = tokio::task::spawn_blocking(move || {
        git_ops::list_branches(PathBuf::from(local_path).as_path(), Some(&token))
            .map(|(current_branch, branches)| RepoBranchState { current_branch, branches })
            .map_err(|e| e.to_string())
    }).await.map_err(|e| e.to_string())?;

    match result {
        Ok(state) => Ok(ApiResult::ok(state)),
        Err(err) => Ok(ApiResult::err(err)),
    }
}

#[tauri::command]
pub async fn checkout_repo_branch(
    state: State<'_, DbState>,
    repo_id: i64,
    branch_name: String,
) -> Result<ApiResult<RepoBranchState>, String> {
    let (local_path, token) = with_db(&state.0, move |conn| {
        let repos = match db_repos::list_repos(conn, None) {
            Ok(r) => r,
            Err(e) => return Err(e.to_string()),
        };
        let repo = repos
            .into_iter()
            .find(|repo| repo.id == repo_id)
            .ok_or_else(|| format!("Repo {repo_id} not found"))?;
        let local_path = repo
            .local_path
            .ok_or_else(|| "Repo has not been cloned yet.".to_string())?;
        let token = db_repos::get_setting(conn, "github_token")
            .unwrap_or(None)
            .unwrap_or_default();
        Ok::<(String, String), String>((local_path, token))
    }).await??;

    let result = tokio::task::spawn_blocking(move || {
        let path = PathBuf::from(local_path);
        git_ops::checkout_branch(path.as_path(), &branch_name, Some(&token))
            .and_then(|_| git_ops::list_branches(path.as_path(), Some(&token)))
            .map(|(current_branch, branches)| RepoBranchState { current_branch, branches })
            .map_err(|e| e.to_string())
    }).await.map_err(|e| e.to_string())?;

    match result {
        Ok(state) => Ok(ApiResult::ok(state)),
        Err(err) => Ok(ApiResult::err(err)),
    }
}

#[tauri::command]
pub async fn get_repo_sources(
    state: State<'_, DbState>,
    repo_id: i64,
) -> Result<ApiResult<db_repos::RepoSources>, String> {
    with_db(&state.0, move |conn| {
        match db_repos::get_repo_sources(conn, repo_id) {
            Ok(sources) => ApiResult::ok(sources),
            Err(err) => ApiResult::err(err),
        }
    }).await
}

#[tauri::command]
pub async fn save_repo_sources(
    state: State<'_, DbState>,
    repo_id: i64,
    platform_name: Option<String>,
    tfs_project: Option<String>,
    tfs_area_path: Option<String>,
    tfs_team: Option<String>,
    tfs_release_definition: Option<String>,
    confluence_space_key: Option<String>,
    confluence_parent_page_id: Option<String>,
    confluence_site_label: Option<String>,
    notes: Option<String>,
) -> Result<ApiResult<()>, String> {
    with_db(&state.0, move |conn| {
        let sources = db_repos::RepoSources {
            repo_id,
            platform_name,
            tfs_project,
            tfs_area_path,
            tfs_team,
            tfs_release_definition,
            confluence_space_key,
            confluence_parent_page_id,
            confluence_site_label,
            notes,
        };
        match db_repos::upsert_repo_sources(conn, &sources) {
            Ok(_) => ApiResult::ok(()),
            Err(err) => ApiResult::err(err),
        }
    }).await
}

#[tauri::command]
pub async fn set_repo_enabled(state: State<'_, DbState>, id: i64, enabled: bool) -> Result<ApiResult<()>, String> {
    with_db(&state.0, move |conn| {
        match db_repos::set_repo_enabled(conn, id, enabled) {
            Ok(_) => ApiResult::ok(()),
            Err(e) => ApiResult::err(e),
        }
    }).await
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

/// Open the repo directory in a new terminal window.
#[tauri::command]
pub async fn open_in_terminal(state: State<'_, DbState>, repo_id: i64) -> Result<ApiResult<()>, String> {
    let local_path = {
        let conn = state.0.lock().unwrap();
        let repos = match db_repos::list_repos(&conn, None) {
            Ok(r) => r,
            Err(e) => return Ok(ApiResult::err(e)),
        };
        let repo = match repos.into_iter().find(|r| r.id == repo_id) {
            Some(r) => r,
            None => return Ok(ApiResult::err(format!("Repo {} not found", repo_id))),
        };
        match repo.local_path {
            Some(p) => p,
            None => return Ok(ApiResult::err("Repo has not been cloned yet.")),
        }
    };

    let path = PathBuf::from(&local_path);
    if !path.exists() {
        return Ok(ApiResult::err(format!("Directory not found: {}", local_path)));
    }

    // macOS: open Terminal.app at the repo directory
    let result = StdCommand::new("open")
        .arg("-a")
        .arg("Terminal")
        .arg(&local_path)
        .spawn();

    match result {
        Ok(_) => Ok(ApiResult::ok(())),
        Err(e) => Ok(ApiResult::err(format!("Failed to open terminal: {}", e))),
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
    let node_version = git_ops::read_node_version(&dest);
    let conn = state.0.lock().unwrap();
    if let Err(e) = db_repos::update_repo_local_path(
        &conn,
        repo_id,
        &dest.to_string_lossy(),
        ruby_version.as_deref(),
        node_version.as_deref(),
    ) {
        return Ok(ApiResult::err(e));
    }
    Ok(ApiResult::ok(dest.to_string_lossy().to_string()))
}
