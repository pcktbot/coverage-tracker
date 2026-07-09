use crate::ado::{AdoClient, AdoConfig, AdoPreview, AdoRelease, AdoReleaseDefinition, AdoWorkItemsQuery, AdoWorkItem};
use crate::commands::repos::{with_db, ApiResult, DbState};
use crate::db::projects as db_projects;
use crate::db::repos as db_repos;
use tauri::State;

fn load_config(conn: &rusqlite::Connection) -> Result<AdoConfig, String> {
    let base_url = db_repos::get_setting(conn, "tfs_base_url")
        .map_err(|e| e.to_string())?
        .unwrap_or_default();
    let token = db_repos::get_setting(conn, "tfs_pat")
        .map_err(|e| e.to_string())?
        .unwrap_or_default();
    let collection = db_repos::get_setting(conn, "tfs_collection")
        .map_err(|e| e.to_string())?
        .unwrap_or_default();
    let default_project = db_repos::get_setting(conn, "tfs_default_project")
        .map_err(|e| e.to_string())?
        .unwrap_or_else(|| "Consumer Solutions".into());
    let default_area_path = db_repos::get_setting(conn, "tfs_default_area_path")
        .map_err(|e| e.to_string())?
        .unwrap_or_else(|| "MKT-Websites".into());

    Ok(AdoConfig {
        base_url,
        collection,
        token,
        default_project,
        default_area_path,
    })
}

#[tauri::command]
pub async fn ado_preview(
    state: State<'_, DbState>,
) -> Result<ApiResult<AdoPreview>, String> {
    let config = with_db(&state.0, load_config).await??;
    let client = AdoClient::new(config).map_err(|e| e.to_string())?;
    match client.preview().await {
        Ok(preview) => Ok(ApiResult::ok(preview)),
        Err(err) => Ok(ApiResult::err(err)),
    }
}

#[tauri::command]
pub async fn ado_query_project_work_items(
    state: State<'_, DbState>,
    project_id: i64,
) -> Result<ApiResult<Vec<AdoWorkItem>>, String> {
    let config = with_db(&state.0, load_config).await??;
    let project = with_db(&state.0, move |conn| db_projects::get_project(conn, project_id))
        .await?
        .map_err(|e| e.to_string())?;
    let default_project = config.default_project.clone();
    let default_area_path = config.default_area_path.clone();
    let ado_team = project.ado_team.clone();
    let query = AdoWorkItemsQuery {
        project: default_project,
        team: ado_team.clone().or_else(|| Some(default_area_path.clone())),
        area_path: Some(default_area_path),
        iteration_path: project.ado_iteration_path.clone(),
        states: project.ado_states.clone(),
        tag: project.ado_tag.clone(),
        top: 100,
    };

    let client = AdoClient::new(config).map_err(|e| e.to_string())?;
    match client.query_work_items(query).await {
        Ok((_, work_items)) => Ok(ApiResult::ok(work_items)),
        Err(err) => Ok(ApiResult::err(err)),
    }
}

#[tauri::command]
pub async fn ado_list_release_definitions(
    state: State<'_, DbState>,
) -> Result<ApiResult<Vec<AdoReleaseDefinition>>, String> {
    let config = with_db(&state.0, load_config).await??;
    let default_project = config.default_project.clone();
    let client = AdoClient::new(config).map_err(|e| e.to_string())?;
    match client.list_release_definitions(Some(&default_project), 50).await {
        Ok(definitions) => Ok(ApiResult::ok(definitions)),
        Err(err) => Ok(ApiResult::err(err)),
    }
}

#[tauri::command]
pub async fn ado_list_releases(
    state: State<'_, DbState>,
) -> Result<ApiResult<Vec<AdoRelease>>, String> {
    let config = with_db(&state.0, load_config).await??;
    let default_project = config.default_project.clone();
    let client = AdoClient::new(config).map_err(|e| e.to_string())?;
    match client.list_releases(Some(&default_project), 50).await {
        Ok(releases) => Ok(ApiResult::ok(releases)),
        Err(err) => Ok(ApiResult::err(err)),
    }
}
