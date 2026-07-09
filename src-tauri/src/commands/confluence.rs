use crate::commands::repos::{with_db, ApiResult, DbState};
use crate::confluence::{ConfluenceClient, ConfluenceConfig, ConfluencePage, ConfluencePageSummary, ConfluenceSpace};
use crate::db::confluence as db_confluence;
use crate::db::repos as db_repos;
use tauri::State;

fn load_config(conn: &rusqlite::Connection) -> Result<ConfluenceConfig, String> {
    let base_url = db_repos::get_setting(conn, "confluence_base_url")
        .map_err(|e| e.to_string())?
        .unwrap_or_default();
    let username = db_repos::get_setting(conn, "confluence_username")
        .map_err(|e| e.to_string())?
        .unwrap_or_default();
    let token = db_repos::get_setting(conn, "confluence_token")
        .map_err(|e| e.to_string())?
        .unwrap_or_default();
    Ok(ConfluenceConfig { base_url, username, token })
}

#[tauri::command]
pub async fn confluence_get_space(
    state: State<'_, DbState>,
    space_key: String,
) -> Result<ApiResult<ConfluenceSpace>, String> {
    let config = with_db(&state.0, load_config).await??;
    let client = ConfluenceClient::new(config).map_err(|e| e.to_string())?;
    match client.get_space(&space_key).await {
        Ok(space) => Ok(ApiResult::ok(space)),
        Err(err) => Ok(ApiResult::err(err)),
    }
}

#[tauri::command]
pub async fn confluence_get_page(
    state: State<'_, DbState>,
    page_id: String,
) -> Result<ApiResult<ConfluencePage>, String> {
    let config = with_db(&state.0, load_config).await??;
    let client = ConfluenceClient::new(config).map_err(|e| e.to_string())?;
    match client.get_page(&page_id).await {
        Ok(page) => Ok(ApiResult::ok(page)),
        Err(err) => Ok(ApiResult::err(err)),
    }
}

#[tauri::command]
pub async fn confluence_refresh_page(
    state: State<'_, DbState>,
    page_id: String,
) -> Result<ApiResult<db_confluence::CachedConfluencePage>, String> {
    let config = with_db(&state.0, load_config).await??;
    let client = ConfluenceClient::new(config).map_err(|e| e.to_string())?;
    let page = match client.get_page(&page_id).await {
        Ok(page) => page,
        Err(err) => return Ok(ApiResult::err(err)),
    };

    with_db(&state.0, move |conn| {
        let cached = db_confluence::CachedConfluencePage {
            page_id: page.id,
            space_key: page.space_key,
            title: page.title,
            web_url: page.web_url,
            version_number: page.version_number,
            last_updated_at: page.last_updated_at,
            last_fetched_at: String::new(),
            raw_html: page.body_html,
            plain_text: page.plain_text,
            excerpt: page.excerpt,
        };
        match db_confluence::upsert_page(conn, &cached).and_then(|_| {
            db_confluence::get_page(conn, &cached.page_id)?.ok_or_else(|| anyhow::anyhow!("Cached page missing after refresh"))
        }) {
            Ok(saved) => ApiResult::ok(saved),
            Err(err) => ApiResult::err(err),
        }
    }).await
}

#[tauri::command]
pub async fn confluence_get_cached_page(
    state: State<'_, DbState>,
    page_id: String,
) -> Result<ApiResult<Option<db_confluence::CachedConfluencePage>>, String> {
    with_db(&state.0, move |conn| match db_confluence::get_page(conn, &page_id) {
        Ok(page) => ApiResult::ok(page),
        Err(err) => ApiResult::err(err),
    }).await
}

#[tauri::command]
pub async fn confluence_list_cached_pages(
    state: State<'_, DbState>,
    page_ids: Vec<String>,
) -> Result<ApiResult<Vec<db_confluence::CachedConfluencePage>>, String> {
    with_db(&state.0, move |conn| match db_confluence::list_pages(conn, &page_ids) {
        Ok(pages) => ApiResult::ok(pages),
        Err(err) => ApiResult::err(err),
    }).await
}

#[tauri::command]
pub async fn confluence_search_pages(
    state: State<'_, DbState>,
    query: String,
    space_key: Option<String>,
    limit: Option<usize>,
) -> Result<ApiResult<Vec<ConfluencePageSummary>>, String> {
    let config = with_db(&state.0, load_config).await??;
    let client = ConfluenceClient::new(config).map_err(|e| e.to_string())?;
    match client
        .search_pages(&query, space_key.as_deref(), limit.unwrap_or(10))
        .await
    {
        Ok(results) => Ok(ApiResult::ok(results)),
        Err(err) => Ok(ApiResult::err(err)),
    }
}
