use crate::commands::repos::{ApiResult, DbState, with_db};
use crate::db::coverage as db_cov;
use tauri::State;

#[tauri::command]
pub async fn list_runs(
    state: State<'_, DbState>,
    repo_id: i64,
) -> Result<ApiResult<Vec<db_cov::CoverageRun>>, String> {
    with_db(&state.0, move |conn| {
        match db_cov::list_runs(conn, repo_id) {
            Ok(runs) => ApiResult::ok(runs),
            Err(e) => ApiResult::err(e),
        }
    }).await
}

#[tauri::command]
pub async fn get_trend(
    state: State<'_, DbState>,
    repo_id: i64,
    limit: Option<i64>,
) -> Result<ApiResult<Vec<db_cov::CoverageTrendPoint>>, String> {
    with_db(&state.0, move |conn| {
        match db_cov::get_trend(conn, repo_id, limit.unwrap_or(20)) {
            Ok(trend) => ApiResult::ok(trend),
            Err(e) => ApiResult::err(e),
        }
    }).await
}

#[tauri::command]
pub async fn get_file_coverage(
    state: State<'_, DbState>,
    run_id: i64,
) -> Result<ApiResult<Vec<db_cov::FileCoverage>>, String> {
    with_db(&state.0, move |conn| {
        match db_cov::get_file_coverage(conn, run_id) {
            Ok(files) => ApiResult::ok(files),
            Err(e) => ApiResult::err(e),
        }
    }).await
}
