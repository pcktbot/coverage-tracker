use anyhow::{anyhow, Context, Result};
use reqwest::Client;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;

const API_VERSIONS: [&str; 3] = ["7.1", "6.0", "5.1"];

#[derive(Debug, Clone)]
pub struct AdoConfig {
    pub base_url: String,
    pub collection: String,
    pub token: String,
    pub default_project: String,
    pub default_area_path: String,
}

#[derive(Debug, Clone)]
pub struct AdoWorkItemsQuery {
    pub project: String,
    pub team: Option<String>,
    pub area_path: Option<String>,
    pub iteration_path: Option<String>,
    pub states: Vec<String>,
    pub tag: Option<String>,
    pub top: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct AdoWorkItem {
    pub id: i64,
    pub title: String,
    pub state: String,
    pub work_item_type: String,
    pub area_path: Option<String>,
    pub iteration_path: Option<String>,
    pub assigned_to: Option<String>,
    pub tags: Vec<String>,
    pub url: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct AdoReleaseDefinition {
    pub id: i64,
    pub name: String,
    pub path: Option<String>,
    pub url: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AdoRelease {
    pub id: i64,
    pub name: String,
    pub status: Option<String>,
    pub created_on: Option<String>,
    pub modified_on: Option<String>,
    pub definition_name: Option<String>,
    pub web_url: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AdoPreview {
    pub base_url: String,
    pub collection: String,
    pub project: String,
    pub area_path: String,
    pub api_version: String,
    pub work_items: Vec<AdoWorkItem>,
    pub release_definitions: Vec<AdoReleaseDefinition>,
    pub releases: Vec<AdoRelease>,
}

pub struct AdoClient {
    http: Client,
    config: AdoConfig,
}

impl AdoClient {
    pub fn new(config: AdoConfig) -> Result<Self> {
        if config.base_url.trim().is_empty() {
            return Err(anyhow!("TFS / ADO base URL is not configured."));
        }
        if config.collection.trim().is_empty() {
            return Err(anyhow!("TFS / ADO collection is not configured."));
        }
        if config.token.trim().is_empty() {
            return Err(anyhow!("TFS / ADO PAT is not configured."));
        }

        Ok(Self {
            http: Client::new(),
            config: AdoConfig {
                base_url: config.base_url.trim().trim_end_matches('/').to_string(),
                collection: config.collection.trim().to_string(),
                token: config.token.trim().to_string(),
                default_project: config.default_project.trim().to_string(),
                default_area_path: config.default_area_path.trim().to_string(),
            },
        })
    }

    pub async fn preview(&self) -> Result<AdoPreview> {
        let query = AdoWorkItemsQuery {
            project: self.config.default_project.clone(),
            team: Some(self.config.default_area_path.clone()),
            area_path: Some(self.config.default_area_path.clone()),
            iteration_path: None,
            states: vec!["New".into(), "Active".into(), "Committed".into(), "In Progress".into(), "Blocked".into()],
            tag: None,
            top: 10,
        };

        let (api_version, work_items) = self.query_work_items(query).await?;
        let release_definitions = self
            .list_release_definitions(Some(&self.config.default_project), 10)
            .await
            .unwrap_or_default();
        let releases = self
            .list_releases(Some(&self.config.default_project), 10)
            .await
            .unwrap_or_default();

        Ok(AdoPreview {
            base_url: self.config.base_url.clone(),
            collection: self.config.collection.clone(),
            project: self.config.default_project.clone(),
            area_path: self.config.default_area_path.clone(),
            api_version,
            work_items,
            release_definitions,
            releases,
        })
    }

    pub async fn query_work_items(&self, query: AdoWorkItemsQuery) -> Result<(String, Vec<AdoWorkItem>)> {
        let project = query.project.trim();
        if project.is_empty() {
            return Err(anyhow!("ADO project is required."));
        }

        let wiql = build_wiql(&query);
        let team_segment = query.team.as_deref().filter(|value| !value.trim().is_empty());
        let mut last_err: Option<anyhow::Error> = None;

        for version in API_VERSIONS {
            let url = self.project_api_url(project, team_segment, "wit/wiql", version);
            match self
                .post_json::<WiqlResponse>(&url, &serde_json::json!({ "query": wiql }))
                .await
            {
                Ok(result) => {
                    let ids: Vec<i64> = result.work_items.into_iter().map(|item| item.id).collect();
                    let work_items = self.get_work_items(project, &ids, version).await?;
                    return Ok((version.to_string(), work_items));
                }
                Err(err) => last_err = Some(err),
            }
        }

        Err(last_err.unwrap_or_else(|| anyhow!("Failed to query ADO work items.")))
    }

    pub async fn list_release_definitions(
        &self,
        project: Option<&str>,
        top: usize,
    ) -> Result<Vec<AdoReleaseDefinition>> {
        let project = project
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or(self.config.default_project.as_str());
        let mut last_err: Option<anyhow::Error> = None;

        for version in API_VERSIONS {
            let url = format!(
                "{}?api-version={}&$top={}",
                self.project_api_url(project, None, "release/definitions", version),
                version,
                top.clamp(1, 50)
            );
            match self.get_json::<ListResponse<ReleaseDefinitionPayload>>(&url).await {
                Ok(result) => {
                    return Ok(result
                        .value
                        .into_iter()
                        .map(|item| AdoReleaseDefinition {
                            id: item.id,
                            name: item.name,
                            path: item.path,
                            url: item.url,
                        })
                        .collect());
                }
                Err(err) => last_err = Some(err),
            }
        }

        Err(last_err.unwrap_or_else(|| anyhow!("Failed to list ADO release definitions.")))
    }

    pub async fn list_releases(
        &self,
        project: Option<&str>,
        top: usize,
    ) -> Result<Vec<AdoRelease>> {
        let project = project
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or(self.config.default_project.as_str());
        let mut last_err: Option<anyhow::Error> = None;

        for version in API_VERSIONS {
            let url = format!(
                "{}?api-version={}&$top={}",
                self.project_api_url(project, None, "release/releases", version),
                version,
                top.clamp(1, 50)
            );
            match self.get_json::<ListResponse<ReleasePayload>>(&url).await {
                Ok(result) => {
                    return Ok(result
                        .value
                        .into_iter()
                        .map(|item| AdoRelease {
                            id: item.id,
                            name: item.name,
                            status: item.status,
                            created_on: item.created_on,
                            modified_on: item.modified_on,
                            definition_name: item.release_definition.map(|definition| definition.name),
                            web_url: item._links.and_then(|links| links.web.and_then(|web| web.href)),
                        })
                        .collect());
                }
                Err(err) => last_err = Some(err),
            }
        }

        Err(last_err.unwrap_or_else(|| anyhow!("Failed to list ADO releases.")))
    }

    async fn get_work_items(&self, project: &str, ids: &[i64], version: &str) -> Result<Vec<AdoWorkItem>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }

        let joined_ids = ids.iter().map(|id| id.to_string()).collect::<Vec<_>>().join(",");
        let fields = [
            "System.Id",
            "System.Title",
            "System.State",
            "System.WorkItemType",
            "System.AreaPath",
            "System.IterationPath",
            "System.AssignedTo",
            "System.Tags",
        ]
        .join(",");
        let url = format!(
            "{}?ids={}&fields={}&api-version={}",
            self.project_api_url(project, None, "wit/workitems", version),
            joined_ids,
            urlencoding::encode(&fields),
            version
        );
        let payload: ListResponse<WorkItemPayload> = self.get_json(&url).await?;
        Ok(payload.value.into_iter().map(map_work_item).collect())
    }

    fn project_api_url(&self, project: &str, team: Option<&str>, area: &str, _version: &str) -> String {
        let mut segments = vec![
            self.config.base_url.clone(),
            urlencoding::encode(&self.config.collection).into_owned(),
            urlencoding::encode(project).into_owned(),
        ];
        if let Some(team) = team {
            segments.push(urlencoding::encode(team).into_owned());
        }
        segments.push("_apis".into());
        segments.push(area.into());
        format!("{}/{}", segments.join("/"), if area.contains('?') { "" } else { "" }).trim_end_matches('/').to_string()
    }

    async fn get_json<T: for<'de> Deserialize<'de>>(&self, url: &str) -> Result<T> {
        let response = self
            .http
            .get(url)
            .basic_auth("", Some(&self.config.token))
            .header("Accept", "application/json")
            .send()
            .await
            .with_context(|| format!("Failed to call ADO at {url}"))?;

        parse_response(response).await
    }

    async fn post_json<T: for<'de> Deserialize<'de>>(&self, url: &str, body: &Value) -> Result<T> {
        let response = self
            .http
            .post(url)
            .basic_auth("", Some(&self.config.token))
            .header("Accept", "application/json")
            .json(body)
            .send()
            .await
            .with_context(|| format!("Failed to call ADO at {url}"))?;

        parse_response(response).await
    }
}

async fn parse_response<T: for<'de> Deserialize<'de>>(response: reqwest::Response) -> Result<T> {
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(anyhow!("ADO API error ({status}): {body}"));
    }
    serde_json::from_str(&body).map_err(Into::into)
}

fn build_wiql(query: &AdoWorkItemsQuery) -> String {
    let mut clauses = vec![format!("[System.TeamProject] = '{}'", escape_wiql(&query.project))];

    if let Some(area_path) = query.area_path.as_deref().filter(|value| !value.trim().is_empty()) {
        clauses.push(format!("[System.AreaPath] UNDER '{}'", escape_wiql(area_path.trim())));
    }
    if let Some(iteration_path) = query.iteration_path.as_deref().filter(|value| !value.trim().is_empty()) {
        clauses.push(format!("[System.IterationPath] UNDER '{}'", escape_wiql(iteration_path.trim())));
    }
    if !query.states.is_empty() {
        let states = query
            .states
            .iter()
            .map(|state| format!("'{}'", escape_wiql(state.trim())))
            .collect::<Vec<_>>()
            .join(", ");
        clauses.push(format!("[System.State] IN ({states})"));
    } else {
        clauses.push("[System.State] <> 'Closed'".into());
        clauses.push("[System.State] <> 'Done'".into());
        clauses.push("[System.State] <> 'Removed'".into());
    }
    if let Some(tag) = query.tag.as_deref().filter(|value| !value.trim().is_empty()) {
        clauses.push(format!("[System.Tags] CONTAINS '{}'", escape_wiql(tag.trim())));
    }

    format!(
        "SELECT TOP {} [System.Id], [System.Title], [System.State], [System.WorkItemType], [System.AssignedTo], [System.IterationPath], [System.AreaPath], [System.Tags] \
         FROM WorkItems WHERE {} ORDER BY [System.ChangedDate] DESC",
        query.top.clamp(1, 200),
        clauses.join(" AND ")
    )
}

fn escape_wiql(value: &str) -> String {
    value.replace('\'', "''")
}

fn map_work_item(item: WorkItemPayload) -> AdoWorkItem {
    let fields = item.fields.unwrap_or_default();
    AdoWorkItem {
        id: item.id,
        title: string_field(&fields, "System.Title").unwrap_or_default(),
        state: string_field(&fields, "System.State").unwrap_or_default(),
        work_item_type: string_field(&fields, "System.WorkItemType").unwrap_or_default(),
        area_path: string_field(&fields, "System.AreaPath"),
        iteration_path: string_field(&fields, "System.IterationPath"),
        assigned_to: assigned_to_field(&fields.get("System.AssignedTo")),
        tags: string_field(&fields, "System.Tags")
            .map(|tags| {
                tags.split(';')
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(ToString::to_string)
                    .collect()
            })
            .unwrap_or_default(),
        url: item.url,
    }
}

fn string_field(fields: &serde_json::Map<String, Value>, key: &str) -> Option<String> {
    fields.get(key).and_then(|value| match value {
        Value::Null => None,
        Value::String(text) => Some(text.clone()),
        _ => Some(value.to_string()),
    })
}

fn assigned_to_field(value: &Option<&Value>) -> Option<String> {
    value.and_then(|value| match value {
        Value::String(text) => Some(text.clone()),
        Value::Object(map) => map
            .get("displayName")
            .and_then(|display_name| display_name.as_str())
            .map(ToString::to_string)
            .or_else(|| {
                map.get("uniqueName")
                    .and_then(|unique_name| unique_name.as_str())
                    .map(ToString::to_string)
            }),
        _ => Some(value.to_string()),
    })
}

#[derive(Debug, Deserialize)]
struct WiqlResponse {
    #[serde(default)]
    work_items: Vec<WorkItemReference>,
}

#[derive(Debug, Deserialize)]
struct WorkItemReference {
    id: i64,
}

#[derive(Debug, Deserialize)]
struct ListResponse<T> {
    #[serde(default)]
    value: Vec<T>,
}

#[derive(Debug, Default, Deserialize)]
struct WorkItemPayload {
    id: i64,
    url: String,
    #[serde(default)]
    fields: Option<serde_json::Map<String, Value>>,
}

#[derive(Debug, Default, Deserialize)]
struct ReleaseDefinitionPayload {
    id: i64,
    name: String,
    path: Option<String>,
    url: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct ReleasePayload {
    id: i64,
    name: String,
    status: Option<String>,
    #[serde(rename = "createdOn")]
    created_on: Option<String>,
    #[serde(rename = "modifiedOn")]
    modified_on: Option<String>,
    #[serde(rename = "releaseDefinition")]
    release_definition: Option<NamedReference>,
    #[serde(rename = "_links")]
    _links: Option<ReleaseLinks>,
}

#[derive(Debug, Deserialize)]
struct NamedReference {
    name: String,
}

#[derive(Debug, Deserialize)]
struct ReleaseLinks {
    web: Option<LinkRef>,
}

#[derive(Debug, Deserialize)]
struct LinkRef {
    href: Option<String>,
}
