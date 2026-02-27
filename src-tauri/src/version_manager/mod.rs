//! Unified runtime version management.
//!
//! Auto-detects the version manager installed on the host machine and provides
//! a single [`RuntimeEnv`] that any runner can apply to a [`std::process::Command`].
//!
//! Detection order (first match wins):
//!   mise → asdf → rbenv/nodenv → nvm → system PATH
//!
//! Version file priority per-repo:
//!   `.tool-versions`  (asdf / mise)
//!   `.ruby-version`   (rbenv)
//!   `.node-version`   (nodenv)
//!   `.nvmrc`          (nvm)
//!
//! This module replaces the ad-hoc PATH/env manipulation that was previously
//! duplicated inside `ruby::runner` and `node::runner`.

use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

// ── Public types ──────────────────────────────────────────────────────────────

/// Which runtime we need.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Runtime {
    Ruby,
    Node,
}

/// Which version manager we detected on the host.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VersionManager {
    /// https://mise.jdx.dev  (née rtx, modern asdf successor)
    Mise,
    /// https://asdf-vm.com
    Asdf,
    /// rbenv (Ruby) or nodenv (Node) — same shim pattern.
    XEnv,
    /// Node Version Manager (Node only).
    Nvm,
    /// No version manager found; use whatever is on PATH.
    System,
}

/// Everything a runner needs to apply to a `Command` so the right runtime
/// version is used.  Obtain one via [`RuntimeEnv::detect`].
#[derive(Debug, Clone)]
pub struct RuntimeEnv {
    pub manager: VersionManager,
    /// Extra env vars to set on the child process (e.g. `RBENV_VERSION`).
    pub env_vars: HashMap<String, String>,
    /// The full `PATH` value to set, or `None` to inherit.
    pub path: Option<String>,
    /// An optional shell prefix to prepend **inside** `bash -c "…"` commands.
    /// Used by nvm which must be sourced in every shell invocation.
    pub shell_prefix: Option<String>,
}

impl RuntimeEnv {
    // ── Constructor ───────────────────────────────────────────────────────

    /// Detect the best available version manager for `runtime` and, optionally,
    /// pin it to the given `version` (read from the repo's version file).
    pub fn detect(runtime: Runtime, version: Option<&str>) -> Self {
        let home = std::env::var("HOME").unwrap_or_default();
        let existing_path = std::env::var("PATH").unwrap_or_default();

        // 1. mise  ──────────────────────────────────────────────────────
        if let Some(env) = Self::try_mise(&home, &existing_path, runtime, version) {
            return env;
        }

        // 2. asdf  ──────────────────────────────────────────────────────
        if let Some(env) = Self::try_asdf(&home, &existing_path, runtime, version) {
            return env;
        }

        // 3. rbenv / nodenv  ────────────────────────────────────────────
        if let Some(env) = Self::try_xenv(&home, &existing_path, runtime, version) {
            return env;
        }

        // 4. nvm (Node only) ───────────────────────────────────────────
        if runtime == Runtime::Node {
            if let Some(env) = Self::try_nvm(&home, &existing_path, version) {
                return env;
            }
        }

        // 5. System fallback ───────────────────────────────────────────
        let mut path = existing_path;
        // On macOS, Homebrew paths might not be in PATH inside a GUI app.
        for p in &["/opt/homebrew/bin", "/usr/local/bin"] {
            if !path.contains(p) {
                path = format!("{}:{}", p, path);
            }
        }
        Self {
            manager: VersionManager::System,
            env_vars: HashMap::new(),
            path: Some(path),
            shell_prefix: None,
        }
    }

    // ── Apply to a Command ────────────────────────────────────────────────

    /// Set env vars and PATH on an already-created `Command`.
    pub fn apply(&self, cmd: &mut Command) {
        for (k, v) in &self.env_vars {
            cmd.env(k, v);
        }
        if let Some(ref p) = self.path {
            cmd.env("PATH", p);
        }
    }

    /// Build a `bash -c "…"` `Command`, automatically prepending the
    /// `shell_prefix` (e.g. nvm source) when needed.
    pub fn bash_command(&self, repo_path: &Path, shell_cmd: &str) -> Command {
        let full_cmd = match &self.shell_prefix {
            Some(prefix) => format!("{}{} 2>&1", prefix, shell_cmd),
            None => format!("{} 2>&1", shell_cmd),
        };

        let mut cmd = Command::new("bash");
        cmd.arg("-c");
        cmd.arg(full_cmd);
        cmd.current_dir(repo_path);
        self.apply(&mut cmd);
        cmd
    }

