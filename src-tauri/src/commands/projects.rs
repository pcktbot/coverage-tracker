use crate::commands::repos::{with_db, ApiResult, DbState};
use crate::db::projects as db_projects;
use tauri::State;

#[tauri::command]
pub async fn list_projects(
    state: State<'_, DbState>,
) -> Result<ApiResult<Vec<db_projects::ProjectSummary>>, String> {
    with_db(&state.0, |conn| match db_projects::list_projects(conn) {
        Ok(projects) => ApiResult::ok(projects),
        Err(err) => ApiResult::err(err),
    }).await
}

#[tauri::command]
pub async fn get_project(
    state: State<'_, DbState>,
    project_id: i64,
) -> Result<ApiResult<db_projects::Project>, String> {
    with_db(&state.0, move |conn| match db_projects::get_project(conn, project_id) {
        Ok(project) => ApiResult::ok(project),
        Err(err) => ApiResult::err(err),
    }).await
}

#[tauri::command]
pub async fn create_project(
    state: State<'_, DbState>,
    name: String,
) -> Result<ApiResult<i64>, String> {
    with_db(&state.0, move |conn| match db_projects::create_project(conn, &name) {
        Ok(project_id) => ApiResult::ok(project_id),
        Err(err) => ApiResult::err(err),
    }).await
}

#[tauri::command]
pub async fn save_project(
    state: State<'_, DbState>,
    project_id: i64,
    name: String,
    status: String,
    platform_name: Option<String>,
    manual_priority: i64,
    notes: Option<String>,
    is_active: bool,
    linked_repo_ids: Vec<i64>,
) -> Result<ApiResult<()>, String> {
    with_db(&state.0, move |conn| {
        let project = db_projects::Project {
            id: project_id,
            name,
            slug: String::new(),
            status,
            platform_name,
            manual_priority,
            notes,
            is_active,
            linked_repo_ids,
        };
        match db_projects::save_project(conn, &project) {
            Ok(_) => ApiResult::ok(()),
            Err(err) => ApiResult::err(err),
        }
    }).await
}

#[tauri::command]
pub async fn list_agent_profiles(
    state: State<'_, DbState>,
) -> Result<ApiResult<Vec<db_projects::AgentProfile>>, String> {
    with_db(&state.0, |conn| match db_projects::list_agent_profiles(conn) {
        Ok(profiles) => ApiResult::ok(profiles),
        Err(err) => ApiResult::err(err),
    }).await
}

#[tauri::command]
pub async fn create_agent_profile(
    state: State<'_, DbState>,
    name: String,
) -> Result<ApiResult<i64>, String> {
    with_db(&state.0, move |conn| match db_projects::create_agent_profile(conn, &name) {
        Ok(profile_id) => ApiResult::ok(profile_id),
        Err(err) => ApiResult::err(err),
    }).await
}

#[tauri::command]
pub async fn save_agent_profile(
    state: State<'_, DbState>,
    profile_id: i64,
    name: String,
    goal: String,
    instructions: String,
    source_types: Vec<String>,
    project_scope: String,
    weight_manual_priority: i64,
    weight_release_risk: i64,
    weight_doc_gap: i64,
    weight_meeting_followup: i64,
    is_active: bool,
) -> Result<ApiResult<()>, String> {
    with_db(&state.0, move |conn| {
        let profile = db_projects::AgentProfile {
            id: profile_id,
            name,
            goal,
            instructions,
            source_types,
            project_scope,
            weight_manual_priority,
            weight_release_risk,
            weight_doc_gap,
            weight_meeting_followup,
            is_active,
        };
        match db_projects::save_agent_profile(conn, &profile) {
            Ok(_) => ApiResult::ok(()),
            Err(err) => ApiResult::err(err),
        }
    }).await
}

#[tauri::command]
pub async fn delete_agent_profile(
    state: State<'_, DbState>,
    profile_id: i64,
) -> Result<ApiResult<()>, String> {
    with_db(&state.0, move |conn| match db_projects::delete_agent_profile(conn, profile_id) {
        Ok(_) => ApiResult::ok(()),
        Err(err) => ApiResult::err(err),
    }).await
}
