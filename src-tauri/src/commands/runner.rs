use crate::commands::repos::{ApiResult, DbState};
use crate::db::{coverage as db_cov, repos as db_repos};
use crate::ruby::run_rspec;
use crate::simplecov;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, State};

#[derive(serde::Serialize, Clone)]
pub struct RspecLineEvent {
    pub repo_id: i64,
    pub run_id: i64,
    pub line: String,
}

/// Run RSpec for a single repo. Streams output via `rspec-output` events.
/// This is intentionally synchronous from Tauri's perspective; callers should
/// invoke it from a non-blocking context (JavaScript worker or async invoke).
#[tauri::command]
pub fn run_coverage(
    app: AppHandle,
    state: State<DbState>,
    repo_id: i64,
) -> ApiResult<i64> {
    let conn = state.0.lock().unwrap();

    let repos = match db_repos::list_repos(&conn, None) {
        Ok(r) => r,
        Err(e) => return ApiResult::err(e),
    };
    let repo = match repos.iter().find(|r| r.id == repo_id) {
        Some(r) => r.clone(),
        None => return ApiResult::err(format!("Repo {} not found", repo_id)),
    };

    let local_path = match &repo.local_path {
        Some(p) => PathBuf::from(p),
        None => return ApiResult::err("Repo has not been cloned yet. Clone it first."),
    };

    let run_id = match db_cov::start_run(&conn, repo_id) {
        Ok(id) => id,
        Err(e) => return ApiResult::err(e),
    };

    drop(conn); // release the lock before blocking on rspec

    let app2 = app.clone();
    let result = run_rspec(&local_path, repo.ruby_version.as_deref(), move |line| {
        let _ = app2.emit(
            "rspec-output",
            RspecLineEvent { repo_id, run_id, line: line.to_string() },
        );
    });

    let conn = state.0.lock().unwrap();

    match result {
        Ok(rspec) => {
            if rspec.exit_code == 0 {
                // Parse SimpleCov output
                match simplecov::parse(&local_path) {
                    Ok(cov) => {
                        let _ = db_cov::finish_run(
                            &conn,
                            run_id,
                            "success",
                            None,
                            Some(cov.overall_percent),
                            Some(cov.lines_covered),
                            Some(cov.lines_total),
                        );
                        for f in &cov.files {
                            let _ = db_cov::insert_file_coverage(
                                &conn,
                                run_id,
                                &f.path,
                                Some(f.coverage_percent),
                                Some(f.lines_covered),
                                Some(f.lines_total),
                            );
                        }
                        ApiResult::ok(run_id)
                    }
                    Err(e) => {
                        let msg = format!("RSpec succeeded but SimpleCov parse failed: {}", e);
                        let _ = db_cov::finish_run(&conn, run_id, "failed", Some(&msg), None, None, None);
                        ApiResult::err(msg)
                    }
                }
            } else {
                let msg = if rspec.stderr.is_empty() {
                    format!("RSpec exited with code {}", rspec.exit_code)
                } else {
                    format!("RSpec exited with code {}:\n{}", rspec.exit_code, rspec.stderr)
                };
                let _ = db_cov::finish_run(&conn, run_id, "failed", Some(&msg), None, None, None);
                ApiResult::err(msg)
            }
        }
        Err(e) => {
            let msg = e.to_string();
            let _ = db_cov::finish_run(&conn, run_id, "failed", Some(&msg), None, None, None);
            ApiResult::err(msg)
        }
    }
}
