mod commands;
mod db;
mod git;
mod github;
mod ruby;
mod simplecov;

use commands::repos::DbState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let conn = db::open().expect("failed to open database");

    // Seed default orgs on first run
    let _ = db::repos::add_org(&conn, "g5search");
    let _ = db::repos::add_org(&conn, "g5components");
    if db::repos::get_active_org(&conn).unwrap_or(None).is_none() {
        let _ = db::repos::set_active_org(&conn, "g5search");
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(DbState(std::sync::Mutex::new(conn)))
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

