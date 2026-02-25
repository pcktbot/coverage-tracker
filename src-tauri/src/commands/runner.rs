use crate::commands::repos::{ApiResult, DbState};
use crate::db::{coverage as db_cov, repos as db_repos};
use crate::git as git_ops;
use crate::ruby::{run_rspec, run_bundle_install, setup_test_database};
use crate::simplecov;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};
use tokio::sync::Semaphore;

/// Concurrency limit for parallel spec runs.
const MAX_CONCURRENT_RUNS: usize = 3;

/// Shared runner state: concurrency semaphore + in-flight tracking.
pub struct RunnerState {
    pub semaphore: Arc<Semaphore>,
    pub in_flight: Arc<std::sync::Mutex<HashSet<i64>>>,
}

impl RunnerState {
    pub fn new() -> Self {
        Self {
            semaphore: Arc::new(Semaphore::new(MAX_CONCURRENT_RUNS)),
            in_flight: Arc::new(std::sync::Mutex::new(HashSet::new())),
        }
    }
}

/// RAII guard that removes a repo_id from the in-flight set on drop,
/// so cleanup happens even if the task panics.
struct InFlightGuard {
    repo_id: i64,
    in_flight: Arc<std::sync::Mutex<HashSet<i64>>>,
}

impl Drop for InFlightGuard {
    fn drop(&mut self) {
        let mut flights = self.in_flight.lock().unwrap();
        flights.remove(&self.repo_id);
    }
}

#[derive(serde::Serialize, Clone)]
pub struct RspecLineEvent {
    pub repo_id: i64,
    pub run_id: i64,
    pub line: String,
}

/// Emit a line to the rspec-output channel (convenience helper).
fn emit_line(app: &AppHandle, repo_id: i64, run_id: i64, line: impl Into<String>) {
    let _ = app.emit(
        "rspec-output",
        RspecLineEvent { repo_id, run_id, line: line.into() },
    );
}

