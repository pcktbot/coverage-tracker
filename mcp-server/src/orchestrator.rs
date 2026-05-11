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
