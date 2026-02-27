mod commands;
mod db;
mod eol;
mod git;
mod github;
mod istanbul;
mod node;
mod ruby;
mod simplecov;
mod version_manager;

use commands::repos::DbState;
use commands::runner::RunnerState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let conn = db::open().expect("failed to open database");

    // Mark any runs left as 'running' from a previous session as interrupted
    match db::coverage::mark_interrupted_runs(&conn) {
        Ok(n) if n > 0 => eprintln!("Marked {n} stale running run(s) as interrupted"),
        Err(e) => eprintln!("Warning: failed to clean up stale runs: {e}"),
        _ => {}
    }

    // Seed default orgs on first run
    let _ = db::repos::add_org(&conn, "g5search");
    let _ = db::repos::add_org(&conn, "g5components");
    if db::repos::get_active_org(&conn).unwrap_or(None).is_none() {
        let _ = db::repos::set_active_org(&conn, "g5search");
    }

    // Pre-warm the EOL cache (non-blocking — skips silently on network errors)
    if let Err(e) = eol::refresh_all_if_stale(&conn) {
        eprintln!("Warning: initial EOL cache refresh failed: {e}");
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(DbState(std::sync::Arc::new(std::sync::Mutex::new(conn))))
        .manage(RunnerState::new())
        .invoke_handler(tauri::generate_handler![
            // orgs & repos
            commands::repos::list_orgs,
            commands::repos::add_org,
            commands::repos::remove_org,
            commands::repos::set_active_org,
            commands::repos::get_active_org,
            commands::repos::list_repos,
            commands::repos::set_repo_enabled,
            commands::repos::sync_org_repos,
            commands::repos::clone_or_pull_repo,
            commands::repos::open_in_terminal,
            commands::repos::read_env_file,
            commands::repos::write_env_file,
            // settings
            commands::repos::get_settings,
            commands::repos::save_settings,
            // runner
            commands::runner::run_coverage,
            // coverage queries
            commands::coverage::list_runs,
            commands::coverage::get_trend,
            commands::coverage::get_file_coverage,
            // export
            commands::export::export_csv,
            // EOL tracking
            commands::eol::refresh_eol,
            commands::eol::check_eol,
            commands::eol::list_eol_cycles,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

