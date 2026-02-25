use anyhow::{anyhow, Result};
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

#[derive(Debug)]
pub struct RspecResult {
    pub exit_code: i32,
    pub stderr: String,
}

/// Check whether `bundle install` needs to run by comparing Gemfile.lock
/// mtime against a `.bundle_installed` marker file we drop after a successful install.
fn needs_bundle_install(repo_path: &Path) -> bool {
    let lockfile = repo_path.join("Gemfile.lock");
    let marker = repo_path.join(".bundle_installed");
    if !lockfile.exists() {
        // No Gemfile.lock means we definitely need to install
        return repo_path.join("Gemfile").exists();
    }
    match (lockfile.metadata().and_then(|m| m.modified()),
           marker.metadata().and_then(|m| m.modified())) {
        (Ok(lock_t), Ok(mark_t)) => lock_t > mark_t, // re-install if lockfile is newer
        _ => true,                                     // marker missing or unreadable
    }
}

/// Run `bundle install` in the given repo directory, streaming output via `on_line`.
/// Skips if Gemfile.lock hasn't changed since the last successful install.
/// Returns Ok(true) if it ran, Ok(false) if skipped.
pub fn run_bundle_install<F>(
    repo_path: &Path,
    ruby_version: Option<&str>,
    mut on_line: F,
) -> Result<bool>
where
    F: FnMut(&str),
{
    if !needs_bundle_install(repo_path) {
        on_line("[bundle install] skipped — Gemfile.lock unchanged");
        return Ok(false);
    }

    on_line("[bundle install] running…");

    let mut cmd = Command::new("bash");
    cmd.arg("-c");
    cmd.arg("bundle install 2>&1");
    cmd.current_dir(repo_path);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    if let Some(ver) = ruby_version {
        cmd.env("RBENV_VERSION", ver);
    }
    if let Ok(home) = std::env::var("HOME") {
        let shims = format!("{home}/.rbenv/shims:{home}/.rbenv/bin");
        let existing_path = std::env::var("PATH").unwrap_or_default();
        cmd.env("PATH", format!("{}:{}", shims, existing_path));
    }

    let mut child = cmd.spawn().map_err(|e| anyhow!("Failed to spawn bundle install: {}", e))?;
    let stdout = child.stdout.take().unwrap();
    let reader = BufReader::new(stdout);
    for line in reader.lines().flatten() {
        on_line(&line);
    }

    let status = child.wait()?;
    if !status.success() {
        return Err(anyhow!("bundle install exited with code {}", status.code().unwrap_or(-1)));
    }

    // Drop marker file so we skip next time if lockfile hasn't changed
    let _ = std::fs::write(repo_path.join(".bundle_installed"), "");
    on_line("[bundle install] done");
    Ok(true)
}

/// Ensure the test database exists for a Rails app.
/// 1. Copies config/database.example.yml → config/database.yml if missing.
/// 2. Parses the `test:` section to find the database name.
/// 3. Creates the database via `createdb` if it doesn't already exist.
/// 4. Runs `bundle exec rake db:migrate RAILS_ENV=test`.
pub fn setup_test_database<F>(
    repo_path: &Path,
    ruby_version: Option<&str>,
    mut on_line: F,
) -> Result<()>
where
    F: FnMut(&str),
{
    let config_dir = repo_path.join("config");
    let db_yml = config_dir.join("database.yml");
    let db_example = config_dir.join("database.example.yml");

    // If neither file exists, this isn't a Rails app with DB config — skip.
    if !db_yml.exists() && !db_example.exists() {
        on_line("[db setup] no database.yml or database.example.yml found — skipping");
        return Ok(());
    }

    // Copy example → database.yml if it doesn't exist yet
    if !db_yml.exists() && db_example.exists() {
        on_line("[db setup] copying database.example.yml → database.yml");
        std::fs::copy(&db_example, &db_yml)
            .map_err(|e| anyhow!("Failed to copy database.example.yml: {}", e))?;
    }

    // Parse the test database name from database.yml
    let content = std::fs::read_to_string(&db_yml)
        .map_err(|e| anyhow!("Failed to read database.yml: {}", e))?;

    let db_name = match parse_test_db_name(&content) {
        Some(name) => name,
        None => {
            on_line("[db setup] could not find test database name in database.yml — skipping");
            return Ok(());
        }
    };

    on_line(&format!("[db setup] test database: {}", db_name));

    // Check if the database already exists
    let check = Command::new("psql")
        .args(["-U", "postgres", "-tc",
               &format!("SELECT 1 FROM pg_database WHERE datname = '{}'", db_name)])
        .output();

    let needs_create = match check {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            !stdout.contains('1')
        }
        Err(_) => {
            on_line("[db setup] psql not found or not accessible — skipping db creation");
            return Ok(());
        }
    };

    if needs_create {
        on_line(&format!("[db setup] creating database {}…", db_name));
        let result = Command::new("createdb")
            .args(["-U", "postgres", &db_name])
            .output();
        match result {
            Ok(output) if output.status.success() => {
                on_line(&format!("[db setup] database {} created", db_name));
            }
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                on_line(&format!("[db setup] warning: createdb failed: {}", stderr.trim()));
                // Non-fatal — `rake db:setup` or migrations may handle it
            }
            Err(e) => {
                on_line(&format!("[db setup] warning: could not run createdb: {}", e));
            }
        }
    } else {
        on_line(&format!("[db setup] database {} already exists", db_name));
    }

    // ── Run migrations ────────────────────────────────────────────────────
    on_line("[db setup] running migrations…");
    let mut migrate_cmd = Command::new("bash");
    migrate_cmd.arg("-c");
    migrate_cmd.arg("bundle exec rake db:migrate RAILS_ENV=test 2>&1");
    migrate_cmd.current_dir(repo_path);
    migrate_cmd.stdout(Stdio::piped());
    migrate_cmd.stderr(Stdio::piped());
    if let Some(ver) = ruby_version {
        migrate_cmd.env("RBENV_VERSION", ver);
    }
    if let Ok(home) = std::env::var("HOME") {
        let shims = format!("{home}/.rbenv/shims:{home}/.rbenv/bin");
        let existing_path = std::env::var("PATH").unwrap_or_default();
        migrate_cmd.env("PATH", format!("{}:{}", shims, existing_path));
    }
    match migrate_cmd.spawn() {
        Ok(mut child) => {
            if let Some(stdout) = child.stdout.take() {
                let reader = BufReader::new(stdout);
                for line in reader.lines().flatten() {
                    on_line(&line);
                }
            }
            let status = child.wait();
            match status {
                Ok(s) if s.success() => on_line("[db setup] migrations done"),
                Ok(s) => on_line(&format!("[db setup] warning: migrations exited with code {}", s.code().unwrap_or(-1))),
                Err(e) => on_line(&format!("[db setup] warning: migration error: {}", e)),
            }
        }
        Err(e) => {
            on_line(&format!("[db setup] warning: could not run migrations: {}", e));
        }
    }

    Ok(())
}

