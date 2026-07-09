# coverage-mcp

Read-only MCP server for Coverage Manager. Exposes coverage data and repo docs to any MCP client (Claude Code, Claude Desktop, Codex, etc.).

## Build

```bash
cargo build --release --manifest-path mcp-server/Cargo.toml
```

The binary lands at `mcp-server/target/release/coverage-mcp`.

## Database

The server reads from the same SQLite database as the app. Set `COVERAGE_DB_PATH` if you moved it, otherwise it defaults to `~/.local/share/coverage-manager/coverage.db`.

## Connecting to Claude Code

Add to `.claude/settings.json` (project) or `~/.claude/settings.json` (global):

```json
{
  "mcpServers": {
    "coverage-manager": {
      "command": "/path/to/coverage-manager/mcp-server/target/release/coverage-mcp"
    }
  }
}
```

Or use `cargo run` during development (slower startup):

```json
{
  "mcpServers": {
    "coverage-manager": {
      "command": "cargo",
      "args": ["run", "--manifest-path", "/path/to/coverage-manager/mcp-server/Cargo.toml"]
    }
  }
}
```

## Connecting to Claude Desktop

In `~/Library/Application Support/Claude/claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "coverage-manager": {
      "command": "/path/to/coverage-manager/mcp-server/target/release/coverage-mcp"
    }
  }
}
```

## Connecting to Codex

```json
{
  "mcp_servers": {
    "coverage-manager": {
      "command": "/path/to/coverage-manager/mcp-server/target/release/coverage-mcp"
    }
  }
}
```

## Available tools

| Tool | Description |
|------|-------------|
| `list_repos` | List all tracked repos with latest coverage %. Optional `org` filter. |
| `get_coverage_summary` | Most recent coverage run for a repo. |
| `get_coverage_trend` | Historical coverage percentages (most recent first, default 20 points). |
| `search_file_coverage` | Find coverage data for files matching a path pattern. |
| `list_repo_docs` | List markdown docs in a local repo checkout. Supports `query` and `runbooks_only`. |
| `read_repo_doc` | Read a specific markdown file from a local repo checkout. |
| `search_repo_docs` | Full-text search across markdown docs in one or all repos. |

The `mcp_enabled` toggle in Settings → MCP access is informational only — it does not start or stop the server process.
