use anyhow::Result;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct CoverageResult {
    pub overall_percent: f64,
    pub lines_covered: i64,
    pub lines_total: i64,
    pub files: Vec<FileCoverageResult>,
}

#[derive(Debug, Clone)]
pub struct FileCoverageResult {
    pub path: String,
    pub coverage_percent: f64,
    pub lines_covered: i64,
    pub lines_total: i64,
    pub uncovered_lines: Vec<usize>,
}

/// Parse Istanbul/NYC coverage output. Looks for coverage data in standard locations:
/// 1. coverage/coverage-summary.json  (json-summary reporter)
/// 2. coverage/coverage-final.json    (json reporter — detailed per-file data)
/// 3. .nyc_output/                    (NYC raw data)
pub fn parse(repo_path: &Path) -> Result<CoverageResult> {
    let coverage_dir = repo_path.join("coverage");

    // Try the detailed coverage-final.json first (has line-level data)
    let final_path = coverage_dir.join("coverage-final.json");
    if final_path.exists() {
        return parse_coverage_final(&final_path);
    }

    // Fall back to coverage-summary.json (aggregate only, no line-level data)
    let summary_path = coverage_dir.join("coverage-summary.json");
    if summary_path.exists() {
        return parse_coverage_summary(&summary_path);
    }

    // Try NYC output directory
    let nyc_dir = repo_path.join(".nyc_output");
    if nyc_dir.exists() {
        // Look for any JSON files in .nyc_output
        for entry in std::fs::read_dir(&nyc_dir)? {
            let entry = entry?;
            if entry.path().extension().map_or(false, |ext| ext == "json") {
                if let Ok(result) = parse_coverage_final(&entry.path()) {
                    return Ok(result);
                }
            }
        }
    }

    anyhow::bail!(
        "No Istanbul/NYC coverage output found in {}. \
         Ensure your test runner is configured to output JSON coverage \
         (e.g., --coverageReporters=json for Jest, or --reporter=json for NYC/c8).",
        coverage_dir.display()
    )
}

// ── coverage-final.json ───────────────────────────────────────────────────────
//
// Structure (Istanbul format):
// {
//   "/abs/path/to/file.js": {
//     "path": "/abs/path/to/file.js",
//     "statementMap": { "0": { "start": {...}, "end": {...} }, ... },
//     "s": { "0": 1, "1": 0, ... },     ← statement hit counts
//     "branchMap": { ... },
//     "b": { ... },
//     "fnMap": { ... },
//     "f": { ... }
//   }
// }

#[derive(Deserialize)]
struct IstanbulFileCoverage {
    path: Option<String>,
    #[serde(rename = "statementMap")]
    statement_map: Option<HashMap<String, StatementLocation>>,
    s: Option<HashMap<String, i64>>,
}

#[derive(Deserialize)]
struct StatementLocation {
    start: Location,
    #[allow(dead_code)]
    end: Location,
}

#[derive(Deserialize)]
struct Location {
    line: usize,
    #[allow(dead_code)]
    column: usize,
}

fn parse_coverage_final(path: &Path) -> Result<CoverageResult> {
    let content = std::fs::read_to_string(path)?;
    let data: HashMap<String, IstanbulFileCoverage> = serde_json::from_str(&content)?;

    let mut files = Vec::new();
    let mut total_covered = 0i64;
    let mut total_statements = 0i64;

    for (file_key, file_cov) in &data {
        let file_path = file_cov
            .path
            .as_deref()
            .unwrap_or(file_key.as_str())
            .to_string();

        let statements = file_cov.s.as_ref();
        let statement_map = file_cov.statement_map.as_ref();

        let (covered, total, uncovered_lines) = match (statements, statement_map) {
            (Some(s), Some(sm)) => {
                let total = s.len() as i64;
                let covered = s.values().filter(|&&count| count > 0).count() as i64;

                // Find uncovered lines from the statement map
                let mut uncovered: Vec<usize> = Vec::new();
                for (key, &count) in s {
                    if count == 0 {
                        if let Some(loc) = sm.get(key) {
                            uncovered.push(loc.start.line);
                        }
                    }
                }
                uncovered.sort();
                uncovered.dedup();

                (covered, total, uncovered)
            }
            _ => (0, 0, vec![]),
        };

        let pct = if total > 0 {
            covered as f64 / total as f64 * 100.0
        } else {
            0.0
        };

        total_covered += covered;
        total_statements += total;

        files.push(FileCoverageResult {
            path: file_path,
            coverage_percent: pct,
            lines_covered: covered,
            lines_total: total,
            uncovered_lines,
        });
    }

    files.sort_by(|a, b| a.path.cmp(&b.path));

    let overall_percent = if total_statements > 0 {
        total_covered as f64 / total_statements as f64 * 100.0
    } else {
        0.0
    };

    Ok(CoverageResult {
        overall_percent,
        lines_covered: total_covered,
        lines_total: total_statements,
        files,
    })
}

// ── coverage-summary.json ─────────────────────────────────────────────────────
//
// Structure:
// {
//   "total": {
//     "lines": { "total": 1000, "covered": 800, "skipped": 0, "pct": 80 },
//     "statements": { ... },
//     "functions": { ... },
//     "branches": { ... }
//   },
//   "/abs/path/to/file.js": {
//     "lines": { "total": 50, "covered": 40, "skipped": 0, "pct": 80 },
//     ...
//   }
// }

#[derive(Deserialize)]
struct SummaryMetric {
    total: i64,
    covered: i64,
    #[allow(dead_code)]
    pct: f64,
}

#[derive(Deserialize)]
struct FileSummary {
    lines: Option<SummaryMetric>,
    statements: Option<SummaryMetric>,
}

fn parse_coverage_summary(path: &Path) -> Result<CoverageResult> {
    let content = std::fs::read_to_string(path)?;
    let data: HashMap<String, FileSummary> = serde_json::from_str(&content)?;

    let mut files = Vec::new();
    let mut total_covered = 0i64;
    let mut total_lines = 0i64;

    for (file_path, summary) in &data {
        if file_path == "total" {
            continue;
        }

        // Prefer lines metric, fall back to statements
        let metric = summary
            .lines
            .as_ref()
            .or(summary.statements.as_ref());

        if let Some(m) = metric {
            let pct = if m.total > 0 {
                m.covered as f64 / m.total as f64 * 100.0
            } else {
                0.0
            };

            total_covered += m.covered;
            total_lines += m.total;

            files.push(FileCoverageResult {
                path: file_path.clone(),
                coverage_percent: pct,
                lines_covered: m.covered,
                lines_total: m.total,
                uncovered_lines: vec![], // summary format doesn't have line-level data
            });
        }
    }

    files.sort_by(|a, b| a.path.cmp(&b.path));

    let overall_percent = if total_lines > 0 {
        total_covered as f64 / total_lines as f64 * 100.0
    } else {
        0.0
    };

    // If we have a "total" entry, prefer its values for the overall stats
    if let Some(total_summary) = data.get("total") {
        if let Some(m) = total_summary.lines.as_ref().or(total_summary.statements.as_ref()) {
            return Ok(CoverageResult {
                overall_percent: if m.total > 0 {
                    m.covered as f64 / m.total as f64 * 100.0
                } else {
                    0.0
                },
                lines_covered: m.covered,
                lines_total: m.total,
                files,
            });
        }
    }

    Ok(CoverageResult {
        overall_percent,
        lines_covered: total_covered,
        lines_total: total_lines,
        files,
    })
}
