# Claude Session Orchestrator — Design

**Date:** 2026-05-08
**Status:** Approved for implementation planning
**Host app:** `coverage-manager` (Tauri 2 + SvelteKit, with existing `mcp-server/`)

## Problem

Running multiple long-lived Claude Code sessions in separate terminals creates four pain points:

1. Hard to tell which session is **idle vs working vs needs input**.
2. No glanceable view of **what each session is doing**.
3. No notification when a session finishes, errors, or pauses for input.
4. No aggregation of **artifacts** each session produces (PRs, files, summaries).

The user already maintains a Tauri desktop app (`coverage-manager`) and prefers to extend it rather than build a sibling app.

## Goals

- Track all running Claude sessions with live status.
- Native macOS notifications on lifecycle events.
- Aggregate per-session artifacts and progress notes.
- Provide a high-level command channel from app → agent (no terminal keystroke injection).
- Allow agents to coordinate with each other through the orchestrator.

## Non-goals

- No PTY/terminal control (no tmux/iTerm `send-keys`). User keeps interacting with each session via its terminal.
- No mobile/push notifications.
- No multi-machine support — single-host only.
- No replacement for Claude Code's own permission prompts; the orchestrator observes, it does not approve tools.

## Architecture

```
┌─ Claude session A ─┐   ┌─ Claude session B ─┐
│ ~/.claude hooks    │   │ ~/.claude hooks    │
│  SessionStart      │   │  PreToolUse        │
│  PreToolUse        │   │  Notification      │
│  Notification      │   │  Stop              │
│  Stop              │   │                    │
│  UserPromptSubmit  │   │  UserPromptSubmit  │
└──────┬─────────────┘   └──────┬─────────────┘
       │  curl POST http://127.0.0.1:9876/...
       └──────────────┬──────────┘
                      ▼
        ┌──────────────────────────────────┐
        │ coverage-manager (Tauri app)     │
        │                                  │
        │  Rust:                           │
        │   • axum HTTP server on :9876    │
        │   • SQLite (sessions, events,    │
        │     artifacts, inbox)            │
        │   • tray icon + badge            │
        │   • notification plugin          │
        │   • Tauri events → frontend      │
        │                                  │
        │  Svelte: /sessions route         │
        │   live list, filters, drill-down │
        │                                  │
        │  MCP server (existing) extends   │
        │   with orchestrator tools        │
        └──────────────────────────────────┘
```

A single Tauri process owns HTTP ingest, persistence, tray, notifications, and UI. The existing `mcp-server/` gains four new tools that proxy to the local HTTP API. Closing the Tauri window does not stop the orchestrator — only quitting the tray does.

## Components

### Rust (`src-tauri/`)

`axum` HTTP server bound to `127.0.0.1:9876`:

| Method & path | Caller | Purpose |
|---|---|---|
| `POST /event` | hook scripts | Lifecycle events (session_start, stop, notification, pre_tool, user_prompt) |
| `POST /progress` | MCP `report_progress` | Free-form progress note |
| `POST /artifact` | MCP `attach_artifact` | Register an output (path/url + label) |
| `POST /inbox/:sessionId` | UI + MCP `send_to` | Queue a message for a session |
| `GET /inbox/:sessionId` | MCP `check_inbox` | Read & mark delivered |
| `GET /sessions` | UI bootstrap | List sessions |
| `GET /events?session=…` | UI drawer | Event timeline for a session |

Persistence: `sqlx` + SQLite at `~/Library/Application Support/coverage-manager/orchestrator.db`.

Tray icon (Tauri tray plugin) shows a badge equal to the count of `needs_input` sessions. Clicking opens the main window to `/sessions`. Right-click menu: Show, Quit.

Notifications (Tauri `notification` plugin) fire on:
- `session.needs_input` (Notification hook event)
- `session.done` (Stop hook with successful end)
- `session.error` (Stop hook with error reason)

State changes are emitted as Tauri events (`orchestrator://state`) so the Svelte frontend can update without polling.

### MCP server (extends existing `mcp-server/`)

Four new tools, each a thin wrapper over the local HTTP API:

| Tool | Args | Returns |
|---|---|---|
| `report_progress` | `summary: string` | `ok` |
| `attach_artifact` | `path: string`, `label?: string` | `{ id }` |
| `send_to` | `sessionId: string`, `message: string` | `ok` |
| `check_inbox` | (none) | `{ messages: [{from, ts, message}] }` (then marked delivered) |

Tools must learn the active session id to attribute calls correctly. The implementation will choose one of: (a) `SessionStart` hook writes the session id to a per-pid file the MCP server reads on startup, (b) the agent calls a `register(sessionId)` tool first, or (c) the MCP server is launched per-session with the id baked into argv. The plan should pick one based on what Claude Code actually exposes to MCP processes. If the orchestrator HTTP server is unreachable, tools return a structured error `{ offline: true }` and the agent continues.

