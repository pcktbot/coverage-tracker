use anyhow::Result;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedConfluencePage {
    pub page_id: String,
    pub space_key: Option<String>,
    pub title: String,
    pub web_url: String,
    pub version_number: Option<i64>,
    pub last_updated_at: Option<String>,
    pub last_fetched_at: String,
    pub raw_html: String,
    pub plain_text: String,
    pub excerpt: String,
}

pub fn get_page(conn: &Connection, page_id: &str) -> Result<Option<CachedConfluencePage>> {
    let result = conn.query_row(
        "SELECT page_id, space_key, title, web_url, version_number, last_updated_at,
                last_fetched_at, raw_html, plain_text, excerpt
         FROM confluence_pages
         WHERE page_id = ?1",
        params![page_id],
        |row| {
            Ok(CachedConfluencePage {
                page_id: row.get(0)?,
                space_key: row.get(1)?,
                title: row.get(2)?,
                web_url: row.get(3)?,
                version_number: row.get(4)?,
                last_updated_at: row.get(5)?,
                last_fetched_at: row.get(6)?,
                raw_html: row.get(7)?,
                plain_text: row.get(8)?,
                excerpt: row.get(9)?,
            })
        },
    );

    match result {
        Ok(page) => Ok(Some(page)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(err) => Err(err.into()),
    }
}

pub fn list_pages(conn: &Connection, page_ids: &[String]) -> Result<Vec<CachedConfluencePage>> {
    let mut pages = Vec::new();
    for page_id in page_ids {
        if let Some(page) = get_page(conn, page_id)? {
            pages.push(page);
        }
    }
    Ok(pages)
}

pub fn upsert_page(conn: &Connection, page: &CachedConfluencePage) -> Result<()> {
    conn.execute(
        "INSERT INTO confluence_pages (
            page_id, space_key, title, web_url, version_number, last_updated_at,
            last_fetched_at, raw_html, plain_text, excerpt
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, datetime('now'), ?7, ?8, ?9)
         ON CONFLICT(page_id) DO UPDATE SET
            space_key = excluded.space_key,
            title = excluded.title,
            web_url = excluded.web_url,
            version_number = excluded.version_number,
            last_updated_at = excluded.last_updated_at,
            last_fetched_at = datetime('now'),
            raw_html = excluded.raw_html,
            plain_text = excluded.plain_text,
            excerpt = excluded.excerpt",
        params![
            page.page_id,
            page.space_key,
            page.title,
            page.web_url,
            page.version_number,
            page.last_updated_at,
            page.raw_html,
            page.plain_text,
            page.excerpt,
        ],
    )?;
    Ok(())
}
