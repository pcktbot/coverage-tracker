//! Runtime end-of-life tracking.
//!
//! Fetches LTS / EOL schedules from <https://endoflife.date> and caches
//! them in the local SQLite database.  The API is free, requires no key,
//! and is community-maintained for 300+ products including Node.js and Ruby.
//!
//! Refresh policy: at most once per day per runtime.

use anyhow::{anyhow, Result};
use chrono::NaiveDate;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

// ── Public types ──────────────────────────────────────────────────────────────

/// A single release cycle (e.g. Node 20, Ruby 3.2).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EolCycle {
    pub runtime: String,
    pub cycle: String,
    pub release_date: Option<String>,
    pub eol_date: Option<String>,
    pub lts_date: Option<String>,
    pub latest: Option<String>,
    pub is_eol: bool,
}

/// Summary returned to the frontend for a specific version string.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EolStatus {
    /// The major cycle that was matched (e.g. "20" for Node 20.11.0).
    pub cycle: Option<String>,
    /// Whether this version's cycle has reached end-of-life.
    pub is_eol: bool,
    /// The EOL date, if known.
    pub eol_date: Option<String>,
    /// Whether this cycle ever had LTS status.
    pub has_lts: bool,
    /// When LTS started, if applicable.
    pub lts_date: Option<String>,
}

// ── API response shape ────────────────────────────────────────────────────────

/// Shape returned by `https://endoflife.date/api/{product}.json`.
/// Fields can be either a date string or a boolean (`false`).
#[derive(Debug, Deserialize)]
struct ApiCycle {
    cycle: String,
    #[serde(default, deserialize_with = "de_date_or_bool")]
    eol: DateOrBool,
    #[serde(default, deserialize_with = "de_date_or_bool")]
    lts: DateOrBool,
    #[serde(rename = "releaseDate")]
    release_date: Option<String>,
    latest: Option<String>,
}

#[derive(Debug, Clone, Default)]
enum DateOrBool {
    Date(String),
    Bool(bool),
    #[default]
    Unknown,
}

fn de_date_or_bool<'de, D: serde::Deserializer<'de>>(d: D) -> Result<DateOrBool, D::Error> {
    let v: serde_json::Value = serde::Deserialize::deserialize(d)?;
    match v {
        serde_json::Value::String(s) => Ok(DateOrBool::Date(s)),
        serde_json::Value::Bool(b) => Ok(DateOrBool::Bool(b)),
        _ => Ok(DateOrBool::Unknown),
    }
}

// ── Core logic ────────────────────────────────────────────────────────────────

const API_BASE: &str = "https://endoflife.date/api";

/// Supported runtimes (must match the `runtime` column values in DB).
pub fn supported_runtimes() -> &'static [&'static str] {
    &["nodejs", "ruby"]
}

/// Refresh cached EOL data for a runtime if the cache is stale (>24 h).
/// Returns `Ok(true)` if new data was fetched, `Ok(false)` if cache is fresh.
pub fn refresh_if_stale(conn: &Connection, runtime: &str) -> Result<bool> {
    if !supported_runtimes().contains(&runtime) {
        return Err(anyhow!("Unsupported runtime: {runtime}"));
    }

    if !is_stale(conn, runtime)? {
        return Ok(false);
    }

    let cycles = fetch_from_api(runtime)?;
    upsert_cycles(conn, runtime, &cycles)?;
    update_last_fetched(conn, runtime)?;
    Ok(true)
}

/// Refresh all supported runtimes.
pub fn refresh_all_if_stale(conn: &Connection) -> Result<()> {
    for rt in supported_runtimes() {
        if let Err(e) = refresh_if_stale(conn, rt) {
            eprintln!("Warning: failed to refresh EOL data for {rt}: {e}");
        }
    }
    Ok(())
}

/// Check EOL status for a concrete version string (e.g. "20.11.0" for Node,
/// "3.2.2" for Ruby).  Uses cached data — call `refresh_if_stale` first.
pub fn check_version(conn: &Connection, runtime: &str, version: &str) -> Result<EolStatus> {
    let cycle_key = extract_cycle(runtime, version);

    let row = conn.query_row(
        "SELECT cycle, release_date, eol_date, lts_date, latest, is_eol
         FROM runtime_eol
         WHERE runtime = ?1 AND cycle = ?2",
        params![runtime, cycle_key],
        |row| {
            Ok(EolCycle {
                runtime: runtime.to_string(),
                cycle: row.get(0)?,
                release_date: row.get(1)?,
                eol_date: row.get(2)?,
                lts_date: row.get(3)?,
                latest: row.get(4)?,
                is_eol: row.get::<_, i64>(5)? != 0,
            })
        },
    );

    match row {
        Ok(c) => Ok(EolStatus {
            cycle: Some(c.cycle),
            is_eol: c.is_eol,
            eol_date: c.eol_date,
            has_lts: c.lts_date.is_some(),
            lts_date: c.lts_date,
        }),
        Err(rusqlite::Error::QueryReturnedNoRows) => {
            // Version not found in our cache — could be very new or very old
            Ok(EolStatus {
                cycle: Some(cycle_key),
                is_eol: false,
                eol_date: None,
                has_lts: false,
                lts_date: None,
            })
        }
        Err(e) => Err(e.into()),
    }
}

