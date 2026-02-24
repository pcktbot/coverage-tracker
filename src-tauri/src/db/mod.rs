pub mod migrations;
pub mod repos;
pub mod coverage;

use std::path::PathBuf;
use rusqlite::Connection;
use anyhow::Result;
use dirs;

pub fn db_path() -> PathBuf {
    if let Ok(p) = std::env::var("COVERAGE_DB_PATH") {
        return PathBuf::from(p);
    }
    let base = dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("coverage-manager").join("coverage.db")
}

pub fn open() -> Result<Connection> {
    let path = db_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let conn = Connection::open(&path)?;
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
    migrations::run(&conn)?;
    Ok(conn)
}
