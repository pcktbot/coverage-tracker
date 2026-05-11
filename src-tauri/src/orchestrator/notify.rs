use crate::orchestrator::db::Store;

pub fn badge_count(store: &Store) -> usize {
    let conn = store.lock_conn();
    let mut stmt = conn.prepare(
        "SELECT COUNT(*) FROM sessions WHERE status='needs_input' AND ended_at IS NULL"
    ).unwrap();
    stmt.query_row([], |r| r.get::<_, i64>(0)).unwrap_or(0) as usize
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orchestrator::db::Store;

    #[test]
    fn counts_only_needs_input_active() {
        let s = Store::open_in_memory().unwrap();
        s.upsert_session_start("a", None, "/", 1, 1).unwrap();
        s.upsert_session_start("b", None, "/", 2, 1).unwrap();
        s.apply_status("b", "needs_input", 2, None).unwrap();
        assert_eq!(badge_count(&s), 1);
    }
}