/// List all cached cycles for a runtime, ordered newest first.
pub fn list_cycles(conn: &Connection, runtime: &str) -> Result<Vec<EolCycle>> {
    let mut stmt = conn.prepare(
        "SELECT runtime, cycle, release_date, eol_date, lts_date, latest, is_eol
         FROM runtime_eol
         WHERE runtime = ?1
         ORDER BY release_date DESC"
    )?;
    let rows = stmt.query_map(params![runtime], |row| {
        Ok(EolCycle {
            runtime: row.get(0)?,
            cycle: row.get(1)?,
            release_date: row.get(2)?,
            eol_date: row.get(3)?,
            lts_date: row.get(4)?,
            latest: row.get(5)?,
            is_eol: row.get::<_, i64>(6)? != 0,
        })
    })?.collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

// ── Private helpers ───────────────────────────────────────────────────────────

/// Extract the cycle identifier from a full version string.
/// Node.js uses major version as cycle (e.g. "20" from "20.11.0").
/// Ruby uses major.minor (e.g. "3.2" from "3.2.2").
fn extract_cycle(runtime: &str, version: &str) -> String {
    let v = version.strip_prefix('v').unwrap_or(version);
    match runtime {
        "nodejs" => {
            // Node cycles are just the major number: "18", "20", "22"
            v.split('.').next().unwrap_or(v).to_string()
        }
        "ruby" => {
            // Ruby cycles are major.minor: "3.2", "3.3"
            let parts: Vec<&str> = v.split('.').collect();
            if parts.len() >= 2 {
                format!("{}.{}", parts[0], parts[1])
            } else {
                v.to_string()
            }
        }
        _ => v.to_string(),
    }
}

fn is_stale(conn: &Connection, runtime: &str) -> Result<bool> {
    let last: Result<String, _> = conn.query_row(
        "SELECT last_fetched FROM runtime_eol_meta WHERE runtime = ?1",
        params![runtime],
        |row| row.get(0),
    );

    match last {
        Ok(ts) => {
            let fetched = chrono::NaiveDateTime::parse_from_str(&ts, "%Y-%m-%dT%H:%M:%S")
                .or_else(|_| chrono::NaiveDateTime::parse_from_str(&ts, "%Y-%m-%d %H:%M:%S"))
                .unwrap_or_else(|_| chrono::NaiveDateTime::default());
            let age = chrono::Utc::now().naive_utc() - fetched;
            Ok(age.num_hours() >= 24)
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(true),
        Err(e) => Err(e.into()),
    }
}

fn fetch_from_api(runtime: &str) -> Result<Vec<ApiCycle>> {
    let url = format!("{API_BASE}/{runtime}.json");
    let resp = reqwest::blocking::Client::new()
        .get(&url)
        .header("Accept", "application/json")
        .send()
        .map_err(|e| anyhow!("Failed to fetch {url}: {e}"))?;

    if !resp.status().is_success() {
        return Err(anyhow!("endoflife.date returned HTTP {}", resp.status()));
    }

    let cycles: Vec<ApiCycle> = resp.json()
        .map_err(|e| anyhow!("Failed to parse endoflife.date response: {e}"))?;
    Ok(cycles)
}

fn upsert_cycles(conn: &Connection, runtime: &str, cycles: &[ApiCycle]) -> Result<()> {
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();

    for c in cycles {
        let eol_date = match &c.eol {
            DateOrBool::Date(d) => Some(d.clone()),
            _ => None,
        };
        let lts_date = match &c.lts {
            DateOrBool::Date(d) => Some(d.clone()),
            _ => None,
        };
        let is_eol = match &c.eol {
            DateOrBool::Bool(true) => true,
            DateOrBool::Date(d) => {
                // If the EOL date is in the past, it's EOL
                NaiveDate::parse_from_str(d, "%Y-%m-%d")
                    .map(|eol| {
                        let today_d = NaiveDate::parse_from_str(&today, "%Y-%m-%d")
                            .unwrap_or_default();
                        eol <= today_d
                    })
                    .unwrap_or(false)
            }
            _ => false,
        };

        conn.execute(
            "INSERT INTO runtime_eol (runtime, cycle, release_date, eol_date, lts_date, latest, is_eol)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(runtime, cycle) DO UPDATE SET
                release_date = excluded.release_date,
                eol_date     = excluded.eol_date,
                lts_date     = excluded.lts_date,
                latest       = excluded.latest,
                is_eol       = excluded.is_eol",
            params![
                runtime,
                c.cycle,
                c.release_date,
                eol_date,
                lts_date,
                c.latest,
                is_eol as i64,
            ],
        )?;
    }
    Ok(())
}

fn update_last_fetched(conn: &Connection, runtime: &str) -> Result<()> {
    let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string();
    conn.execute(
        "INSERT INTO runtime_eol_meta (runtime, last_fetched)
         VALUES (?1, ?2)
         ON CONFLICT(runtime) DO UPDATE SET last_fetched = excluded.last_fetched",
        params![runtime, now],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_node_cycle() {
        assert_eq!(extract_cycle("nodejs", "20.11.0"), "20");
        assert_eq!(extract_cycle("nodejs", "v18.17.0"), "18");
        assert_eq!(extract_cycle("nodejs", "22"), "22");
    }

    #[test]
    fn extract_ruby_cycle() {
        assert_eq!(extract_cycle("ruby", "3.2.2"), "3.2");
        assert_eq!(extract_cycle("ruby", "3.3.0"), "3.3");
        assert_eq!(extract_cycle("ruby", "2.7.8"), "2.7");
    }
}
