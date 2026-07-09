use std::process::Command;
use std::time::Duration;

pub fn parent_chain(start_pid: i64) -> std::io::Result<Vec<i64>> {
    let mut out = vec![start_pid];
    let mut cur = start_pid;
    for _ in 0..50 {
        let stdout = Command::new("ps")
            .args(["-o", "ppid=", "-p", &cur.to_string()])
            .output()?;
        let s = String::from_utf8_lossy(&stdout.stdout).trim().to_string();
        let parent: i64 = s.parse().unwrap_or(0);
        if parent <= 1 { out.push(parent); break; }
        out.push(parent);
        cur = parent;
    }
    Ok(out)
}

pub struct OrchestratorClient {
    base: String,
    http: reqwest::Client,
    session_id: tokio::sync::OnceCell<String>,
    retry_total: Duration,
    retry_step:  Duration,
}

impl OrchestratorClient {
    pub fn new(base: String) -> Self {
        Self {
            base,
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(2)).build().unwrap_or_default(),
            session_id: Default::default(),
            retry_total: Duration::from_secs(10),
            retry_step:  Duration::from_millis(250),
        }
    }

    /// Constructor for tests with a tighter retry budget so the late-registration
    /// test doesn't actually need to wait 10s.
    #[cfg(test)]
    pub fn new_with_retry(base: String, total: Duration, step: Duration) -> Self {
        let mut c = Self::new(base);
        c.retry_total = total;
        c.retry_step = step;
        c
    }

    async fn try_lookup(&self, pid: i64)
        -> Result<Option<String>, Box<dyn std::error::Error + Send + Sync>>
    {
        let url = format!("{}/sessions/by-pid/{}", self.base, pid);
        let resp = self.http.get(&url).send().await?;
        if resp.status().is_success() {
            let v: serde_json::Value = resp.json().await?;
            Ok(v.get("session_id").and_then(|x| x.as_str()).map(String::from))
        } else if resp.status() == reqwest::StatusCode::NOT_FOUND {
            Ok(None)
        } else {
            Err(format!("by-pid lookup http {}", resp.status()).into())
        }
    }

    pub async fn resolve_session_for_pid(&self, pid: i64)
        -> Result<String, Box<dyn std::error::Error + Send + Sync>>
    {
        let deadline = std::time::Instant::now() + self.retry_total;
        loop {
            for p in parent_chain(pid)? {
                if let Ok(Some(sid)) = self.try_lookup(p).await {
                    return Ok(sid);
                }
            }
            if std::time::Instant::now() >= deadline {
                return Err("no session for any ancestor pid (timed out)".into());
            }
            tokio::time::sleep(self.retry_step).await;
        }
    }

    pub async fn session_id(&self) -> Option<String> {
        if let Some(s) = self.session_id.get() { return Some(s.clone()); }
        let pid = std::process::id() as i64;
        match self.resolve_session_for_pid(pid).await {
            Ok(sid) => { let _ = self.session_id.set(sid.clone()); Some(sid) }
            Err(_)  => None,
        }
    }

    pub async fn report_progress(&self, summary: &str)
        -> Result<(), Box<dyn std::error::Error + Send + Sync>>
    {
        let sid = self.session_id().await.ok_or("no session id")?;
        self.http.post(format!("{}/progress", self.base))
            .json(&serde_json::json!({"session_id": sid, "summary": summary}))
            .send().await?.error_for_status()?;
        Ok(())
    }

    pub async fn attach_artifact(&self, path: &str, label: Option<&str>)
        -> Result<i64, Box<dyn std::error::Error + Send + Sync>>
    {
        let sid = self.session_id().await.ok_or("no session id")?;
        let v: serde_json::Value = self.http.post(format!("{}/artifact", self.base))
            .json(&serde_json::json!({"session_id": sid, "path": path, "label": label}))
            .send().await?.error_for_status()?.json().await?;
        Ok(v["id"].as_i64().unwrap_or(0))
    }

    pub async fn send_to(&self, target: &str, message: &str)
        -> Result<(), Box<dyn std::error::Error + Send + Sync>>
    {
        let sid = self.session_id().await;
        self.http.post(format!("{}/inbox/{}", self.base, target))
            .json(&serde_json::json!({
                "from_kind": "session",
                "from_id": sid,
                "message": message
            }))
            .send().await?.error_for_status()?;
        Ok(())
    }

    pub async fn check_inbox(&self)
        -> Result<Vec<serde_json::Value>, Box<dyn std::error::Error + Send + Sync>>
    {
        let sid = self.session_id().await.ok_or("no session id")?;
        let v: serde_json::Value = self.http.get(format!("{}/inbox/{}", self.base, sid))
            .send().await?.error_for_status()?.json().await?;
        Ok(v["messages"].as_array().cloned().unwrap_or_default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Spawn a minimal in-process HTTP server that mimics the orchestrator's
    /// `GET /sessions/by-pid/{pid}` endpoint. The shared state is a HashMap.
    /// Returns (port, JoinHandle, shared state Arc).
    async fn spawn_mock_orchestrator()
        -> (u16, tokio::task::JoinHandle<()>, std::sync::Arc<std::sync::Mutex<std::collections::HashMap<i64, String>>>)
    {
        use axum::{Router, routing::get, extract::{State, Path}, http::StatusCode, Json};
        use std::sync::{Arc, Mutex};
        use std::collections::HashMap;

        type Db = Arc<Mutex<HashMap<i64, String>>>;
        let db: Db = Arc::new(Mutex::new(HashMap::new()));

        async fn lookup(State(db): State<Db>, Path(pid): Path<i64>)
            -> Result<Json<serde_json::Value>, StatusCode>
        {
            let g = db.lock().unwrap();
            match g.get(&pid) {
                Some(sid) => Ok(Json(serde_json::json!({"session_id": sid}))),
                None => Err(StatusCode::NOT_FOUND),
            }
        }

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let app = Router::new()
            .route("/sessions/by-pid/{pid}", get(lookup))
            .with_state(db.clone());
        let handle = tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        (port, handle, db)
    }

    #[test]
    fn parent_chain_starts_with_self() {
        let chain = parent_chain(std::process::id() as i64).unwrap();
        assert!(!chain.is_empty());
        assert_eq!(chain[0], std::process::id() as i64);
    }

    #[tokio::test]
    async fn resolve_succeeds_immediately_if_session_exists() {
        let (port, handle, db) = spawn_mock_orchestrator().await;
        let pid = std::process::id() as i64;
        db.lock().unwrap().insert(pid, "sx".into());

        let client = OrchestratorClient::new_with_retry(
            format!("http://127.0.0.1:{port}"),
            Duration::from_secs(2),
            Duration::from_millis(50),
        );
        let sid = client.resolve_session_for_pid(pid).await.unwrap();
        assert_eq!(sid, "sx");
        handle.abort();
    }

    #[derive(Default)]
    pub struct MockState {
        pub by_pid: std::collections::HashMap<i64, String>,
        pub last_progress: std::collections::HashMap<String, String>,
        pub artifacts: Vec<serde_json::Value>,
        pub inbox: std::collections::HashMap<String, Vec<serde_json::Value>>,
        pub next_artifact_id: i64,
    }

    async fn spawn_mock_full()
        -> (u16, tokio::task::JoinHandle<()>,
            std::sync::Arc<std::sync::Mutex<MockState>>)
    {
        use axum::{Router, routing::{get, post}, extract::{State, Path, Json as AxJson}, http::StatusCode, Json};
        use std::sync::{Arc, Mutex};

        type St = Arc<Mutex<MockState>>;
        let state: St = Arc::new(Mutex::new(MockState::default()));

        async fn by_pid(State(s): State<std::sync::Arc<std::sync::Mutex<MockState>>>, Path(pid): Path<i64>)
            -> Result<Json<serde_json::Value>, StatusCode>
        {
            let g = s.lock().unwrap();
            match g.by_pid.get(&pid) {
                Some(sid) => Ok(Json(serde_json::json!({"session_id": sid}))),
                None => Err(StatusCode::NOT_FOUND),
            }
        }

        async fn progress(State(s): State<std::sync::Arc<std::sync::Mutex<MockState>>>, AxJson(body): AxJson<serde_json::Value>)
            -> &'static str
        {
            let mut g = s.lock().unwrap();
            let sid = body["session_id"].as_str().unwrap_or("").to_string();
            let sum = body["summary"].as_str().unwrap_or("").to_string();
            g.last_progress.insert(sid, sum);
            "ok"
        }

        async fn artifact(State(s): State<std::sync::Arc<std::sync::Mutex<MockState>>>, AxJson(body): AxJson<serde_json::Value>)
            -> Json<serde_json::Value>
        {
            let mut g = s.lock().unwrap();
            g.next_artifact_id += 1;
            let id = g.next_artifact_id;
            let mut row = body.clone();
            row["id"] = id.into();
            g.artifacts.push(row);
            Json(serde_json::json!({"id": id}))
        }

        async fn post_inbox(State(s): State<std::sync::Arc<std::sync::Mutex<MockState>>>, Path(sid): Path<String>, AxJson(body): AxJson<serde_json::Value>)
            -> &'static str
        {
            s.lock().unwrap().inbox.entry(sid).or_default().push(body);
            "ok"
        }

        async fn get_inbox(State(s): State<std::sync::Arc<std::sync::Mutex<MockState>>>, Path(sid): Path<String>)
            -> Json<serde_json::Value>
        {
            let mut g = s.lock().unwrap();
            let msgs = g.inbox.remove(&sid).unwrap_or_default();
            Json(serde_json::json!({"messages": msgs}))
        }

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let app = Router::new()
            .route("/sessions/by-pid/{pid}", get(by_pid))
            .route("/progress", post(progress))
            .route("/artifact", post(artifact))
            .route("/inbox/{sid}", post(post_inbox).get(get_inbox))
            .with_state(state.clone());
        let handle = tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        (port, handle, state)
    }

    #[tokio::test]
    async fn report_progress_lands() {
        let (port, handle, state) = spawn_mock_full().await;
        let pid = std::process::id() as i64;
        state.lock().unwrap().by_pid.insert(pid, "sx".into());

        let client = OrchestratorClient::new_with_retry(
            format!("http://127.0.0.1:{port}"),
            Duration::from_secs(2),
            Duration::from_millis(50),
        );
        client.report_progress("compiled").await.unwrap();
        assert_eq!(state.lock().unwrap().last_progress.get("sx").map(String::as_str), Some("compiled"));
        handle.abort();
    }

    #[tokio::test]
    async fn attach_artifact_returns_id() {
        let (port, handle, state) = spawn_mock_full().await;
        let pid = std::process::id() as i64;
        state.lock().unwrap().by_pid.insert(pid, "sx".into());

        let client = OrchestratorClient::new_with_retry(
            format!("http://127.0.0.1:{port}"),
            Duration::from_secs(2),
            Duration::from_millis(50),
        );
        let id = client.attach_artifact("/tmp/out.txt", Some("out")).await.unwrap();
        assert!(id > 0);
        handle.abort();
    }

    #[tokio::test]
    async fn send_to_then_check_inbox() {
        let (port, handle, state) = spawn_mock_full().await;
        let pid = std::process::id() as i64;
        state.lock().unwrap().by_pid.insert(pid, "me".into());

        let me = OrchestratorClient::new_with_retry(
            format!("http://127.0.0.1:{port}"),
            Duration::from_secs(2),
            Duration::from_millis(50),
        );
        me.send_to("you", "ping").await.unwrap();
        assert_eq!(state.lock().unwrap().inbox.get("you").map(|v| v.len()).unwrap_or(0), 1);
        handle.abort();
    }

    #[tokio::test]
    async fn resolve_retries_until_session_registers() {
        let (port, handle, db) = spawn_mock_orchestrator().await;
        let pid = std::process::id() as i64;

        // Register the session 200ms after starting the resolution.
        let db_late = db.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(200)).await;
            db_late.lock().unwrap().insert(pid, "late".into());
        });

        let client = OrchestratorClient::new_with_retry(
            format!("http://127.0.0.1:{port}"),
            Duration::from_secs(5),
            Duration::from_millis(50),
        );
        let sid = client.resolve_session_for_pid(pid).await.unwrap();
        assert_eq!(sid, "late");
        handle.abort();
    }
}
