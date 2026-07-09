use crate::commands::repos::{with_db, ApiResult, DbState};
use crate::db::repos as db_repos;
use reqwest::header::{CONTENT_TYPE, HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize};
use tauri::State;

const ANTHROPIC_API_URL: &str = "https://api.anthropic.com/v1/messages";
const ANTHROPIC_VERSION: &str = "2023-06-01";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Serialize)]
struct AnthropicRequest {
    model: String,
    max_tokens: u32,
    system: String,
    messages: Vec<AnthropicMessage>,
}

#[derive(Debug, Serialize)]
struct AnthropicMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct AnthropicResponse {
    content: Vec<AnthropicContentBlock>,
}

#[derive(Debug, Deserialize)]
struct AnthropicContentBlock {
    #[serde(rename = "type")]
    kind: String,
    text: Option<String>,
}

#[tauri::command]
pub async fn send_ai_message(
    state: State<'_, DbState>,
    messages: Vec<AIChatMessage>,
    context_lines: Vec<String>,
) -> Result<ApiResult<String>, String> {
    let settings = with_db(&state.0, |conn| {
        let anthropic_api_key = db_repos::get_setting(conn, "anthropic_api_key").unwrap_or(None).unwrap_or_default();
        let anthropic_model = db_repos::get_setting(conn, "anthropic_model").unwrap_or(None).unwrap_or_else(|| "claude-sonnet-4-5".into());
        let ai_system_prompt = db_repos::get_setting(conn, "ai_system_prompt").unwrap_or(None).unwrap_or_else(|| {
            "Focus on prioritization, blockers, missing context, and next checks.".into()
        });
        (anthropic_api_key, anthropic_model, ai_system_prompt)
    }).await?;

    let (api_key, model, system_prompt) = settings;

    if api_key.trim().is_empty() {
        return Ok(ApiResult::err("Anthropic API key not configured. Add it in Settings."));
    }

    let mut headers = HeaderMap::new();
    headers.insert("x-api-key", HeaderValue::from_str(api_key.trim()).map_err(|e| e.to_string())?);
    headers.insert("anthropic-version", HeaderValue::from_static(ANTHROPIC_VERSION));
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

    let system = if context_lines.is_empty() {
        system_prompt
    } else {
        format!(
            "{system_prompt}\n\nCurrent app context:\n{}",
            context_lines
                .iter()
                .map(|line| format!("- {line}"))
                .collect::<Vec<_>>()
                .join("\n")
        )
    };

    let request = AnthropicRequest {
        model: model.trim().to_string(),
        max_tokens: 1024,
        system,
        messages: messages
            .iter()
            .enumerate()
            .map(|(index, message)| {
                let is_latest_user = index == messages.len().saturating_sub(1) && message.role == "user";
                let content = if is_latest_user && !context_lines.is_empty() {
                    format!(
                        "Current app context:\n{}\n\nUser request:\n{}",
                        context_lines
                            .iter()
                            .map(|line| format!("- {line}"))
                            .collect::<Vec<_>>()
                            .join("\n"),
                        message.content
                    )
                } else {
                    message.content.clone()
                };

                AnthropicMessage {
                    role: message.role.clone(),
                    content,
                }
            })
            .collect(),
    };

    let client = reqwest::Client::new();
    let response = client
        .post(ANTHROPIC_API_URL)
        .headers(headers)
        .json(&request)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Ok(ApiResult::err(format!("Anthropic request failed ({status}): {body}")));
    }

    let payload: AnthropicResponse = response.json().await.map_err(|e| e.to_string())?;
    let text = payload
        .content
        .into_iter()
        .filter(|block| block.kind == "text")
        .filter_map(|block| block.text)
        .collect::<Vec<_>>()
        .join("\n\n");

    if text.trim().is_empty() {
        return Ok(ApiResult::err("Anthropic response did not include text content."));
    }

    Ok(ApiResult::ok(text))
}
