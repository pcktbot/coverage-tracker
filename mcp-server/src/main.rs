mod tools;
mod orchestrator;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::{self, BufRead, Write};
use std::sync::{Arc, OnceLock};

static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
static ORCH: OnceLock<Arc<orchestrator::OrchestratorClient>> = OnceLock::new();

fn rt() -> &'static tokio::runtime::Runtime {
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all().worker_threads(2).build()
            .expect("build tokio runtime")
    })
}

fn orch() -> Arc<orchestrator::OrchestratorClient> {
    ORCH.get_or_init(|| {
        let base = std::env::var("ORCHESTRATOR_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:9876".into());
        Arc::new(orchestrator::OrchestratorClient::new(base))
    }).clone()
}

// ── MCP protocol types ────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct Request {
    jsonrpc: String,
    id: Value,
    method: String,
    #[serde(default)]
    params: Value,
}

#[derive(Debug, Serialize)]
struct Response {
    jsonrpc: String,
    id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<ErrorObj>,
}

#[derive(Debug, Serialize)]
struct ErrorObj {
    code: i32,
    message: String,
}

impl Response {
    fn ok(id: Value, result: Value) -> Self {
        Self { jsonrpc: "2.0".into(), id, result: Some(result), error: None }
    }
    fn err(id: Value, code: i32, msg: impl ToString) -> Self {
        Self { jsonrpc: "2.0".into(), id, result: None, error: Some(ErrorObj { code, message: msg.to_string() }) }
    }
}

fn write_response(resp: &Response) {
    let s = serde_json::to_string(resp).unwrap();
    println!("{}", s);
    io::stdout().flush().ok();
}

// ── Tool registry ─────────────────────────────────────────────────────────────

fn tools_list() -> Value {
    json!({
        "tools": [
            {
                "name": "list_repos",
                "description": "List all tracked repositories with their latest coverage percentage.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "org": { "type": "string", "description": "Filter by GitHub org name (optional)" }
                    }
                }
            },
            {
                "name": "get_coverage_summary",
                "description": "Get the most recent coverage run result for a repository.",
                "inputSchema": {
                    "type": "object",
                    "required": ["repo_name"],
                    "properties": {
                        "repo_name": { "type": "string", "description": "Repository name" },
                        "org": { "type": "string", "description": "GitHub org (optional, narrows search)" }
                    }
                }
            },
            {
                "name": "get_coverage_trend",
                "description": "Get historical coverage percentages for a repository (most recent runs first).",
                "inputSchema": {
                    "type": "object",
                    "required": ["repo_name"],
                    "properties": {
                        "repo_name": { "type": "string", "description": "Repository name" },
                        "limit": { "type": "integer", "description": "Number of data points (default 20)" }
                    }
                }
            },
            {
                "name": "search_file_coverage",
                "description": "Find coverage data for files matching a path pattern across all repos.",
                "inputSchema": {
                    "type": "object",
                    "required": ["pattern"],
                    "properties": {
                        "pattern": { "type": "string", "description": "Substring or glob pattern to match file paths" },
                        "repo_name": { "type": "string", "description": "Limit to a specific repo (optional)" }
                    }
                }
            },
            {
                "name": "list_repo_docs",
                "description": "List markdown docs for a local repo checkout, with optional runbook-only filtering and text search.",
                "inputSchema": {
                    "type": "object",
                    "required": ["repo_name"],
                    "properties": {
                        "repo_name": { "type": "string", "description": "Repository name" },
                        "org": { "type": "string", "description": "GitHub org (optional, narrows search)" },
                        "query": { "type": "string", "description": "Filter docs by path/title/content" },
                        "runbooks_only": { "type": "boolean", "description": "Limit to likely runbooks" }
                    }
                }
            },
            {
                "name": "read_repo_doc",
                "description": "Read a markdown doc from a local repo checkout.",
                "inputSchema": {
                    "type": "object",
                    "required": ["repo_name", "path"],
                    "properties": {
                        "repo_name": { "type": "string", "description": "Repository name" },
                        "org": { "type": "string", "description": "GitHub org (optional, narrows search)" },
                        "path": { "type": "string", "description": "Relative markdown path inside the repo" }
                    }
                }
            },
            {
                "name": "report_progress",
                "description": "Append a free-form progress note to this session's timeline.",
                "inputSchema": {
                    "type": "object",
                    "required": ["summary"],
                    "properties": { "summary": { "type": "string" } }
                }
            },
            {
                "name": "attach_artifact",
                "description": "Register an artifact (file path, URL, or PR link) produced by this session.",
                "inputSchema": {
                    "type": "object",
                    "required": ["path"],
                    "properties": {
                        "path": { "type": "string" },
                        "label": { "type": "string" }
                    }
                }
            },
            {
                "name": "send_to",
                "description": "Send a message to another Claude session's inbox.",
                "inputSchema": {
                    "type": "object",
                    "required": ["sessionId", "message"],
                    "properties": {
                        "sessionId": { "type": "string" },
                        "message": { "type": "string" }
                    }
                }
            },
            {
                "name": "check_inbox",
                "description": "Read and clear this session's pending inbox messages.",
                "inputSchema": { "type": "object", "properties": {} }
            },
            {
                "name": "search_repo_docs",
                "description": "Search markdown docs across one repo or all local repos.",
                "inputSchema": {
                    "type": "object",
                    "required": ["pattern"],
                    "properties": {
                        "pattern": { "type": "string", "description": "Text to search within repo docs" },
                        "repo_name": { "type": "string", "description": "Limit to a single repo (optional)" },
                        "org": { "type": "string", "description": "GitHub org (optional, narrows repo lookup)" },
                        "runbooks_only": { "type": "boolean", "description": "Limit to likely runbooks" }
                    }
                }
            }
        ]
    })
}

