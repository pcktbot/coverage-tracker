use anyhow::{anyhow, Result};
use git2::{BranchType, Cred, FetchOptions, RemoteCallbacks, Repository};
use serde::Serialize;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize)]
pub struct RepoBranch {
    pub name: String,
    pub is_current: bool,
    pub is_remote: bool,
}

pub fn clone_or_pull(url: &str, dest: &Path, token: &str) -> Result<()> {
    if dest.join(".git").exists() {
        pull(dest, token)
    } else {
        clone(url, dest, token)
    }
}

pub fn probe_auth(url: &str, token: &str) -> Result<()> {
    let temp = std::env::temp_dir().join(format!(
        "coverage-manager-git-auth-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
    ));
    let result = clone(url, &temp, token);
    let _ = std::fs::remove_dir_all(&temp);
    result
}

pub fn list_branches(dest: &Path, token: Option<&str>) -> Result<(String, Vec<RepoBranch>)> {
    let repo = Repository::open(dest)?;
    if let Some(token) = token.filter(|token| !token.is_empty()) {
        let _ = fetch_origin(&repo, token);
    }

    let current = current_branch_name(&repo);
    let mut branches = Vec::new();

    for branch in repo.branches(Some(BranchType::Local))? {
        let (branch, _) = branch?;
        if let Some(name) = branch.name()? {
            branches.push(RepoBranch {
                name: name.to_string(),
                is_current: name == current,
                is_remote: false,
            });
        }
    }

    for branch in repo.branches(Some(BranchType::Remote))? {
        let (branch, _) = branch?;
        if let Some(name) = branch.name()? {
            if let Some(name) = name.strip_prefix("origin/") {
                if name == "HEAD" {
                    continue;
                }
                if branches.iter().any(|b| b.name == name) {
                    continue;
                }
                branches.push(RepoBranch {
                    name: name.to_string(),
                    is_current: name == current,
                    is_remote: true,
                });
            }
        }
    }

    branches.sort_by(|a, b| a.name.cmp(&b.name));
    Ok((current, branches))
}

pub fn checkout_branch(dest: &Path, branch_name: &str, token: Option<&str>) -> Result<()> {
    let repo = Repository::open(dest)?;
    if let Some(token) = token.filter(|token| !token.is_empty()) {
        let _ = fetch_origin(&repo, token);
    }

    if repo.find_branch(branch_name, BranchType::Local).is_err() {
        let remote_ref = format!("refs/remotes/origin/{branch_name}");
        let remote = repo
            .find_reference(&remote_ref)
            .map_err(|_| anyhow!("Branch `{branch_name}` was not found locally or on origin"))?;
        let commit = remote.peel_to_commit()?;
        repo.branch(branch_name, &commit, false)?;
    }

    let refname = format!("refs/heads/{branch_name}");
    repo.set_head(&refname)?;
    repo.checkout_head(Some(git2::build::CheckoutBuilder::default().safe()))?;
    Ok(())
}

fn make_callbacks(token: &str) -> RemoteCallbacks<'_> {
    let token = token.to_string();
    let mut callbacks = RemoteCallbacks::new();
    callbacks.credentials(move |_url, _username_from_url, _allowed_types| {
        Cred::userpass_plaintext("x-access-token", &token)
    });
    callbacks
}

fn fetch_origin(repo: &Repository, token: &str) -> Result<()> {
    let mut remote = repo.find_remote("origin")?;
    let mut fo = FetchOptions::new();
    fo.remote_callbacks(make_callbacks(token));
    remote.fetch(&["refs/heads/*:refs/remotes/origin/*"], Some(&mut fo), None)?;
    Ok(())
}

fn clone(url: &str, dest: &Path, token: &str) -> Result<()> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut fo = FetchOptions::new();
    fo.remote_callbacks(make_callbacks(token));
    let mut builder = git2::build::RepoBuilder::new();
    builder.fetch_options(fo);
    builder.clone(url, dest)?;
    Ok(())
}

fn pull(dest: &Path, token: &str) -> Result<()> {
    let repo = Repository::open(dest)?;
    fetch_origin(&repo, token)?;
    let fetch_head = repo.find_reference("FETCH_HEAD")?;
    let fetch_commit = repo.reference_to_annotated_commit(&fetch_head)?;
    let (analysis, _) = repo.merge_analysis(&[&fetch_commit])?;
    if analysis.is_fast_forward() {
        let refname = format!("refs/heads/{}", default_branch_name(&repo));
        if let Ok(mut r) = repo.find_reference(&refname) {
            r.set_target(fetch_commit.id(), "Fast-forward")?;
            repo.set_head(&refname)?;
            repo.checkout_head(Some(git2::build::CheckoutBuilder::default().force()))?;
        }
    }
    Ok(())
}

fn default_branch_name(repo: &Repository) -> String {
    if let Ok(head) = repo.head() {
        if let Some(name) = head.shorthand() {
            return name.to_string();
        }
    }
    "main".to_string()
}

fn current_branch_name(repo: &Repository) -> String {
    if let Ok(head) = repo.head() {
        if let Some(name) = head.shorthand() {
            return name.to_string();
        }
    }
    default_branch_name(repo)
}

/// Read .ruby-version (or .tool-versions) from repo root, if present.
pub fn read_ruby_version(dest: &Path) -> Option<String> {
    crate::version_manager::read_ruby_version(dest)
}

/// Read .node-version, .nvmrc, or .tool-versions from repo root, if present.
pub fn read_node_version(dest: &Path) -> Option<String> {
    crate::version_manager::read_node_version(dest)
}
