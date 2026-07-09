# Runbook

Quick reference for resolving operational issues with the coverage-manager app + Claude session orchestrator.

## Quick checks

```bash
# Is the orchestrator HTTP server running?
curl -s --max-time 1 http://127.0.0.1:9876/sessions >/dev/null && echo OK || echo "NOT RUNNING"

# What does it know about sessions?
curl -s http://127.0.0.1:9876/sessions | python3 -m json.tool | head -40

# Are the SessionStart / UserPromptSubmit hooks registered?
jq '.hooks | keys' ~/.claude/settings.json
```

## Issue: Claude sessions don't show up in the UI

### Symptom
You start a new Claude Code session in a project, but no card appears under **Sessions** (`/inbox`).

### Most likely causes, in order

**1. The Tauri app isn't running.**
The orchestrator is embedded in the Tauri app's lifecycle. It only listens on `127.0.0.1:9876` while the app window is open.

Fix: launch the app (`npm run tauri dev` from the repo root, or open the built app), then start your Claude session.

**2. Hooks aren't registered with Claude Code.**
Claude Code only runs our scripts if they're wired into `~/.claude/settings.json`. Confirm with:

```bash
jq '.hooks.SessionStart' ~/.claude/settings.json
```

You should see a command pointing at `<repo>/scripts/hooks/session-start.sh`. If empty/null, copy the entries from `scripts/hooks/settings-snippet.json` into your `~/.claude/settings.json` under the `hooks` key.

**3. Stale frontend.**
The UI subscribes to `orchestrator://state` events. If the listener was set up before the orchestrator was reachable, the page may not auto-refresh. Reload the Sessions view.

## Issue: Orchestrator returns SQLite "no such column" errors

### Symptom
`GET /sessions` (or any orchestrator endpoint that reads sessions) returns an error like:
```
no such column: loaded_snapshot in SELECT id,label,cwd,... FROM sessions ...
```

### Root cause
The DB schema drifted from `schema_version`. As of the self-healing migration, this should auto-repair on the next app start. If it doesn't, manually repair the column.

### Repair

```bash
# 1. Stop the app first
osascript -e 'quit app "coverage-manager"'   # or close the window/dev process

# 2. Add the missing column manually (replace column name as needed)
sqlite3 ~/Library/Application\ Support/coverage-manager/orchestrator.db \
  "ALTER TABLE sessions ADD COLUMN <missing_column> TEXT"

# 3. Verify
sqlite3 ~/Library/Application\ Support/coverage-manager/orchestrator.db \
  "PRAGMA table_info(sessions)"

# 4. Restart the app
```

Known columns at current schema (v3): `id, label, cwd, pid, status, current_tool, last_progress, last_user_prompt, started_at, updated_at, ended_at, end_reason, transcript_path, artifact_kind, artifact_id, artifact_title, artifact_url, loaded_snapshot`.

## Issue: Reset orchestrator DB from scratch

If state has become unrecoverable, you can blow it away. The Tauri app will recreate a fresh DB on next launch (you'll lose all session history).

```bash
# Stop the app
osascript -e 'quit app "coverage-manager"'

# Move the DB aside (keeps a backup in case you want it)
mv ~/Library/Application\ Support/coverage-manager/orchestrator.db \
   ~/Library/Application\ Support/coverage-manager/orchestrator.db.bak

# Restart the app — it will create a fresh DB
```

`coverage.db` (your projects/teams/settings) is a separate file and is untouched by this.

## Issue: Inspecting the DB

Two files live under `~/Library/Application Support/coverage-manager/`:

- `coverage.db` — projects, teams, settings, agent profiles
- `orchestrator.db` — Claude session state (sessions, events, artifacts, inbox)

```bash
# List tables
sqlite3 ~/Library/Application\ Support/coverage-manager/orchestrator.db ".tables"

# Schema of a table
sqlite3 ~/Library/Application\ Support/coverage-manager/orchestrator.db ".schema sessions"

# Recent rows
sqlite3 ~/Library/Application\ Support/coverage-manager/orchestrator.db \
  "SELECT id, status, cwd, datetime(updated_at, 'unixepoch', 'localtime') FROM sessions ORDER BY updated_at DESC LIMIT 10"
```

The app also exposes a read-only DB viewer at `/admin/db` for browsing without dropping into a shell.

## Issue: Hooks fire but events never persist

### Symptom
You can `tail -f` a hook log (or `bash -x` the hook script) and see it running, but `/sessions` shows nothing.

### Likely cause
Hook is POSTing to `127.0.0.1:9876` but the orchestrator is down. Curl in `scripts/hooks/_common.sh:post_event` silently swallows failures (`|| true`) — by design (we don't want a broken orchestrator to crash Claude sessions).

Verify with the quick check at the top of this file. If the orchestrator is down, start the Tauri app.

If you want visibility into silent failures, add `set -x` to a hook temporarily, or capture curl output:

```bash
# in scripts/hooks/_common.sh, replace post_event temporarily:
post_event() {
  curl -v --max-time 3 -X POST -H 'content-type: application/json' \
    --data "$1" "$ORCH_URL/event" 2>>/tmp/orch-hook.log
}
```

Then `tail -f /tmp/orch-hook.log`.

## Locations

| File | Purpose |
|---|---|
| `~/Library/Application Support/coverage-manager/orchestrator.db` | Session state |
| `~/Library/Application Support/coverage-manager/coverage.db` | Project/settings state |
| `~/.claude/settings.json` | Where Claude Code hook commands are registered |
| `<repo>/scripts/hooks/` | The hook scripts that POST to the orchestrator |
| `<repo>/scripts/hooks/settings-snippet.json` | Paste-in template for `~/.claude/settings.json` |
