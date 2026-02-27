use anyhow::Result;
use git2::{Cred, FetchOptions, RemoteCallbacks, Repository};
use std::path::Path;

pub fn clone_or_pull(url: &str, dest: &Path, token: &str) -> Result<()> {
    if dest.join(".git").exists() {
        pull(dest, token)
    } else {
        clone(url, dest, token)
    }
}

fn make_callbacks(token: &str) -> RemoteCallbacks<'_> {
    let token = token.to_string();
    let mut callbacks = RemoteCallbacks::new();
    callbacks.credentials(move |_url, _username_from_url, _allowed_types| {
        Cred::userpass_plaintext("x-access-token", &token)
    });
    callbacks
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
    let mut remote = repo.find_remote("origin")?;
    let mut fo = FetchOptions::new();
    fo.remote_callbacks(make_callbacks(token));
    remote.fetch(&["HEAD"], Some(&mut fo), None)?;
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

/// Read .ruby-version (or .tool-versions) from repo root, if present.
pub fn read_ruby_version(dest: &Path) -> Option<String> {
    crate::version_manager::read_ruby_version(dest)
}

/// Read .node-version, .nvmrc, or .tool-versions from repo root, if present.
pub fn read_node_version(dest: &Path) -> Option<String> {
    crate::version_manager::read_node_version(dest)
}
