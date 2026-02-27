use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use anyhow::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Repo {
    pub id: i64,
    pub org: String,
    pub name: String,
    pub github_url: String,
    pub local_path: Option<String>,
    pub ruby_version: Option<String>,
    pub node_version: Option<String>,
    pub enabled: bool,
    pub last_synced_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Org {
    pub id: i64,
    pub name: String,
    pub is_active: bool,
}

pub fn list_repos(conn: &Connection, org: Option<&str>) -> Result<Vec<Repo>> {
    let rows: Vec<Repo> = if let Some(org) = org {
        let mut stmt = conn.prepare(
            "SELECT id, org, name, github_url, local_path, ruby_version, node_version, enabled, last_synced_at
             FROM repos WHERE org = ?1 ORDER BY name"
        )?;
        let r = stmt.query_map(params![org], map_repo)?.collect::<Result<Vec<_>, _>>()?;
        r
    } else {
        let mut stmt = conn.prepare(
            "SELECT id, org, name, github_url, local_path, ruby_version, node_version, enabled, last_synced_at
             FROM repos ORDER BY org, name"
        )?;
        let r = stmt.query_map([], map_repo)?.collect::<Result<Vec<_>, _>>()?;
        r
    };
    Ok(rows)
}

fn map_repo(row: &rusqlite::Row<'_>) -> rusqlite::Result<Repo> {
    Ok(Repo {
        id: row.get(0)?,
        org: row.get(1)?,
        name: row.get(2)?,
        github_url: row.get(3)?,
        local_path: row.get(4)?,
        ruby_version: row.get(5)?,
        node_version: row.get(6)?,
        enabled: row.get::<_, i64>(7)? != 0,
        last_synced_at: row.get(8)?,
    })
}

pub fn upsert_repo(conn: &Connection, org: &str, name: &str, github_url: &str) -> Result<i64> {
    conn.execute(
        "INSERT INTO repos (org, name, github_url)
         VALUES (?1, ?2, ?3)
         ON CONFLICT(org, name) DO UPDATE SET github_url = excluded.github_url",
        params![org, name, github_url],
    )?;
    let id = conn.query_row(
        "SELECT id FROM repos WHERE org = ?1 AND name = ?2",
        params![org, name],
        |r| r.get(0),
    )?;
    Ok(id)
}

pub fn update_repo_local_path(
    conn: &Connection,
    id: i64,
    path: &str,
    ruby_version: Option<&str>,
    node_version: Option<&str>,
) -> Result<()> {
    conn.execute(
        "UPDATE repos SET local_path = ?1, ruby_version = ?2, node_version = ?3, last_synced_at = datetime('now')
         WHERE id = ?4",
        params![path, ruby_version, node_version, id],
    )?;
    Ok(())
}

pub fn set_repo_enabled(conn: &Connection, id: i64, enabled: bool) -> Result<()> {
    conn.execute(
        "UPDATE repos SET enabled = ?1 WHERE id = ?2",
        params![enabled as i64, id],
    )?;
    Ok(())
}

pub fn list_orgs(conn: &Connection) -> Result<Vec<Org>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, is_active FROM orgs ORDER BY name"
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(Org {
            id: row.get(0)?,
            name: row.get(1)?,
            is_active: row.get::<_, i64>(2)? != 0,
        })
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

pub fn add_org(conn: &Connection, name: &str) -> Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO orgs (name) VALUES (?1)",
        params![name],
    )?;
    Ok(())
}

pub fn remove_org(conn: &Connection, name: &str) -> Result<()> {
    conn.execute("DELETE FROM orgs WHERE name = ?1", params![name])?;
    Ok(())
}

pub fn set_active_org(conn: &Connection, name: &str) -> Result<()> {
    conn.execute("UPDATE orgs SET is_active = 0", [])?;
    conn.execute("UPDATE orgs SET is_active = 1 WHERE name = ?1", params![name])?;
    Ok(())
}

pub fn get_active_org(conn: &Connection) -> Result<Option<String>> {
    let result = conn.query_row(
        "SELECT name FROM orgs WHERE is_active = 1 LIMIT 1",
        [],
        |r| r.get(0),
    );
    match result {
        Ok(name) => Ok(Some(name)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

pub fn get_setting(conn: &Connection, key: &str) -> Result<Option<String>> {
    let result = conn.query_row(
        "SELECT value FROM settings WHERE key = ?1",
        params![key],
        |r| r.get(0),
    );
    match result {
        Ok(v) => Ok(Some(v)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

pub fn set_setting(conn: &Connection, key: &str, value: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}
