use crate::commands::repos::{ApiResult, DbState, with_db};
use crate::eol;
use tauri::State;

/// Refresh EOL data from endoflife.date (at most once per day per runtime).
#[tauri::command]
pub async fn refresh_eol(
    state: State<'_, DbState>,
) -> Result<ApiResult<()>, String> {
    with_db(&state.0, move |conn| {
        match eol::refresh_all_if_stale(conn) {
            Ok(_) => ApiResult::ok(()),
            Err(e) => ApiResult::err(e),
        }
    }).await
}

/// Check EOL status for a specific runtime + version (e.g. "nodejs", "20.11.0").
#[tauri::command]
pub async fn check_eol(
    state: State<'_, DbState>,
    runtime: String,
    version: String,
) -> Result<ApiResult<eol::EolStatus>, String> {
    with_db(&state.0, move |conn| {
        // Attempt a refresh first (no-ops if cache is fresh)
        let _ = eol::refresh_if_stale(conn, &runtime);
        match eol::check_version(conn, &runtime, &version) {
            Ok(status) => ApiResult::ok(status),
            Err(e) => ApiResult::err(e),
        }
    }).await
}

/// List all cached EOL cycles for a runtime ("nodejs" or "ruby").
#[tauri::command]
pub async fn list_eol_cycles(
    state: State<'_, DbState>,
    runtime: String,
) -> Result<ApiResult<Vec<eol::EolCycle>>, String> {
    with_db(&state.0, move |conn| {
        let _ = eol::refresh_if_stale(conn, &runtime);
        match eol::list_cycles(conn, &runtime) {
            Ok(cycles) => ApiResult::ok(cycles),
            Err(e) => ApiResult::err(e),
        }
    }).await
}
