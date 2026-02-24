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
            return Err(anyhow!("GitHub auth failed ({}). Check your Personal Access Token in Settings.", status));
        }
        if status == 429 {
            return Err(anyhow!("GitHub API rate limit exceeded. Please wait before retrying."));
        }
        if !status.is_success() {
            return Err(anyhow!("GitHub API error: {}", status));
        }
        Ok(resp.json()?)
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

