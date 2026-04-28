use anyhow::Result;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: i64,
    pub name: String,
    pub slug: String,
    pub status: String,
    pub platform_name: Option<String>,
    pub manual_priority: i64,
    pub notes: Option<String>,
    pub is_active: bool,
    pub linked_repo_ids: Vec<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectSummary {
    pub id: i64,
    pub name: String,
    pub slug: String,
    pub status: String,
    pub platform_name: Option<String>,
    pub manual_priority: i64,
    pub is_active: bool,
    pub repo_count: i64,
    pub source_link_count: i64,
    pub focus_score: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentProfile {
    pub id: i64,
    pub name: String,
    pub goal: String,
    pub instructions: String,
    pub source_types: Vec<String>,
    pub project_scope: String,
    pub weight_manual_priority: i64,
    pub weight_release_risk: i64,
    pub weight_doc_gap: i64,
    pub weight_meeting_followup: i64,
    pub is_active: bool,
}

pub fn list_projects(conn: &Connection) -> Result<Vec<ProjectSummary>> {
    let mut stmt = conn.prepare(
        "SELECT
            p.id,
            p.name,
            p.slug,
            p.status,
            p.platform_name,
            p.manual_priority,
            p.is_active,
            COUNT(DISTINCT pr.repo_id) AS repo_count,
            (
              SELECT COUNT(*)
              FROM repo_sources rs
              JOIN project_repos pr2 ON pr2.repo_id = rs.repo_id
              WHERE pr2.project_id = p.id
            ) AS source_link_count
         FROM projects p
         LEFT JOIN project_repos pr ON pr.project_id = p.id
         GROUP BY p.id
         ORDER BY p.is_active DESC, p.manual_priority DESC, p.name"
    )?;

    let rows = stmt.query_map([], |row| {
        let manual_priority = row.get::<_, i64>(5)?;
        let repo_count = row.get::<_, i64>(7)?;
        let source_link_count = row.get::<_, i64>(8)?;
        Ok(ProjectSummary {
            id: row.get(0)?,
            name: row.get(1)?,
            slug: row.get(2)?,
            status: row.get(3)?,
            platform_name: row.get(4)?,
            manual_priority,
            is_active: row.get::<_, i64>(6)? != 0,
            repo_count,
            source_link_count,
            focus_score: manual_priority * 10 + repo_count * 2 + source_link_count * 3,
        })
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

pub fn get_project(conn: &Connection, project_id: i64) -> Result<Project> {
    let project = conn.query_row(
        "SELECT id, name, slug, status, platform_name, manual_priority, notes, is_active
         FROM projects
         WHERE id = ?1",
        params![project_id],
        |row| {
            Ok(Project {
                id: row.get(0)?,
                name: row.get(1)?,
                slug: row.get(2)?,
                status: row.get(3)?,
                platform_name: row.get(4)?,
                manual_priority: row.get(5)?,
                notes: row.get(6)?,
                is_active: row.get::<_, i64>(7)? != 0,
                linked_repo_ids: Vec::new(),
            })
        },
    )?;

    let mut project = project;
    let mut stmt = conn.prepare(
        "SELECT repo_id FROM project_repos WHERE project_id = ?1 ORDER BY repo_id"
    )?;
    let repo_ids = stmt
        .query_map(params![project_id], |row| row.get::<_, i64>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    project.linked_repo_ids = repo_ids;
    Ok(project)
}

pub fn create_project(conn: &Connection, name: &str) -> Result<i64> {
    let slug = slugify(name);
    conn.execute(
        "INSERT INTO projects (name, slug, status, is_active, manual_priority)
         VALUES (?1, ?2, 'incoming', 1, 3)",
        params![name, slug],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn save_project(conn: &Connection, project: &Project) -> Result<()> {
    conn.execute(
        "UPDATE projects
         SET name = ?1, slug = ?2, status = ?3, platform_name = ?4,
             manual_priority = ?5, notes = ?6, is_active = ?7, updated_at = datetime('now')
         WHERE id = ?8",
        params![
            project.name,
            slugify(&project.name),
            project.status,
            project.platform_name,
            project.manual_priority,
            project.notes,
            project.is_active as i64,
            project.id,
        ],
    )?;

    conn.execute("DELETE FROM project_repos WHERE project_id = ?1", params![project.id])?;
    for repo_id in &project.linked_repo_ids {
        conn.execute(
            "INSERT INTO project_repos (project_id, repo_id) VALUES (?1, ?2)",
            params![project.id, repo_id],
        )?;
    }
    Ok(())
}

pub fn list_agent_profiles(conn: &Connection) -> Result<Vec<AgentProfile>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, goal, instructions, source_types, project_scope,
                weight_manual_priority, weight_release_risk, weight_doc_gap,
                weight_meeting_followup, is_active
         FROM agent_profiles
         ORDER BY is_active DESC, name"
    )?;
    let rows = stmt.query_map([], |row| {
        let source_types_raw: String = row.get(4)?;
        Ok(AgentProfile {
            id: row.get(0)?,
            name: row.get(1)?,
            goal: row.get(2)?,
            instructions: row.get(3)?,
            source_types: serde_json::from_str(&source_types_raw).unwrap_or_default(),
            project_scope: row.get(5)?,
            weight_manual_priority: row.get(6)?,
            weight_release_risk: row.get(7)?,
            weight_doc_gap: row.get(8)?,
            weight_meeting_followup: row.get(9)?,
            is_active: row.get::<_, i64>(10)? != 0,
        })
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

pub fn create_agent_profile(conn: &Connection, name: &str) -> Result<i64> {
    conn.execute(
        "INSERT INTO agent_profiles (
            name, goal, instructions, source_types, project_scope,
            weight_manual_priority, weight_release_risk, weight_doc_gap,
            weight_meeting_followup, is_active
         ) VALUES (?1, '', '', '[]', 'all_active_projects', 5, 4, 2, 3, 1)",
        params![name],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn save_agent_profile(conn: &Connection, profile: &AgentProfile) -> Result<()> {
    conn.execute(
        "UPDATE agent_profiles
         SET name = ?1, goal = ?2, instructions = ?3, source_types = ?4, project_scope = ?5,
             weight_manual_priority = ?6, weight_release_risk = ?7,
             weight_doc_gap = ?8, weight_meeting_followup = ?9, is_active = ?10,
             updated_at = datetime('now')
         WHERE id = ?11",
        params![
            profile.name,
            profile.goal,
            profile.instructions,
            serde_json::to_string(&profile.source_types).unwrap_or_else(|_| "[]".into()),
            profile.project_scope,
            profile.weight_manual_priority,
            profile.weight_release_risk,
            profile.weight_doc_gap,
            profile.weight_meeting_followup,
            profile.is_active as i64,
            profile.id,
        ],
    )?;
    Ok(())
}

pub fn delete_agent_profile(conn: &Connection, profile_id: i64) -> Result<()> {
    conn.execute("DELETE FROM agent_profiles WHERE id = ?1", params![profile_id])?;
    Ok(())
}

fn slugify(name: &str) -> String {
    let mut slug = String::new();
    let mut last_dash = false;
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash {
            slug.push('-');
            last_dash = true;
        }
    }
    slug.trim_matches('-').to_string()
}
