use anyhow::{anyhow, Result};
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};

#[derive(Debug)]
pub struct RspecResult {
    pub exit_code: i32,
    pub stderr: String,
}

/// Run `bundle exec rspec` in the given repo directory.
/// `ruby_version` is the version string from `.ruby-version` (used with rbenv).
/// `on_line` is a callback called for each line of stdout/stderr output.
pub fn run_rspec<F>(
    repo_path: &Path,
    ruby_version: Option<&str>,
    mut on_line: F,
) -> Result<RspecResult>
where
    F: FnMut(&str),
{
    // Verify rbenv is available
    let rbenv_path = find_rbenv()?;

    // Check if requested ruby version is installed; surface helpful error if not
    if let Some(ver) = ruby_version {
        let installed = rbenv_versions(&rbenv_path)?;
        if !installed.iter().any(|v| v == ver) {
            return Err(anyhow!(
                "Ruby {} is not installed via rbenv.\nRun: rbenv install {}",
                ver,
                ver
            ));
        }
    }

    // Build the shell command:
    //   set -a; source .env; set +a; bundle exec rspec
    // We use bash -c so we can chain the env loading.
    let env_setup = if repo_path.join(".env").exists() {
        "set -a; source .env; set +a; "
    } else {
        ""
    };
    let cmd_str = format!("{}bundle exec rspec --format progress 2>&1", env_setup);

    let mut cmd = Command::new("bash");
    cmd.arg("-c");
    cmd.arg(&cmd_str);
    cmd.current_dir(repo_path);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    // Set RBENV_VERSION so rbenv picks the right ruby without needing `rbenv shell`
    if let Some(ver) = ruby_version {
        cmd.env("RBENV_VERSION", ver);
    }

    // Ensure rbenv shims are on PATH
    if let Ok(home) = std::env::var("HOME") {
        let shims = format!("{home}/.rbenv/shims:{home}/.rbenv/bin", home = home);
        let existing_path = std::env::var("PATH").unwrap_or_default();
        cmd.env("PATH", format!("{}:{}", shims, existing_path));
    }

    let mut child = cmd.spawn().map_err(|e| anyhow!("Failed to spawn rspec: {}", e))?;

    let stdout = child.stdout.take().unwrap();
    let stderr_handle = child.stderr.take().unwrap();

    // Collect stderr in a thread
    let stderr_lines = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
    let stderr_clone = stderr_lines.clone();
    let stderr_thread = std::thread::spawn(move || {
        let reader = BufReader::new(stderr_handle);
        for line in reader.lines().flatten() {
            stderr_clone.lock().unwrap().push(line);
        }
    });

    // Stream stdout
    let reader = BufReader::new(stdout);
    for line in reader.lines().flatten() {
        on_line(&line);
    }

    stderr_thread.join().ok();

    let status = child.wait()?;
    let exit_code = status.code().unwrap_or(-1);
    let stderr = stderr_lines.lock().unwrap().join("\n");

    Ok(RspecResult { exit_code, stderr })
}

fn find_rbenv() -> Result<String> {
    // Try common locations
    let candidates = vec![
        std::env::var("HOME")
            .map(|h| format!("{}/.rbenv/bin/rbenv", h))
            .unwrap_or_default(),
        "/opt/homebrew/bin/rbenv".to_string(),
        "/usr/local/bin/rbenv".to_string(),
    ];
    for candidate in &candidates {
        if Path::new(candidate).exists() {
            return Ok(candidate.clone());
        }
    }
    // Fall back to PATH
    which("rbenv").ok_or_else(|| {
        anyhow!("rbenv not found. Please install rbenv: https://github.com/rbenv/rbenv")
    })
}

fn rbenv_versions(rbenv: &str) -> Result<Vec<String>> {
    let output = Command::new(rbenv).arg("versions").arg("--bare").output()?;
    let text = String::from_utf8_lossy(&output.stdout);
    Ok(text.lines().map(|l| l.trim().to_string()).collect())
}

fn which(cmd: &str) -> Option<String> {
    Command::new("which")
        .arg(cmd)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
}
