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

    /// List all non-archived repos in an org that have `simplecov` in their Gemfile.
    pub fn list_simplecov_repos(&self, org: &str) -> Result<Vec<GhRepo>> {
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
                if repo.archived || repo.fork {
                    continue;
                }
                if self.has_simplecov(org, &repo.name).unwrap_or(false) {
                    repos.push(repo);
                }
            }
            page += 1;
        }
        Ok(repos)
    }

    fn has_simplecov(&self, org: &str, repo: &str) -> Result<bool> {
        let url = format!(
            "{}/repos/{}/{}/contents/Gemfile",
            GITHUB_API, org, repo
        );

        #[derive(Deserialize)]
        struct Contents { content: String }

        match self.get_json::<Contents>(&url) {
            Ok(c) => {
                // content is base64-encoded
                let decoded = general_purpose_decode(&c.content)?;
                Ok(decoded.contains("simplecov"))
            }
            Err(_) => Ok(false), // no Gemfile → not a Rails app
        }
    }
}

fn general_purpose_decode(b64: &str) -> Result<String> {
    // GitHub adds newlines to base64 — strip them
    let clean: String = b64.chars().filter(|c| !c.is_whitespace()).collect();
    let bytes = base64_decode(&clean)?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

fn base64_decode(input: &str) -> Result<Vec<u8>> {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut lookup = [255u8; 256];
    for (i, &c) in TABLE.iter().enumerate() {
        lookup[c as usize] = i as u8;
    }
    let input = input.trim_end_matches('=');
    let mut out = Vec::with_capacity(input.len() * 3 / 4);
    let bytes = input.as_bytes();
    let mut i = 0;
    while i + 3 < bytes.len() {
        let a = lookup[bytes[i] as usize];
        let b = lookup[bytes[i+1] as usize];
        let c = lookup[bytes[i+2] as usize];
        let d = lookup[bytes[i+3] as usize];
        if a == 255 || b == 255 { break; }
        out.push((a << 2) | (b >> 4));
        if c != 255 { out.push((b << 4) | (c >> 2)); }
        if d != 255 { out.push((c << 6) | d); }
        i += 4;
    }
    // handle remaining
    if i + 1 < bytes.len() {
        let a = lookup[bytes[i] as usize];
        let b = lookup[bytes[i+1] as usize];
        if a != 255 && b != 255 {
            out.push((a << 2) | (b >> 4));
        }
    }
    if i + 2 < bytes.len() {
        let b = lookup[bytes[i+1] as usize];
        let c = lookup[bytes[i+2] as usize];
        if b != 255 && c != 255 {
            out.push((b << 4) | (c >> 2));
        }
    }
    Ok(out)
}
