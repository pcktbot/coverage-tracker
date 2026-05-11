# Orchestrator hooks

Five `~/.claude/` hook scripts that report Claude Code session lifecycle
events to the coverage-manager orchestrator HTTP server at
`http://127.0.0.1:9876`. If the server is down, hooks silently no-op
(curl `--max-time 3`).

## Install

1. `chmod +x scripts/hooks/*.sh`.
2. Open `~/.claude/settings.json`.
3. Copy entries from `settings-snippet.json`, replacing `COVERAGE_MANAGER_REPO`
   with this repo's absolute path.
4. Append to each `hooks.<EventName>` array rather than overwriting.
5. Restart any running Claude Code session.

## Verify

With coverage-manager running, start a new Claude Code session and
`curl http://127.0.0.1:9876/sessions`.