/// Parse the database name from the `test:` section of a database.yml.
/// Looks for lines like:
///   test:
///     ...
///     database: g5_cms_test
fn parse_test_db_name(content: &str) -> Option<String> {
    let mut in_test_section = false;
    for line in content.lines() {
        let trimmed = line.trim();
        // A top-level key starts at column 0 (no leading whitespace) and ends with ':'
        if !line.starts_with(' ') && !line.starts_with('\t') && trimmed.ends_with(':') {
            in_test_section = trimmed == "test:";
            continue;
        }
        if in_test_section {
            // Look for "database: <name>" within the test block
            if let Some(rest) = trimmed.strip_prefix("database:") {
                let db = rest.trim();
                if !db.is_empty() {
                    return Some(db.to_string());
                }
            }
        }
    }
    None
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
    //   set -a; source .env.test; set +a; bundle exec rspec
    // We use bash -c so we can chain the env loading.
    let env_setup = if repo_path.join(".env.test").exists() {
        "set -a; source .env.test; set +a; "
    } else {
        ""
    };
    let cmd_str = format!("{}COVERAGE=true bundle exec rspec --format progress 2>&1", env_setup);

    let mut cmd = Command::new("bash");
    cmd.arg("-c");
    cmd.arg(&cmd_str);
    cmd.current_dir(repo_path);
    cmd.stdout(Stdio::piped());

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

    // stderr is already merged into stdout via `2>&1` in the shell command,
    // so we only need to capture stdout. Don't open a separate stderr pipe —
    // doing so would hang because bash has already redirected fd 2 → fd 1.
    cmd.stderr(Stdio::null());

    let mut child = cmd.spawn().map_err(|e| anyhow!("Failed to spawn rspec: {}", e))?;

    let stdout = child.stdout.take().unwrap();

    // Read stdout in a background thread via a channel.
    // This prevents a hang when a forked subprocess (e.g. Spring, parallel workers)
    // inherits the pipe fd and keeps it open after the main rspec process exits.
    let (tx, rx) = mpsc::channel::<String>();
    let reader_thread = thread::spawn(move || {
        let reader = BufReader::new(stdout);
        for line in reader.lines().flatten() {
            if tx.send(line).is_err() {
                break; // receiver dropped, stop reading
            }
        }
    });

    let mut stderr_lines: Vec<String> = Vec::new();

    // Receive lines while process is running. We poll child.try_wait() so
    // we detect process exit independently of pipe EOF — a grandchild holding
    // the pipe open can no longer block us indefinitely.
    let exit_status;
    loop {
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(line) => {
                if line.starts_with("Traceback") || line.contains("Error") {
                    stderr_lines.push(line.clone());
                }
                on_line(&line);
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                // No line available — check if the process has exited.
                match child.try_wait() {
                    Ok(Some(status)) => {
                        // Process exited. Drain any remaining buffered lines
                        // with a short grace period, then return.
                        while let Ok(line) = rx.recv_timeout(Duration::from_millis(500)) {
                            if line.starts_with("Traceback") || line.contains("Error") {
                                stderr_lines.push(line.clone());
                            }
                            on_line(&line);
                        }
                        exit_status = status;
                        break;
                    }
                    Ok(None) => continue, // still running
                    Err(e) => return Err(anyhow!("Error checking process status: {}", e)),
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                // Reader thread finished (pipe EOF). Wait for process to exit.
                exit_status = child.wait()?;
                break;
            }
        }
    }

    // Clean up the reader thread (it will exit once the pipe closes or we drop rx).
    drop(rx);
    let _ = reader_thread.join();

    let exit_code = exit_status.code().unwrap_or(-1);
    let stderr = stderr_lines.join("\n");

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