fn dispatch_tool(name: &str, args: &Value) -> Result<Value> {
    // Orchestrator tools — async, bridged via the static runtime. No DB needed.
    match name {
        "report_progress" => {
            let summary = args.get("summary").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let client = orch();
            return Ok(match rt().block_on(client.report_progress(&summary)) {
                Ok(_) => json!({"ok": true}),
                Err(_) => json!({"offline": true}),
            });
        }
        "attach_artifact" => {
            let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let label = args.get("label").and_then(|v| v.as_str()).map(String::from);
            let client = orch();
            return Ok(match rt().block_on(client.attach_artifact(&path, label.as_deref())) {
                Ok(id) => json!({"id": id}),
                Err(_) => json!({"offline": true}),
            });
        }
        "send_to" => {
            let sid = args.get("sessionId").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let msg = args.get("message").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let client = orch();
            return Ok(match rt().block_on(client.send_to(&sid, &msg)) {
                Ok(_) => json!({"ok": true}),
                Err(_) => json!({"offline": true}),
            });
        }
        "check_inbox" => {
            let client = orch();
            return Ok(match rt().block_on(client.check_inbox()) {
                Ok(msgs) => json!({"messages": msgs}),
                Err(_) => json!({"offline": true}),
            });
        }
        _ => {}
    }

    // Existing read-only DB-backed tools.
    let conn = tools::open_db()?;
    match name {
        "list_repos" => tools::list_repos(&conn, args),
        "get_coverage_summary" => tools::get_coverage_summary(&conn, args),
        "get_coverage_trend" => tools::get_coverage_trend(&conn, args),
        "search_file_coverage" => tools::search_file_coverage(&conn, args),
        "list_repo_docs" => tools::list_repo_docs(&conn, args),
        "read_repo_doc" => tools::read_repo_doc(&conn, args),
        "search_repo_docs" => tools::search_repo_docs(&conn, args),
        _ => anyhow::bail!("Unknown tool: {}", name),
    }
}

// ── Main loop ─────────────────────────────────────────────────────────────────

fn main() {
    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) if !l.trim().is_empty() => l,
            _ => continue,
        };

        let req: Request = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(e) => {
                let resp = Response::err(Value::Null, -32700, format!("Parse error: {}", e));
                write_response(&resp);
                continue;
            }
        };

        let resp = handle(&req);
        write_response(&resp);
    }
}

fn handle(req: &Request) -> Response {
    match req.method.as_str() {
        "initialize" => Response::ok(
            req.id.clone(),
            json!({
                "protocolVersion": "2024-11-05",
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "coverage-mcp", "version": "0.1.0" }
            }),
        ),
        "tools/list" => Response::ok(req.id.clone(), tools_list()),
        "tools/call" => {
            let tool_name = req.params.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let args = req.params.get("arguments").unwrap_or(&Value::Null);
            match dispatch_tool(tool_name, args) {
                Ok(result) => Response::ok(
                    req.id.clone(),
                    json!({ "content": [{ "type": "text", "text": result.to_string() }] }),
                ),
                Err(e) => Response::err(req.id.clone(), -32603, e.to_string()),
            }
        }
        "notifications/initialized" | "ping" => {
            // No response needed for notifications, but send empty ok for ping
            Response::ok(req.id.clone(), json!({}))
        }
        _ => Response::err(req.id.clone(), -32601, format!("Method not found: {}", req.method)),
    }
}
