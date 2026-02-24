mod tools;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::{self, BufRead, Write};

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
            }
        ]
    })
}

fn dispatch_tool(name: &str, args: &Value) -> Result<Value> {
    let conn = tools::open_db()?;
    match name {
        "list_repos" => tools::list_repos(&conn, args),
        "get_coverage_summary" => tools::get_coverage_summary(&conn, args),
        "get_coverage_trend" => tools::get_coverage_trend(&conn, args),
        "search_file_coverage" => tools::search_file_coverage(&conn, args),
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