/// Run RSpec for a single repo. Streams output via `rspec-output` events.
/// Async — all blocking work runs on a background thread so the UI stays responsive.
/// Guarded by a semaphore (max 3 concurrent) and a duplicate-run check.
#[tauri::command]
pub async fn run_coverage(
    app: AppHandle,
    state: State<'_, DbState>,
    runner: State<'_, RunnerState>,
    repo_id: i64,
) -> Result<ApiResult<i64>, String> {
    // ── Duplicate guard ─────────────────────────────────────────────────────
    // RAII guard ensures repo_id is removed from in_flight even on panic.
    let _in_flight_guard = {
        let mut flights = runner.in_flight.lock().unwrap();
        if !flights.insert(repo_id) {
            return Ok(ApiResult::err("A run is already in progress for this repo."));
        }
        InFlightGuard {
            repo_id,
            in_flight: Arc::clone(&runner.in_flight),
        }
    };

    // ── Acquire semaphore permit (waits if at capacity) ─────────────────
    let _permit = runner.semaphore.clone().acquire_owned().await
        .map_err(|e| format!("Semaphore error: {}", e))?;

    // ── Gather data from DB (short lock) ──────────────────────────────────
    let (repo, token, local_path, run_id) = {
        let conn = state.0.lock().unwrap();
        let repos = match db_repos::list_repos(&conn, None) {
            Ok(r) => r,
            Err(e) => return Ok(ApiResult::err(e)),
        };
        let repo = match repos.into_iter().find(|r| r.id == repo_id) {
            Some(r) => r,
            None => return Ok(ApiResult::err(format!("Repo {} not found", repo_id))),
        };
        let local_path = match &repo.local_path {
            Some(p) => PathBuf::from(p),
            None => return Ok(ApiResult::err("Repo has not been cloned yet. Clone it first.")),
        };
        let token = db_repos::get_setting(&conn, "github_token")
            .unwrap_or(None)
            .unwrap_or_default();
        let run_id = match db_cov::start_run(&conn, repo_id) {
            Ok(id) => id,
            Err(e) => return Ok(ApiResult::err(e)),
        };
        (repo, token, local_path, run_id)
    };
    // Lock is released here.

    // ── Background thread does all the heavy lifting ──────────────────────
    let app2 = app.clone();
    let state_inner: Arc<std::sync::Mutex<rusqlite::Connection>> = Arc::clone(&state.0);
    let github_url = repo.github_url.clone();
    let ruby_version = repo.ruby_version.clone();

    let result = tokio::task::spawn_blocking(move || {
        let app = app2;

        // ── Git pull ──────────────────────────────────────────────────────
        if !token.is_empty() {
            emit_line(&app, repo_id, run_id, "[git pull] updating…");
            match git_ops::clone_or_pull(&github_url, &local_path, &token) {
                Ok(_) => emit_line(&app, repo_id, run_id, "[git pull] done"),
                Err(e) => emit_line(&app, repo_id, run_id, format!("[git pull] warning: {}", e)),
            }
        }

        // ── Bundle install ────────────────────────────────────────────────
        let app_bi = app.clone();
        let bi_result = run_bundle_install(&local_path, ruby_version.as_deref(), move |line| {
            emit_line(&app_bi, repo_id, run_id, line);
        });
        if let Err(e) = bi_result {
            let msg = format!("bundle install failed: {}", e);
            let conn = state_inner.lock().unwrap();
            let _ = db_cov::finish_run(&conn, run_id, "failed", Some(&msg), None, None, None);
            return ApiResult::err(msg);
        }

        // ── Database setup ────────────────────────────────────────────────
        let app_db = app.clone();
        let db_result = setup_test_database(&local_path, ruby_version.as_deref(), move |line| {
            emit_line(&app_db, repo_id, run_id, line);
        });
        if let Err(e) = db_result {
            let msg = format!("Database setup failed: {}", e);
            let conn = state_inner.lock().unwrap();
            let _ = db_cov::finish_run(&conn, run_id, "failed", Some(&msg), None, None, None);
            return ApiResult::err(msg);
        }

        // ── RSpec ─────────────────────────────────────────────────────────
        let app_rs = app.clone();
        let result = run_rspec(&local_path, ruby_version.as_deref(), move |line| {
            emit_line(&app_rs, repo_id, run_id, line);
        });

        // Parse SimpleCov BEFORE acquiring the DB lock — this does file I/O
        // and JSON deserialization which can take seconds for large repos.
        let cov_result = match &result {
            Ok(_) => Some(simplecov::parse(&local_path)),
            Err(_) => None,
        };

        // Now acquire the DB lock only for the short INSERT/UPDATE operations.
        // Wrap all writes in a transaction to minimize lock duration.
        let conn = state_inner.lock().unwrap();

        match result {
            Ok(rspec) => {
                let cov_result = cov_result.unwrap(); // always Some when result is Ok

                if rspec.exit_code == 0 {
                    // All tests passed.
                    match cov_result {
                        Ok(cov) => {
                            let _ = conn.execute_batch("BEGIN");
                            let _ = db_cov::finish_run(
                                &conn, run_id, "success", None,
                                Some(cov.overall_percent),
                                Some(cov.lines_covered),
                                Some(cov.lines_total),
                            );
                            for f in &cov.files {
                                let _ = db_cov::insert_file_coverage(
                                    &conn, run_id, &f.path,
                                    Some(f.coverage_percent),
                                    Some(f.lines_covered),
                                    Some(f.lines_total),
                                    &f.uncovered_lines,
                                );
                            }
                            let _ = conn.execute_batch("COMMIT");
                            ApiResult::ok(run_id)
                        }
                        Err(e) => {
                            let msg = format!("RSpec succeeded but SimpleCov parse failed: {}", e);
                            let _ = db_cov::finish_run(&conn, run_id, "failed", Some(&msg), None, None, None);
                            ApiResult::err(msg)
                        }
                    }
                } else {
                    // Tests failed (exit code != 0) — still save coverage if available.
                    let error_detail = if rspec.stderr.is_empty() {
                        format!("RSpec exited with code {}", rspec.exit_code)
                    } else {
                        format!("RSpec exited with code {}:\n{}", rspec.exit_code, rspec.stderr)
                    };

                    match cov_result {
                        Ok(cov) => {
                            // Tests failed but coverage data was collected.
                            let _ = conn.execute_batch("BEGIN");
                            let _ = db_cov::finish_run(
                                &conn, run_id, "failed", Some(&error_detail),
                                Some(cov.overall_percent),
                                Some(cov.lines_covered),
                                Some(cov.lines_total),
                            );
                            for f in &cov.files {
                                let _ = db_cov::insert_file_coverage(
                                    &conn, run_id, &f.path,
                                    Some(f.coverage_percent),
                                    Some(f.lines_covered),
                                    Some(f.lines_total),
                                    &f.uncovered_lines,
                                );
                            }
                            let _ = conn.execute_batch("COMMIT");
                            // Return success so the frontend refreshes coverage data
                            // even though tests had failures.
                            ApiResult::ok(run_id)
                        }
                        Err(_) => {
                            let _ = db_cov::finish_run(&conn, run_id, "failed", Some(&error_detail), None, None, None);
                            ApiResult::err(error_detail)
                        }
                    }
                }
            }
            Err(e) => {
                let msg = e.to_string();
                let _ = db_cov::finish_run(&conn, run_id, "failed", Some(&msg), None, None, None);
                ApiResult::err(msg)
            }
        }
    })
    .await
    .map_err(|e| format!("Task error: {}", e))?;

    // _in_flight_guard drops here, removing repo_id from in_flight.
    Ok(result)
}