### Hooks (installed once at `~/.claude/settings.json`)

Each hook is a small bash script that posts JSON to `/event` with `--max-time 1` so it never blocks the agent. Hook scripts live in `coverage-manager/scripts/hooks/` and are referenced by absolute path from `settings.json`.

| Hook | Payload extras | Effect on session row |
|---|---|---|
| `SessionStart` | cwd, pid, env-derived label | Insert/upsert; status = `working` |
| `UserPromptSubmit` | last prompt (truncated) | status = `working`; record `last_user_prompt` |
| `PreToolUse` | tool name | record `current_tool` |
| `Notification` | message | status = `needs_input`; trigger native notification |
| `Stop` | reason | status = `done` or `error`; record `ended_at` |

Status derivation lives in Rust, not the hook. Hooks just report raw events.

### Svelte `/sessions` route

- Live list (subscribed to Tauri events): each row shows status dot, label, cwd, current tool, last progress note, age, badge for unread inbox.
- Filter chips: `working` / `idle` / `needs_input` / `done` / `error`.
- Click a row → drawer with:
  - Event timeline (most recent first).
  - Artifact list (clickable; opens via Tauri `opener` plugin).
  - Inbox composer: text area + send button → `POST /inbox/:sessionId` with `from_kind=human`.
  - "Pending inbox" list — messages queued but not yet picked up by `check_inbox`.

## Data model (SQLite)

```sql
sessions(
  id TEXT PRIMARY KEY,            -- claude session_id from hook payload
  label TEXT,
  cwd TEXT,
  pid INTEGER,
  status TEXT,                    -- working | idle | needs_input | done | error | unknown
  current_tool TEXT,
  last_progress TEXT,
  last_user_prompt TEXT,
  started_at INTEGER,
  updated_at INTEGER,
  ended_at INTEGER,
  end_reason TEXT
);

events(
  id INTEGER PRIMARY KEY,
  session_id TEXT REFERENCES sessions(id),
  ts INTEGER,
  kind TEXT,                      -- session_start|stop|notification|pre_tool|user_prompt|progress|artifact|inbox_in|inbox_out
  payload TEXT                    -- JSON blob
);

artifacts(
  id INTEGER PRIMARY KEY,
  session_id TEXT,
  ts INTEGER,
  path TEXT,
  label TEXT,
  kind TEXT                       -- file|url|pr (inferred from path)
);

inbox(
  id INTEGER PRIMARY KEY,
  session_id TEXT,                -- recipient
  from_kind TEXT,                 -- human | session
  from_id TEXT,                   -- sender session id if from_kind=session, else null
  ts INTEGER,
  message TEXT,
  delivered_at INTEGER            -- null until check_inbox returns it
);
```

Status is derived by Rust from the event stream; agents do not set status directly. The events table is append-only.

## Error handling & resilience

- **Tauri app down when a hook fires:** `curl --max-time 1` fails silently and returns 0 from the hook. The session simply isn't tracked for that interval. No agent-visible failure.
- **MCP tool with orchestrator offline:** returns `{ offline: true }` instead of erroring. Agent continues.
- **Stale sessions:** a Rust background task marks `working` sessions `idle` after 5 min with no events, and `unknown` after 1 hr with no Stop event.
- **Inbox at-least-once:** `check_inbox` sets `delivered_at` only after the response body has been written. A crashed agent re-receives the message next call.
- **DB schema migrations:** include a `schema_version` table; bump and migrate on app start.

## Testing

- **Rust:** `cargo test` for HTTP handlers and the status-derivation state machine (table-driven: event in → status out).
- **MCP tools:** unit tests against a mock HTTP server.
- **Hooks:** shell tests with a recording fake server (`bats` or bash + `nc`); validate every hook script's payload shape.
- **Frontend:** Svelte component tests with mocked Tauri events; one Playwright smoke test (post event → row appears).
- **End-to-end:** `make e2e` target spawns a fake Claude session script that fires hooks in sequence; asserts UI shows the expected progression.

## Open questions / deferred

- **Session labeling UX:** auto-derived from cwd basename in v1. Manual rename via the UI is deferred.
- **Inbox surfacing reliability:** in v1, agents must voluntarily call `check_inbox`. If polling proves unreliable in practice, add a `UserPromptSubmit` hook that prepends pending inbox messages to the next prompt.
- **Inter-machine sync:** out of scope.
- **Auth on the local HTTP server:** localhost-only bind is the boundary in v1. If multi-user-on-host becomes a concern, add a per-session shared secret written to a 0600 file.
