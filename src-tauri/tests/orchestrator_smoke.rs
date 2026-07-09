#[tokio::test(flavor = "multi_thread")]
async fn server_starts_and_responds_on_random_port() {
    use coverage_manager_lib::orchestrator;
    use std::sync::Arc;
    let store = Arc::new(orchestrator::db::Store::open_in_memory().unwrap());
    let state = orchestrator::state::AppState::new(store);
    let (port, handle) = orchestrator::spawn_on_random_port(state).await.unwrap();
    let resp = reqwest::get(format!("http://127.0.0.1:{port}/sessions")).await.unwrap();
    assert!(resp.status().is_success());
    handle.abort();
}
