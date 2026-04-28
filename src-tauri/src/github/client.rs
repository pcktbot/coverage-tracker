use anyhow::{anyhow, Result};
use serde::Deserialize;

const GITHUB_API: &str = "https://api.github.com";

#[derive(Debug, Clone)]
pub struct GithubClient {
    token: String,
    client: reqwest::blocking::Client,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct GhRepo {
    pub name: String,
    pub html_url: String,
    pub clone_url: String,
    pub archived: bool,
    pub fork: bool,
    pub default_branch: String,
}

#[derive(Debug, Clone, Deserialize)]
struct GhViewer {
    login: String,
}

impl GithubClient {
    pub fn new(token: &str) -> Self {
        let client = reqwest::blocking::Client::builder()
            .user_agent("coverage-manager/0.1")
            .build()
            .expect("failed to build http client");
        Self { token: token.to_string(), client }
    }

    fn get_json<T: for<'de> Deserialize<'de>>(&self, url: &str) -> Result<T> {
        let resp = self
            .client
            .get(url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .send()?;

        let status = resp.status();
        if status == 401 || status == 403 {
            let sso = resp
                .headers()
                .get("x-github-sso")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("");
            let msg = if !sso.is_empty() {
                format!(
                    "GitHub auth failed ({}). The token appears to need org SSO authorization. GitHub returned x-github-sso: {}.",
                    status, sso
                )
            } else if status == 403 {
                "GitHub auth failed (403). The token may be missing repo access or may need org SSO authorization.".to_string()
            } else {
                format!("GitHub auth failed ({}). Check your Personal Access Token in Settings.", status)
            };
            return Err(anyhow!(msg));
        }
        if status == 429 {
            return Err(anyhow!("GitHub API rate limit exceeded. Please wait before retrying."));
        }
        if !status.is_success() {
            return Err(anyhow!("GitHub API error: {}", status));
        }
        Ok(resp.json()?)
    }

    pub fn get_viewer_login(&self) -> Result<String> {
        let viewer: GhViewer = self.get_json(&format!("{}/user", GITHUB_API))?;
        Ok(viewer.login)
    }

    /// List all non-archived, non-fork repos in an org (fast — no Gemfile checks).
    pub fn list_all_repos(&self, org: &str) -> Result<Vec<GhRepo>> {
        let mut repos: Vec<GhRepo> = Vec::new();
        let mut page = 1u32;
        loop {
            let url = format!(
                "{}/orgs/{}/repos?type=all&per_page=100&page={}",
                GITHUB_API, org, page
            );
            let batch: Vec<GhRepo> = self.get_json(&url)?;
            if batch.is_empty() {
                break;
            }
            for repo in batch {
                if !repo.archived && !repo.fork {
                    repos.push(repo);
                }
            }
            page += 1;
        }
        Ok(repos)
    }
}
