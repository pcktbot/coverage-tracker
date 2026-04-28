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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RepoSources {
    pub repo_id: i64,
    pub platform_name: Option<String>,
    pub tfs_project: Option<String>,
    pub tfs_area_path: Option<String>,
    pub tfs_team: Option<String>,
    pub tfs_release_definition: Option<String>,
    pub confluence_space_key: Option<String>,
    pub confluence_parent_page_id: Option<String>,
    pub confluence_site_label: Option<String>,
    pub notes: Option<String>,
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

pub fn get_repo_sources(conn: &Connection, repo_id: i64) -> Result<RepoSources> {
    let result = conn.query_row(
        "SELECT repo_id, platform_name, tfs_project, tfs_area_path, tfs_team,
                tfs_release_definition, confluence_space_key, confluence_parent_page_id,
                confluence_site_label, notes
         FROM repo_sources
         WHERE repo_id = ?1",
        params![repo_id],
        |row| {
            Ok(RepoSources {
                repo_id: row.get(0)?,
                platform_name: row.get(1)?,
                tfs_project: row.get(2)?,
                tfs_area_path: row.get(3)?,
                tfs_team: row.get(4)?,
                tfs_release_definition: row.get(5)?,
                confluence_space_key: row.get(6)?,
                confluence_parent_page_id: row.get(7)?,
                confluence_site_label: row.get(8)?,
                notes: row.get(9)?,
            })
        },
    );

    match result {
        Ok(sources) => Ok(sources),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(RepoSources { repo_id, ..Default::default() }),
        Err(err) => Err(err.into()),
    }
}

pub fn upsert_repo_sources(conn: &Connection, sources: &RepoSources) -> Result<()> {
    conn.execute(
        "INSERT INTO repo_sources (
            repo_id, platform_name, tfs_project, tfs_area_path, tfs_team,
            tfs_release_definition, confluence_space_key, confluence_parent_page_id,
            confluence_site_label, notes
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
         ON CONFLICT(repo_id) DO UPDATE SET
            platform_name = excluded.platform_name,
            tfs_project = excluded.tfs_project,
            tfs_area_path = excluded.tfs_area_path,
            tfs_team = excluded.tfs_team,
            tfs_release_definition = excluded.tfs_release_definition,
            confluence_space_key = excluded.confluence_space_key,
            confluence_parent_page_id = excluded.confluence_parent_page_id,
            confluence_site_label = excluded.confluence_site_label,
            notes = excluded.notes",
        params![
            sources.repo_id,
            sources.platform_name,
            sources.tfs_project,
            sources.tfs_area_path,
            sources.tfs_team,
            sources.tfs_release_definition,
            sources.confluence_space_key,
            sources.confluence_parent_page_id,
            sources.confluence_site_label,
            sources.notes,
        ],
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