    // ── Private detection helpers ─────────────────────────────────────────

    /// mise: `~/.local/bin/mise` or on PATH.
    /// Shims live at `~/.local/share/mise/shims`.
    fn try_mise(
        home: &str,
        existing_path: &str,
        runtime: Runtime,
        version: Option<&str>,
    ) -> Option<Self> {
        let mise_bin = format!("{home}/.local/bin/mise");
        let mise_shims = format!("{home}/.local/share/mise/shims");

        // Also check Homebrew-installed mise
        let brew_mise = if cfg!(target_arch = "aarch64") {
            "/opt/homebrew/bin/mise"
        } else {
            "/usr/local/bin/mise"
        };

        let has_mise = Path::new(&mise_bin).exists() || Path::new(brew_mise).exists();
        if !has_mise {
            return None;
        }

        let mut env_vars = HashMap::new();
        let mut path_parts = Vec::new();

        // mise uses its own shim directory
        if Path::new(&mise_shims).exists() {
            path_parts.push(mise_shims);
        }
        // Also ensure mise's own bin is on PATH
        let mise_bin_dir = format!("{home}/.local/bin");
        if Path::new(&mise_bin_dir).exists() {
            path_parts.push(mise_bin_dir);
        }
        path_parts.push(existing_path.to_string());

        // mise (like asdf) respects .tool-versions, .ruby-version, .node-version etc.
        // We can also force a version via env var.
        if let Some(ver) = version {
            match runtime {
                Runtime::Ruby => { env_vars.insert("MISE_RUBY_VERSION".into(), ver.into()); }
                Runtime::Node => { env_vars.insert("MISE_NODE_VERSION".into(), ver.into()); }
            }
        }

        Some(Self {
            manager: VersionManager::Mise,
            env_vars,
            path: Some(path_parts.join(":")),
            shell_prefix: None,
        })
    }

    /// asdf: `~/.asdf/shims` on PATH, version set via `ASDF_RUBY_VERSION` /
    /// `ASDF_NODEJS_VERSION` env var.
    fn try_asdf(
        home: &str,
        existing_path: &str,
        runtime: Runtime,
        version: Option<&str>,
    ) -> Option<Self> {
        let asdf_dir = std::env::var("ASDF_DIR")
            .unwrap_or_else(|_| format!("{home}/.asdf"));

        if !Path::new(&asdf_dir).exists() {
            return None;
        }

        let shims = format!("{asdf_dir}/shims");
        let bin = format!("{asdf_dir}/bin");

        let mut env_vars = HashMap::new();
        env_vars.insert("ASDF_DIR".into(), asdf_dir.clone());

        if let Some(ver) = version {
            match runtime {
                Runtime::Ruby => { env_vars.insert("ASDF_RUBY_VERSION".into(), ver.into()); }
                Runtime::Node => { env_vars.insert("ASDF_NODEJS_VERSION".into(), ver.into()); }
            }
        }

        let path = format!("{}:{}:{}", shims, bin, existing_path);
        Some(Self {
            manager: VersionManager::Asdf,
            env_vars,
            path: Some(path),
            shell_prefix: None,
        })
    }

    /// rbenv (Ruby) / nodenv (Node).
    fn try_xenv(
        home: &str,
        existing_path: &str,
        runtime: Runtime,
        version: Option<&str>,
    ) -> Option<Self> {
        let (dir_name, env_key) = match runtime {
            Runtime::Ruby => ("rbenv", "RBENV_VERSION"),
            Runtime::Node => ("nodenv", "NODENV_VERSION"),
        };

        let xenv_dir = format!("{home}/.{dir_name}");
        if !Path::new(&xenv_dir).exists() {
            // Also check Homebrew locations
            let brew_bin = if cfg!(target_arch = "aarch64") {
                format!("/opt/homebrew/bin/{dir_name}")
            } else {
                format!("/usr/local/bin/{dir_name}")
            };
            if !Path::new(&brew_bin).exists() {
                return None;
            }
        }

        let shims = format!("{xenv_dir}/shims");
        let bin = format!("{xenv_dir}/bin");

        let mut env_vars = HashMap::new();
        if let Some(ver) = version {
            env_vars.insert(env_key.into(), ver.into());
        }

        let path = format!("{}:{}:{}", shims, bin, existing_path);
        Some(Self {
            manager: VersionManager::XEnv,
            env_vars,
            path: Some(path),
            shell_prefix: None,
        })
    }

