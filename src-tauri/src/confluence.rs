use anyhow::{anyhow, Context, Result};
use reqwest::Client;
use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct ConfluenceConfig {
    pub base_url: String,
    pub username: String,
    pub token: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ConfluenceSpace {
    pub id: String,
    pub key: String,
    pub name: String,
    pub homepage_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ConfluencePageSummary {
    pub id: String,
    pub title: String,
    pub space_key: Option<String>,
    pub space_name: Option<String>,
    pub web_url: String,
    pub excerpt: String,
    pub last_updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ConfluencePage {
    pub id: String,
    pub title: String,
    pub space_key: Option<String>,
    pub space_name: Option<String>,
    pub web_url: String,
    pub body_html: String,
    pub plain_text: String,
    pub excerpt: String,
    pub last_updated_at: Option<String>,
    pub version_number: Option<i64>,
}

pub struct ConfluenceClient {
    http: Client,
    config: ConfluenceConfig,
}

impl ConfluenceClient {
    pub fn new(config: ConfluenceConfig) -> Result<Self> {
        if config.base_url.trim().is_empty() {
            return Err(anyhow!("Confluence base URL is not configured."));
        }
        if config.username.trim().is_empty() {
            return Err(anyhow!("Confluence username is not configured."));
        }
        if config.token.trim().is_empty() {
            return Err(anyhow!("Confluence token is not configured."));
        }

        Ok(Self {
            http: Client::new(),
            config: ConfluenceConfig {
                base_url: normalize_base_url(&config.base_url),
                username: config.username.trim().to_string(),
                token: config.token.trim().to_string(),
            },
        })
    }

    pub async fn get_space(&self, space_key: &str) -> Result<ConfluenceSpace> {
        let url = format!("{}/api/v2/spaces?keys={}&limit=1", self.config.base_url, urlencoding::encode(space_key));
        let payload = self.get_json(&url).await?;
        let space = payload
            .get("results")
            .and_then(|results| results.as_array())
            .and_then(|results| results.first())
            .ok_or_else(|| anyhow!("No Confluence space found for key {space_key}"))?;

        Ok(ConfluenceSpace {
            id: value_to_string(space.get("id")),
            key: value_to_string(space.get("key")),
            name: value_to_string(space.get("name")),
            homepage_id: optional_string(space.get("homepageId")),
        })
    }

    pub async fn get_page(&self, page_id: &str) -> Result<ConfluencePage> {
        let url = format!(
            "{}/rest/api/content/{}?expand=body.view,space,version",
            self.config.base_url,
            urlencoding::encode(page_id)
        );
        let payload = self.get_json(&url).await?;
        let body_html = payload
            .pointer("/body/view/value")
            .and_then(|value| value.as_str())
            .unwrap_or_default()
            .to_string();
        let web_url = join_web_url(
            &self.config.base_url,
            payload.pointer("/_links/webui").and_then(|value| value.as_str()),
        );

        Ok(ConfluencePage {
            id: value_to_string(payload.get("id")),
            title: value_to_string(payload.get("title")),
            space_key: optional_string(payload.pointer("/space/key")),
            space_name: optional_string(payload.pointer("/space/name")),
            plain_text: strip_html_full(&body_html),
            excerpt: strip_html(&body_html, 320),
            body_html,
            web_url,
            last_updated_at: optional_string(payload.pointer("/version/when")),
            version_number: payload.pointer("/version/number").and_then(|value| value.as_i64()),
        })
    }

    pub async fn search_pages(
        &self,
        query: &str,
        space_key: Option<&str>,
        limit: usize,
    ) -> Result<Vec<ConfluencePageSummary>> {
        let trimmed_query = query.trim();
        if trimmed_query.is_empty() {
            return Ok(Vec::new());
        }

        let mut cql = String::from("type=page");
        if let Some(space_key) = space_key.filter(|value| !value.trim().is_empty()) {
            cql.push_str(&format!(" and space=\"{}\"", escape_cql(space_key.trim())));
        }
        cql.push_str(&format!(
            " and (title~\"{}\" or text~\"{}\")",
            escape_cql(trimmed_query),
            escape_cql(trimmed_query)
        ));

        let url = format!(
            "{}/rest/api/search?cql={}&limit={}",
            self.config.base_url,
            urlencoding::encode(&cql),
            limit.clamp(1, 25)
        );
        let payload = self.get_json(&url).await?;
        let base_from_links = payload.pointer("/_links/base").and_then(|value| value.as_str());

        let results = payload
            .get("results")
            .and_then(|results| results.as_array())
            .cloned()
            .unwrap_or_default();

        Ok(results
            .into_iter()
            .map(|entry| ConfluencePageSummary {
                id: optional_string(entry.pointer("/content/id")).unwrap_or_default(),
                title: value_to_string(entry.get("title")),
                space_key: optional_string(entry.pointer("/space/key")),
                space_name: optional_string(entry.pointer("/space/name")),
                web_url: join_web_url(
                    base_from_links.unwrap_or(self.config.base_url.as_str()),
                    entry.pointer("/content/_links/webui").and_then(|value| value.as_str()),
                ),
                excerpt: strip_html(entry.get("excerpt").and_then(|value| value.as_str()).unwrap_or_default(), 240),
                last_updated_at: optional_string(entry.pointer("/lastModified")),
            })
            .collect())
    }

    async fn get_json(&self, url: &str) -> Result<Value> {
        let response = self
            .http
            .get(url)
            .basic_auth(&self.config.username, Some(&self.config.token))
            .header("Accept", "application/json")
            .send()
            .await
            .with_context(|| format!("Failed to call Confluence at {url}"))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow!("Confluence API error ({status}): {body}"));
        }

        response.json::<Value>().await.map_err(Into::into)
    }
}

fn normalize_base_url(input: &str) -> String {
    let trimmed = input.trim().trim_end_matches('/');
    if let Some(index) = trimmed.find("/wiki") {
        trimmed[..index + 5].to_string()
    } else {
        trimmed.to_string()
    }
}

fn optional_string(value: Option<&Value>) -> Option<String> {
    value.and_then(|value| match value {
        Value::Null => None,
        Value::String(text) => Some(text.clone()),
        _ => Some(value.to_string().trim_matches('"').to_string()),
    })
}

fn value_to_string(value: Option<&Value>) -> String {
    optional_string(value).unwrap_or_default()
}

fn join_web_url(base: &str, relative: Option<&str>) -> String {
    let relative = relative.unwrap_or_default();
    if relative.starts_with("http://") || relative.starts_with("https://") {
        relative.to_string()
    } else if relative.starts_with('/') {
        format!("{}{}", base.trim_end_matches("/wiki"), relative)
    } else if relative.is_empty() {
        base.to_string()
    } else {
        format!("{}/{}", base.trim_end_matches('/'), relative)
    }
}

fn escape_cql(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn strip_html(input: &str, max_len: usize) -> String {
    let mut output = String::with_capacity(input.len());
    let mut in_tag = false;
    let mut last_was_space = false;

    for ch in input.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if in_tag => {}
            _ if ch.is_whitespace() => {
                if !last_was_space {
                    output.push(' ');
                    last_was_space = true;
                }
            }
            _ => {
                output.push(ch);
                last_was_space = false;
            }
        }
        if output.len() >= max_len {
            break;
        }
    }

    let trimmed = output.trim().to_string();
    if input.len() > max_len && !trimmed.is_empty() {
        format!("{trimmed}…")
    } else {
        trimmed
    }
}

fn strip_html_full(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut in_tag = false;
    let mut last_was_space = false;

    for ch in input.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if in_tag => {}
            _ if ch.is_whitespace() => {
                if !last_was_space {
                    output.push(' ');
                    last_was_space = true;
                }
            }
            _ => {
                output.push(ch);
                last_was_space = false;
            }
        }
    }

    output.trim().to_string()
}
