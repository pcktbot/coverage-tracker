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

/// Parse `coverage/.resultset.json` from the repo root.
pub fn parse(repo_path: &Path) -> Result<CoverageResult> {
    let result_path = repo_path.join("coverage").join(".resultset.json");
    if !result_path.exists() {
        anyhow::bail!(
            "SimpleCov output not found at {}. Ensure SimpleCov is configured in spec_helper.rb",
            result_path.display()
        );
    }
    let content = std::fs::read_to_string(&result_path)?;
    parse_json(&content)
}

// SimpleCov .resultset.json structure:
// {
//   "RSpec": {
//     "coverage": {
//       "path/to/file.rb": { "lines": [null, 1, 0, null, ...] }
//     },
//     "timestamp": 12345
//   }
// }
#[derive(Deserialize)]
struct ResultSet {
    #[serde(flatten)]
    suites: HashMap<String, Suite>,
}

#[derive(Deserialize)]
struct Suite {
    coverage: HashMap<String, FileCoverageEntry>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum FileCoverageEntry {
    // Newer SimpleCov format: { "lines": [...] }
    Map { lines: Vec<Option<i64>> },
    // Older format: just an array
    Array(Vec<Option<i64>>),
}

impl FileCoverageEntry {
    fn lines(&self) -> &[Option<i64>] {
        match self {
            Self::Map { lines } => lines,
            Self::Array(v) => v,
        }
    }
}

fn parse_json(content: &str) -> Result<CoverageResult> {
    let rs: ResultSet = serde_json::from_str(content)?;

    // Merge all suites (usually just one "RSpec")
    let mut merged: HashMap<String, Vec<Option<i64>>> = HashMap::new();
    for suite in rs.suites.values() {
        for (path, entry) in &suite.coverage {
            let slot = merged.entry(path.clone()).or_default();
            let lines = entry.lines();
            if slot.len() < lines.len() {
                slot.resize(lines.len(), None);
            }
            for (i, &count) in lines.iter().enumerate() {
                if let Some(c) = count {
                    let existing = slot.get_mut(i).unwrap();
                    *existing = Some(existing.unwrap_or(0) + c);
                }
            }
        }
    }

    let mut files: Vec<FileCoverageResult> = Vec::new();
    let mut total_covered = 0i64;
    let mut total_lines = 0i64;

    for (path, lines) in &merged {
        let relevant: Vec<&Option<i64>> = lines.iter().filter(|l| l.is_some()).collect();
        let lt = relevant.len() as i64;
        let lc = relevant.iter().filter(|&&l| l.map_or(false, |n| n > 0)).count() as i64;
        let pct = if lt > 0 { lc as f64 / lt as f64 * 100.0 } else { 0.0 };
        total_covered += lc;
        total_lines += lt;

        // Collect 1-based line numbers where hit count is 0 (uncovered executable lines)
        let uncovered_lines: Vec<usize> = lines.iter().enumerate()
            .filter_map(|(i, count)| match count {
                Some(0) => Some(i + 1), // 1-based line number
                _ => None,
            })
            .collect();

        files.push(FileCoverageResult {
            path: path.clone(),
            coverage_percent: pct,
            lines_covered: lc,
            lines_total: lt,
            uncovered_lines,
        });
    }

    files.sort_by(|a, b| a.path.cmp(&b.path));

    let overall_percent = if total_lines > 0 {
        total_covered as f64 / total_lines as f64 * 100.0
    } else {
        0.0
    };

    Ok(CoverageResult {
        overall_percent,
        lines_covered: total_covered,
        lines_total: total_lines,
        files,
    })
}