    /// nvm (Node only) — needs to be sourced in every bash invocation.
    fn try_nvm(
        home: &str,
        _existing_path: &str,
        version: Option<&str>,
    ) -> Option<Self> {
        let nvm_dir = std::env::var("NVM_DIR")
            .unwrap_or_else(|_| format!("{home}/.nvm"));

        if !Path::new(&nvm_dir).exists() {
            return None;
        }

        let use_clause = version
            .map(|v| format!("nvm use {v} > /dev/null 2>&1; "))
            .unwrap_or_default();

        let prefix = format!(
            "export NVM_DIR=\"{nvm_dir}\"; [ -s \"$NVM_DIR/nvm.sh\" ] && . \"$NVM_DIR/nvm.sh\"; {use_clause}"
        );

        Some(Self {
            manager: VersionManager::Nvm,
            env_vars: HashMap::new(),
            path: None, // nvm sets PATH itself when sourced
            shell_prefix: Some(prefix),
        })
    }
}

// ── Version file reading ──────────────────────────────────────────────────────

/// Read the desired Ruby version from a repo directory.
/// Checks `.tool-versions` first (asdf/mise), then `.ruby-version` (rbenv).
pub fn read_ruby_version(repo_path: &Path) -> Option<String> {
    // .tool-versions (asdf / mise)
    if let Some(ver) = read_tool_version(repo_path, "ruby") {
        return Some(ver);
    }
    // .ruby-version (rbenv)
    read_file_trimmed(&repo_path.join(".ruby-version"))
}

/// Read the desired Node version from a repo directory.
/// Checks `.tool-versions` first (asdf/mise), then `.node-version` (nodenv),
/// then `.nvmrc` (nvm).
pub fn read_node_version(repo_path: &Path) -> Option<String> {
    // .tool-versions (asdf / mise)
    if let Some(ver) = read_tool_version(repo_path, "nodejs") {
        return Some(normalize_node_version(&ver));
    }
    // .node-version (nodenv)
    if let Some(ver) = read_file_trimmed(&repo_path.join(".node-version")) {
        return Some(normalize_node_version(&ver));
    }
    // .nvmrc (nvm)
    if let Some(ver) = read_file_trimmed(&repo_path.join(".nvmrc")) {
        return Some(normalize_node_version(&ver));
    }
    None
}

/// Parse a specific tool's version from `.tool-versions`.
/// Format: `ruby 3.2.2` or `nodejs 20.11.0` (one tool per line).
fn read_tool_version(repo_path: &Path, tool_name: &str) -> Option<String> {
    let content = std::fs::read_to_string(repo_path.join(".tool-versions")).ok()?;
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut parts = line.split_whitespace();
        if let (Some(name), Some(version)) = (parts.next(), parts.next()) {
            if name == tool_name {
                return Some(version.to_string());
            }
        }
    }
    None
}

/// Strip leading 'v' (e.g. "v18.17.0" → "18.17.0").
fn normalize_node_version(ver: &str) -> String {
    ver.strip_prefix('v').unwrap_or(ver).to_string()
}

/// Read a file, trim whitespace, return None if empty or missing.
fn read_file_trimmed(path: &Path) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn parse_tool_versions_ruby() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join(".tool-versions"), "ruby 3.2.2\nnodejs 20.11.0\n").unwrap();
        assert_eq!(read_ruby_version(dir.path()), Some("3.2.2".into()));
        assert_eq!(read_node_version(dir.path()), Some("20.11.0".into()));
    }

    #[test]
    fn falls_back_to_ruby_version_file() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join(".ruby-version"), "3.1.4\n").unwrap();
        assert_eq!(read_ruby_version(dir.path()), Some("3.1.4".into()));
    }

    #[test]
    fn falls_back_to_nvmrc() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join(".nvmrc"), "v18.17.0\n").unwrap();
        assert_eq!(read_node_version(dir.path()), Some("18.17.0".into()));
    }

    #[test]
    fn tool_versions_takes_priority() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join(".tool-versions"), "ruby 3.3.0\n").unwrap();
        fs::write(dir.path().join(".ruby-version"), "3.1.0\n").unwrap();
        assert_eq!(read_ruby_version(dir.path()), Some("3.3.0".into()));
    }
}
