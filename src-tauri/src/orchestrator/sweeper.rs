use crate::orchestrator::db::Store;
use std::sync::Arc;

pub const IDLE_AFTER_SECS: i64 = 5 * 60;
pub const UNKNOWN_AFTER_SECS: i64 = 60 * 60;

pub fn sweep_once(store: &Store, now: i64) -> rusqlite::Result<usize> {
    let conn = store.lock_conn();
    let mut changed = 0;
    changed += conn.execute(
        "UPDATE sessions SET status='idle'
         WHERE status='working' AND ? - updated_at >= ? AND ended_at IS NULL",
        rusqlite::params![now, IDLE_AFTER_SECS])?;
    changed += conn.execute(
        "UPDATE sessions SET status='unknown'
         WHERE status IN ('working','idle','needs_input') AND ? - updated_at >= ? AND ended_at IS NULL",
        rusqlite::params![now, UNKNOWN_AFTER_SECS])?;
    Ok(changed)
}

/// Async loop that periodically marks stale sessions idle / unknown. Spawn
/// this via `tauri::async_runtime::spawn` (or any active tokio runtime) —
/// it does not start its own runtime.
pub async fn run_loop(store: Arc<Store>) {
    let mut tick = tokio::time::interval(std::time::Duration::from_secs(30));
    loop {
        tick.tick().await;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
        let s = store.clone();
        let _ = tokio::task::spawn_blocking(move || sweep_once(&s, now)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orchestrator::db::Store;

    #[test]
    fn working_becomes_idle() {
        let s = Store::open_in_memory().unwrap();
        s.upsert_session_start("s1", None, "/tmp", 1, 1000).unwrap();
        sweep_once(&s, 1000 + IDLE_AFTER_SECS).unwrap();
        assert_eq!(s.get_session("s1").unwrap().unwrap().status, "idle");
    }
    #[test]
    fn idle_becomes_unknown() {
        let s = Store::open_in_memory().unwrap();
        s.upsert_session_start("s1", None, "/tmp", 1, 1000).unwrap();
        sweep_once(&s, 1000 + IDLE_AFTER_SECS).unwrap();
        sweep_once(&s, 1000 + UNKNOWN_AFTER_SECS).unwrap();
        assert_eq!(s.get_session("s1").unwrap().unwrap().status, "unknown");
    }
}
