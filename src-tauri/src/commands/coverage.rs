use crate::commands::repos::{ApiResult, DbState};
use crate::db::coverage as db_cov;
use tauri::State;

#[tauri::command]
pub fn list_runs(
    state: State<DbState>,
    repo_id: i64,
) -> ApiResult<Vec<db_cov::CoverageRun>> {
    let conn = state.0.lock().unwrap();
    match db_cov::list_runs(&conn, repo_id) {
        Ok(runs) => ApiResult::ok(runs),
        Err(e) => ApiResult::err(e),
    }
}

#[tauri::command]
pub fn get_trend(
    state: State<DbState>,
    repo_id: i64,
    limit: Option<i64>,
) -> ApiResult<Vec<db_cov::CoverageTrendPoint>> {
    let conn = state.0.lock().unwrap();
    match db_cov::get_trend(&conn, repo_id, limit.unwrap_or(20)) {
        Ok(trend) => ApiResult::ok(trend),
        Err(e) => ApiResult::err(e),
    }
}

#[tauri::command]
pub fn get_file_coverage(
    state: State<DbState>,
    run_id: i64,
) -> ApiResult<Vec<db_cov::FileCoverage>> {
    let conn = state.0.lock().unwrap();
    match db_cov::get_file_coverage(&conn, run_id) {
        Ok(files) => ApiResult::ok(files),
        Err(e) => ApiResult::err(e),
    }
}
