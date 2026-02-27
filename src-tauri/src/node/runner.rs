use anyhow::{anyhow, Result};
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use crate::version_manager::{Runtime, RuntimeEnv};

#[derive(Debug)]
pub struct NodeTestResult {
    pub exit_code: i32,
    pub stderr: String,
}

/// Detected package manager for a Node project.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PackageManager {
    Npm,
    Yarn,
    Pnpm,
}

impl PackageManager {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Npm => "npm",
            Self::Yarn => "yarn",
            Self::Pnpm => "pnpm",
        }
    }

    pub fn install_cmd(&self) -> &'static str {
        match self {
            Self::Npm => "npm install",
            Self::Yarn => "yarn install",
            Self::Pnpm => "pnpm install",
        }
    }

    #[allow(dead_code)]
    pub fn test_cmd(&self) -> &'static str {
        match self {
            Self::Npm => "npm test",
            Self::Yarn => "yarn test",
            Self::Pnpm => "pnpm test",
        }
    }
}

/// Detect which package manager the project uses based on lockfile presence.
pub fn detect_package_manager(repo_path: &Path) -> PackageManager {
    if repo_path.join("pnpm-lock.yaml").exists() {
        PackageManager::Pnpm
    } else if repo_path.join("yarn.lock").exists() {
        PackageManager::Yarn
    } else {
        PackageManager::Npm
    }
}

/// Check whether `npm/yarn/pnpm install` needs to run by comparing the lockfile
/// mtime against a `.node_modules_installed` marker file we drop after a successful install.
fn needs_install(repo_path: &Path) -> bool {
    let marker = repo_path.join(".node_modules_installed");

    // If node_modules doesn't exist at all, definitely install
    if !repo_path.join("node_modules").exists() {
        return true;
    }

    let lockfiles = [
        "package-lock.json",
        "yarn.lock",
        "pnpm-lock.yaml",
    ];

    let newest_lock = lockfiles
        .iter()
        .map(|name| repo_path.join(name))
        .filter(|p| p.exists())
        .filter_map(|p| p.metadata().ok()?.modified().ok())
        .max();

    match (newest_lock, marker.metadata().and_then(|m| m.modified())) {
        (Some(lock_t), Ok(mark_t)) => lock_t > mark_t,
        (None, Ok(_)) => false, // no lockfile but marker exists → skip
        _ => true,              // marker missing or unreadable
    }
}

/// Build a `Command` for a Node shell command using the auto-detected
/// version manager (mise, asdf, nodenv, nvm, or system PATH).
fn build_node_cmd(
    repo_path: &Path,
    node_version: Option<&str>,
    shell_cmd: &str,
) -> Command {
    let rt = RuntimeEnv::detect(Runtime::Node, node_version);
    let mut cmd = rt.bash_command(repo_path, shell_cmd);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::null()); // stderr merged via 2>&1
    cmd
}

/// Run `npm install` / `yarn install` / `pnpm install` in the given repo directory,
/// streaming output via `on_line`.
/// Skips if lockfile hasn't changed since the last successful install.
/// Returns Ok(true) if it ran, Ok(false) if skipped.
pub fn run_npm_install<F>(
    repo_path: &Path,
    node_version: Option<&str>,
    mut on_line: F,
) -> Result<bool>
where
    F: FnMut(&str),
{
    if !repo_path.join("package.json").exists() {
        return Err(anyhow!("No package.json found — not a Node project"));
    }

    if !needs_install(repo_path) {
        on_line("[npm install] skipped — lockfile unchanged");
        return Ok(false);
    }

    let pm = detect_package_manager(repo_path);
    on_line(&format!("[{} install] running…", pm.as_str()));

    let mut child = build_node_cmd(repo_path, node_version, pm.install_cmd())
        .spawn()
        .map_err(|e| anyhow!("Failed to spawn {} install: {}", pm.as_str(), e))?;

    let stdout = child.stdout.take().unwrap();
    let reader = BufReader::new(stdout);
    for line in reader.lines().flatten() {
        on_line(&line);
    }

    let status = child.wait()?;
    if !status.success() {
        return Err(anyhow!(
            "{} install exited with code {}",
            pm.as_str(),
            status.code().unwrap_or(-1)
        ));
    }

    // Drop marker file so we skip next time if lockfile hasn't changed
    let _ = std::fs::write(repo_path.join(".node_modules_installed"), "");
    on_line(&format!("[{} install] done", pm.as_str()));
    Ok(true)
}

