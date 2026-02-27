use anyhow::{anyhow, Result};
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use crate::version_manager::{Runtime, RuntimeEnv};

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

    let rt = RuntimeEnv::detect(Runtime::Ruby, ruby_version);
    on_line(&format!("[bundle install] using {:?} version manager", rt.manager));
    let mut cmd = rt.bash_command(repo_path, "bundle install");
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

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

    // Common template names for database.yml across Rails projects.
    let template_candidates = [
        "database.example.yml",
        "database.yml.sample",
        "database.yml.example",
        "database.sample.yml",
    ];

    let db_template = template_candidates
        .iter()
        .map(|name| config_dir.join(name))
        .find(|p| p.exists());

    // If neither database.yml nor any template exists, this isn't a Rails app with DB config — skip.
    if !db_yml.exists() && db_template.is_none() {
        on_line("[db setup] no database.yml or template found — skipping");
        return Ok(());
    }

    // Copy template → database.yml if it doesn't exist yet
    if !db_yml.exists() {
        if let Some(ref template) = db_template {
            let template_name = template.file_name().unwrap().to_string_lossy();
            on_line(&format!("[db setup] copying {} → database.yml", template_name));
            std::fs::copy(template, &db_yml)
                .map_err(|e| anyhow!("Failed to copy {}: {}", template_name, e))?;

            // Patch the username to "postgres" so it works on standard local
            // setups. Template files often ship with non-standard usernames
            // like "vagrant" that don't exist on the developer's machine.
            patch_database_yml_username(&db_yml, &mut on_line);
        }
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
    let rt = RuntimeEnv::detect(Runtime::Ruby, ruby_version);
    let mut migrate_cmd = rt.bash_command(repo_path, "bundle exec rake db:migrate RAILS_ENV=test");
    migrate_cmd.stdout(Stdio::piped());
    migrate_cmd.stderr(Stdio::piped());
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

/// Rewrite `username:` values in a freshly-copied database.yml.
/// Replaces non-standard usernames (e.g. "vagrant") with "postgres" so the
/// app works out-of-the-box on a typical local PostgreSQL install.
fn patch_database_yml_username<F>(db_yml: &Path, on_line: &mut F)
where
    F: FnMut(&str),
{
    const LOCAL_USER: &str = "postgres";
    // Usernames that are known to not exist on a normal macOS/Linux PG setup.
    const REPLACE_USERS: &[&str] = &["vagrant", "deploy", "ubuntu"];

    let content = match std::fs::read_to_string(db_yml) {
        Ok(c) => c,
        Err(_) => return,
    };

    let mut patched = String::with_capacity(content.len());
    let mut changed = false;

    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("username:") {
            let old_user = rest.trim();
            if REPLACE_USERS.iter().any(|u| u.eq_ignore_ascii_case(old_user)) {
                // Preserve original indentation
                let indent = &line[..line.len() - line.trim_start().len()];
                patched.push_str(&format!("{}username: {}\n", indent, LOCAL_USER));
                changed = true;
                continue;
            }
        }
        patched.push_str(line);
        patched.push('\n');
    }

    if changed {
        on_line(&format!("[db setup] patched username in database.yml → {}", LOCAL_USER));
        let _ = std::fs::write(db_yml, patched);
    }
}

/// Run `bundle exec rspec` in the given repo directory.
/// `ruby_version` is the version string from `.ruby-version` / `.tool-versions`.
/// `on_line` is a callback called for each line of stdout/stderr output.
pub fn run_rspec<F>(
    repo_path: &Path,
    ruby_version: Option<&str>,
    mut on_line: F,
) -> Result<RspecResult>
where
    F: FnMut(&str),
{
    let rt = RuntimeEnv::detect(Runtime::Ruby, ruby_version);
    on_line(&format!("[rspec] using {:?} version manager", rt.manager));

    // Build the shell command:
    //   set -a; source .env.test; set +a; bundle exec rspec
    let env_setup = if repo_path.join(".env.test").exists() {
        "set -a; source .env.test; set +a; "
    } else {
        ""
    };
    let shell_cmd = format!("{}COVERAGE=true bundle exec rspec --format progress", env_setup);

    let mut cmd = rt.bash_command(repo_path, &shell_cmd);
    cmd.stdout(Stdio::piped());

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
