use crate::commands::repos::{ApiResult, DbState};
use crate::db::{coverage as db_cov, repos as db_repos};
use crate::git as git_ops;
use crate::istanbul;
use crate::node::{run_npm_install, run_node_tests};
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
    let node_version = repo.node_version.clone();

    // Determine runtime: if there's a Gemfile it's Ruby, if package.json it's Node.
    // node_version being set is a strong signal too.
    let is_node = node_version.is_some()
        || local_path.join("package.json").exists() && !local_path.join("Gemfile").exists();

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

        if is_node {
            run_node_pipeline(&app, &state_inner, repo_id, run_id, &local_path, node_version.as_deref())
        } else {
            run_ruby_pipeline(&app, &state_inner, repo_id, run_id, &local_path, ruby_version.as_deref())
        }
    })
    .await
    .map_err(|e| format!("Task error: {}", e))?;

    // _in_flight_guard drops here, removing repo_id from in_flight.
    Ok(result)
}

// ── Ruby pipeline ─────────────────────────────────────────────────────────────

fn run_ruby_pipeline(
    app: &AppHandle,
    state_inner: &Arc<std::sync::Mutex<rusqlite::Connection>>,
    repo_id: i64,
    run_id: i64,
    local_path: &std::path::Path,
    ruby_version: Option<&str>,
) -> ApiResult<i64> {
    // ── Bundle install ────────────────────────────────────────────────
    let app_bi = app.clone();
    let bi_result = run_bundle_install(local_path, ruby_version, move |line| {
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
    let db_result = setup_test_database(local_path, ruby_version, move |line| {
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
    let result = run_rspec(local_path, ruby_version, move |line| {
        emit_line(&app_rs, repo_id, run_id, line);
    });

    // Parse SimpleCov BEFORE acquiring the DB lock
    let cov_result: Option<Result<CovData, anyhow::Error>> = match &result {
        Ok(_) => Some(simplecov::parse(local_path).map(CovData::from)),
        Err(_) => None,
    };

    let conn = state_inner.lock().unwrap();
    save_test_result(&conn, repo_id, run_id, result.map(|r| (r.exit_code, r.stderr)), cov_result, "RSpec", "SimpleCov")
}

// ── Node pipeline ─────────────────────────────────────────────────────────────

fn run_node_pipeline(
    app: &AppHandle,
    state_inner: &Arc<std::sync::Mutex<rusqlite::Connection>>,
    repo_id: i64,
    run_id: i64,
    local_path: &std::path::Path,
    node_version: Option<&str>,
) -> ApiResult<i64> {
    // ── npm/yarn/pnpm install ─────────────────────────────────────────
    let app_inst = app.clone();
    let install_result = run_npm_install(local_path, node_version, move |line| {
        emit_line(&app_inst, repo_id, run_id, line);
    });
    if let Err(e) = install_result {
        let msg = format!("npm install failed: {}", e);
        let conn = state_inner.lock().unwrap();
        let _ = db_cov::finish_run(&conn, run_id, "failed", Some(&msg), None, None, None);
        return ApiResult::err(msg);
    }

    // ── Run tests ─────────────────────────────────────────────────────
    let app_test = app.clone();
    let result = run_node_tests(local_path, node_version, move |line| {
        emit_line(&app_test, repo_id, run_id, line);
    });

    // Parse Istanbul/NYC coverage BEFORE acquiring the DB lock
    let cov_result: Option<Result<CovData, anyhow::Error>> = match &result {
        Ok(_) => Some(istanbul::parse(local_path).map(CovData::from)),
        Err(_) => None,
    };

    let conn = state_inner.lock().unwrap();
    save_test_result(&conn, repo_id, run_id, result.map(|r| (r.exit_code, r.stderr)), cov_result, "tests", "coverage")
}

// ── Shared result saver ───────────────────────────────────────────────────────

/// Save test + coverage results for both Ruby and Node pipelines.
/// `test_result` is Ok((exit_code, stderr)) or Err.
/// `cov_result` is the parsed coverage data (if tests ran at all).
fn save_test_result(
    conn: &rusqlite::Connection,
    _repo_id: i64,
    run_id: i64,
    test_result: Result<(i32, String), anyhow::Error>,
    cov_result: Option<Result<CovData, anyhow::Error>>,
    runner_name: &str,
    cov_name: &str,
) -> ApiResult<i64> {
    match test_result {
        Ok((exit_code, stderr)) => {
            let cov_result = cov_result.unwrap(); // always Some when test_result is Ok

            if exit_code == 0 {
                match cov_result {
                    Ok(cov) => {
                        let _ = conn.execute_batch("BEGIN");
                        let _ = db_cov::finish_run(
                            conn, run_id, "success", None,
                            Some(cov.overall_percent),
                            Some(cov.lines_covered),
                            Some(cov.lines_total),
                        );
                        for f in &cov.files {
                            let _ = db_cov::insert_file_coverage(
                                conn, run_id, &f.path,
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
                        let msg = format!("{} succeeded but {} parse failed: {}", runner_name, cov_name, e);
                        let _ = db_cov::finish_run(conn, run_id, "failed", Some(&msg), None, None, None);
                        ApiResult::err(msg)
                    }
                }
            } else {
                let error_detail = if stderr.is_empty() {
                    format!("{} exited with code {}", runner_name, exit_code)
                } else {
                    format!("{} exited with code {}:\n{}", runner_name, exit_code, stderr)
                };

                match cov_result {
                    Ok(cov) => {
                        let _ = conn.execute_batch("BEGIN");
                        let _ = db_cov::finish_run(
                            conn, run_id, "failed", Some(&error_detail),
                            Some(cov.overall_percent),
                            Some(cov.lines_covered),
                            Some(cov.lines_total),
                        );
                        for f in &cov.files {
                            let _ = db_cov::insert_file_coverage(
                                conn, run_id, &f.path,
                                Some(f.coverage_percent),
                                Some(f.lines_covered),
                                Some(f.lines_total),
                                &f.uncovered_lines,
                            );
                        }
                        let _ = conn.execute_batch("COMMIT");
                        ApiResult::ok(run_id)
                    }
                    Err(_) => {
                        let _ = db_cov::finish_run(conn, run_id, "failed", Some(&error_detail), None, None, None);
                        ApiResult::err(error_detail)
                    }
                }
            }
        }
        Err(e) => {
            let msg = e.to_string();
            let _ = db_cov::finish_run(conn, run_id, "failed", Some(&msg), None, None, None);
            ApiResult::err(msg)
        }
    }
}

/// Unified coverage data type used by save_test_result.
struct CovData {
    overall_percent: f64,
    lines_covered: i64,
    lines_total: i64,
    files: Vec<CovFileData>,
}

struct CovFileData {
    path: String,
    coverage_percent: f64,
    lines_covered: i64,
    lines_total: i64,
    uncovered_lines: Vec<usize>,
}

impl From<simplecov::CoverageResult> for CovData {
    fn from(r: simplecov::CoverageResult) -> Self {
        Self {
            overall_percent: r.overall_percent,
            lines_covered: r.lines_covered,
            lines_total: r.lines_total,
            files: r.files.into_iter().map(|f| CovFileData {
                path: f.path,
                coverage_percent: f.coverage_percent,
                lines_covered: f.lines_covered,
                lines_total: f.lines_total,
                uncovered_lines: f.uncovered_lines,
            }).collect(),
        }
    }
}

impl From<istanbul::CoverageResult> for CovData {
    fn from(r: istanbul::CoverageResult) -> Self {
        Self {
            overall_percent: r.overall_percent,
            lines_covered: r.lines_covered,
            lines_total: r.lines_total,
            files: r.files.into_iter().map(|f| CovFileData {
                path: f.path,
                coverage_percent: f.coverage_percent,
                lines_covered: f.lines_covered,
                lines_total: f.lines_total,
                uncovered_lines: f.uncovered_lines,
            }).collect(),
        }
    }
}