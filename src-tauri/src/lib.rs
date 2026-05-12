mod commands;
mod db;
pub mod orchestrator;
mod eol;
mod git;
mod ado;
mod confluence;
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
        .plugin(tauri_plugin_notification::init())
        .manage(DbState(std::sync::Arc::new(std::sync::Mutex::new(conn))))
        .manage(RunnerState::new())
        .setup(|app| {
            use std::sync::Arc;
            use tauri::{Manager, Emitter};
            use tauri::menu::{MenuBuilder, MenuItemBuilder};
            use tauri::tray::TrayIconBuilder;
            use tauri_plugin_notification::NotificationExt;
            use crate::orchestrator::{self, state::AppState, db::Store};

            // Co-locate with the existing coverage.db under
            // ~/Library/Application Support/coverage-manager/ instead of
            // Tauri's bundle-id-based dir, so all of the app's persistent
            // state lives under one directory. Matches db::db_path().
            let db_path = dirs::data_local_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join("coverage-manager")
                .join("orchestrator.db");
            let store = Arc::new(Store::open_at(&db_path).expect("open orchestrator db"));
            let state = AppState::new(store.clone());

            let state_for_server = state.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = orchestrator::serve_on(state_for_server, "127.0.0.1:9876").await {
                    eprintln!("orchestrator server failed: {e}");
                }
            });

            let store_for_sweeper = store.clone();
            tauri::async_runtime::spawn(async move {
                orchestrator::sweeper::run_loop(store_for_sweeper).await;
            });

            // Tray icon with Show/Quit menu
            let tray_menu = MenuBuilder::new(app)
                .items(&[
                    &MenuItemBuilder::with_id("show", "Show").build(app)?,
                    &MenuItemBuilder::with_id("quit", "Quit").build(app)?,
                ]).build()?;
            let _tray = TrayIconBuilder::with_id("main")
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&tray_menu)
                .on_menu_event(|app_handle, event| match event.id().as_ref() {
                    "show" => {
                        if let Some(w) = app_handle.get_webview_window("main") {
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                    },
                    "quit" => app_handle.exit(0),
                    _ => {}
                })
                .build(app)?;

            // Badge updates: subscribe to bus, recompute on each state change.
            let bus_for_badge = state.bus.clone();
            let store_for_badge = state.store.clone();
            let app_handle_for_badge = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let mut rx = bus_for_badge.subscribe();
                while rx.recv().await.is_ok() {
                    let s = store_for_badge.clone();
                    let n = tokio::task::spawn_blocking(move ||
                        crate::orchestrator::notify::badge_count(&s)).await.unwrap_or(0);
                    if let Some(tray) = app_handle_for_badge.tray_by_id("main") {
                        let _ = tray.set_title(if n == 0 { None } else { Some(format!("{}", n)) });
                    }
                    let _ = app_handle_for_badge.emit("orchestrator://state",
                        serde_json::json!({"needs_input_count": n}));
                }
            });

            // Notifications: fire on lifecycle transitions.
            let bus_for_notif = state.bus.clone();
            let app_handle_for_notif = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let mut rx = bus_for_notif.subscribe();
                while let Ok(c) = rx.recv().await {
                    let (title, body) = match c.status.as_str() {
                        "needs_input" => ("Claude needs input", c.label.unwrap_or(c.session_id)),
                        "done"        => ("Claude session done", c.label.unwrap_or(c.session_id)),
                        "error"       => ("Claude session error", c.reason.unwrap_or_default()),
                        _ => continue,
                    };
                    let _ = app_handle_for_notif.notification()
                        .builder().title(title).body(body).show();
                }
            });

            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // orgs & repos
            commands::repos::list_orgs,
            commands::repos::add_org,
            commands::repos::remove_org,
            commands::repos::set_active_org,
            commands::repos::get_active_org,
            commands::repos::list_repos,
            commands::repos::list_repo_branches,
            commands::repos::checkout_repo_branch,
            commands::repos::get_repo_sources,
            commands::repos::save_repo_sources,
            commands::repos::set_repo_enabled,
            commands::repos::sync_org_repos,
            commands::repos::clone_or_pull_repo,
            commands::repos::open_in_terminal,
            commands::repos::diagnose_github_auth,
            commands::projects::list_projects,
            commands::projects::get_project,
            commands::projects::create_project,
            commands::projects::save_project,
            commands::projects::update_project_status,
            commands::projects::list_agent_profiles,
            commands::projects::create_agent_profile,
            commands::projects::save_agent_profile,
            commands::projects::delete_agent_profile,
            commands::ai::send_ai_message,
            commands::confluence::confluence_get_space,
            commands::confluence::confluence_get_page,
            commands::confluence::confluence_search_pages,
            commands::confluence::confluence_refresh_page,
            commands::confluence::confluence_get_cached_page,
            commands::confluence::confluence_list_cached_pages,
            commands::ado::ado_preview,
            commands::ado::ado_query_project_work_items,
            commands::ado::ado_list_release_definitions,
            commands::ado::ado_list_releases,
            // settings
            commands::repos::get_settings,
            commands::repos::save_settings,
            // docs
            commands::docs::list_repo_docs,
            commands::docs::read_repo_doc,
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