/// Run the project's test suite with coverage enabled.
/// Attempts to detect the test runner (Jest, Vitest, NYC/Mocha, c8) and
/// configures it for JSON coverage output that we can parse.
///
/// `on_line` is called for each line of stdout/stderr output.
pub fn run_node_tests<F>(
    repo_path: &Path,
    node_version: Option<&str>,
    mut on_line: F,
) -> Result<NodeTestResult>
where
    F: FnMut(&str),
{
    let test_cmd = detect_test_command(repo_path);
    on_line(&format!("[test] running: {}", test_cmd));

    let mut child = build_node_cmd(repo_path, node_version, &test_cmd)
        .spawn()
        .map_err(|e| anyhow!("Failed to spawn test runner: {}", e))?;

    let stdout = child.stdout.take().unwrap();

    // Read stdout in a background thread via a channel, same pattern as rspec runner.
    let (tx, rx) = mpsc::channel::<String>();
    let reader_thread = thread::spawn(move || {
        let reader = BufReader::new(stdout);
        for line in reader.lines().flatten() {
            if tx.send(line).is_err() {
                break;
            }
        }
    });

    let mut stderr_lines: Vec<String> = Vec::new();

    let exit_status;
    loop {
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(line) => {
                if line.contains("ERR!") || line.contains("Error") || line.contains("FAIL") {
                    stderr_lines.push(line.clone());
                }
                on_line(&line);
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                match child.try_wait() {
                    Ok(Some(status)) => {
                        while let Ok(line) = rx.recv_timeout(Duration::from_millis(500)) {
                            if line.contains("ERR!") || line.contains("Error") || line.contains("FAIL") {
                                stderr_lines.push(line.clone());
                            }
                            on_line(&line);
                        }
                        exit_status = status;
                        break;
                    }
                    Ok(None) => continue,
                    Err(e) => return Err(anyhow!("Error checking process status: {}", e)),
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                exit_status = child.wait()?;
                break;
            }
        }
    }

    drop(rx);
    let _ = reader_thread.join();

    let exit_code = exit_status.code().unwrap_or(-1);
    let stderr = stderr_lines.join("\n");

    Ok(NodeTestResult { exit_code, stderr })
}

/// Detect the best test command to run with coverage enabled.
/// Inspects package.json scripts & devDependencies to determine the runner.
fn detect_test_command(repo_path: &Path) -> String {
    let pkg_json = repo_path.join("package.json");
    let content = match std::fs::read_to_string(&pkg_json) {
        Ok(c) => c,
        Err(_) => return "npm test".to_string(),
    };

    let parsed: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(_) => return "npm test".to_string(),
    };

    let dev_deps = parsed.get("devDependencies").and_then(|d| d.as_object());
    let deps = parsed.get("dependencies").and_then(|d| d.as_object());
    let scripts = parsed.get("scripts").and_then(|s| s.as_object());

    let has_dep = |name: &str| -> bool {
        dev_deps.map_or(false, |d| d.contains_key(name))
            || deps.map_or(false, |d| d.contains_key(name))
    };

    let test_script = scripts
        .and_then(|s| s.get("test"))
        .and_then(|v| v.as_str())
        .unwrap_or("");

    // Check if the test script already includes --coverage
    let test_already_has_coverage = test_script.contains("--coverage")
        || test_script.contains("c8 ")
        || test_script.contains("nyc ");

    // Jest (most common in enterprise Node apps)
    if has_dep("jest") || test_script.contains("jest") {
        if test_already_has_coverage {
            return "npx jest --coverage --coverageReporters=json-summary --coverageReporters=json 2>&1".to_string();
        }
        return "npx jest --coverage --coverageReporters=json-summary --coverageReporters=json 2>&1".to_string();
    }

    // Vitest
    if has_dep("vitest") || test_script.contains("vitest") {
        return "npx vitest run --coverage --reporter=json 2>&1".to_string();
    }

    // c8 (native V8 coverage, often used with Mocha)
    if has_dep("c8") || test_script.contains("c8 ") {
        // c8 outputs to coverage/ by default
        if scripts.map_or(false, |s| s.contains_key("test")) {
            return "npx c8 --reporter=json-summary --reporter=json npm test 2>&1".to_string();
        }
    }

    // NYC / Istanbul (classic)
    if has_dep("nyc") || test_script.contains("nyc ") {
        if scripts.map_or(false, |s| s.contains_key("test")) {
            return "npx nyc --reporter=json-summary --reporter=json npm test 2>&1".to_string();
        }
    }

    // Mocha without explicit coverage tool — wrap with c8
    if has_dep("mocha") || test_script.contains("mocha") {
        return "npx c8 --reporter=json-summary --reporter=json npx mocha 2>&1".to_string();
    }

    // Fallback: just run the test script. If jest is the runner behind
    // `npm test`, coverage flags may not be passed — but it's the safest default.
    let pm = detect_package_manager(repo_path);
    format!("{} test 2>&1", pm.as_str())
}

/// Read .node-version, .nvmrc, or .tool-versions from repo root, if present.
#[allow(dead_code)]
pub fn read_node_version(repo_path: &Path) -> Option<String> {
    crate::version_manager::read_node_version(repo_path)
}
