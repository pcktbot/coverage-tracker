# Claude Behavior Artifacts — Phase 1 Design

**Status:** Approved
**Date:** 2026-05-11
**Owner:** David Miller

## Background

The personal usage report at `~/.claude/usage-data/report.html` surfaced three
high-leverage behavioral patterns Claude Code sessions should adopt:

1. **Plan before patch** — force a written root-cause + ranked hypotheses
   diagnosis before any edit, in response to recurring "jumped to fix"
   incidents.
2. **Restore first for deleted files** — default to `git log --all -- <path>`
   and `git show` recovery instead of regenerating from scratch.
3. **Parallel repo explorer** — explicit Task-agent fan-out for cross-repo
   work, with a fixed report shape per agent.

Phase 1 ships these as **reusable Claude artifacts** — each pattern is a
pair of (a) a `SKILL.md` Claude can invoke and (b) a `UserPromptSubmit`
hook script that detects the relevant language and nudges Claude to
invoke the skill. Hooks give deterministic triggering; the skill body is
the content Claude reads once invoked.

No orchestrator runtime, no new MCP tools. The app's management UI is
deferred to Phase 1.2 — initial install is manual (`cp` + a
`settings.json` entry) so we can validate behavior on real bugs before
investing in UI.

## Goals

- Author the three patterns as concrete artifacts checked into this repo.
- Let the user install/uninstall them into `~/.claude/` from the app.
- Track which version is installed so updates surface as "reinstall".
- Allow per-conversation injection of selected patterns into the in-app
  Claude advisor's system prompt (lighter weight than a global install).

## Phasing within Phase 1

- **1.0 (today):** Ship `plan-before-patch` skill + matching
  `UserPromptSubmit` hook. Manual install. Validate against a real bug.
- **1.1:** Add `restore-first` (skill + hook) and `parallel-repo-explorer`
  (skill only — invoked deliberately, no language trigger needed).
- **1.2:** Build the app's "Claude behaviors" settings panel, Tauri
  install/uninstall commands, and AICommandAside per-conversation
  injection. Replaces the manual `cp` workflow.

## Non-Goals (Phase 1)

- No Axum HTTP server, no `/sessions` route, no orchestrator-runtime
  observability of hook invocations.
- No new MCP tools (`report_progress`, etc. — orchestrator spec, separate
  work).
- No compliance/observability tracking of whether sessions actually
  followed the behaviors after the hook fired.
- No headless `claude -p` workflows, no `/repro-and-fix` slash command.
- No project-scoped artifact installs (user-level only).

## Artifacts

All artifacts live under `docs/superpowers/claude-artifacts/` in this
repo. Source-of-truth is this repo; the copy under `~/.claude/` is
regenerated on reinstall.

Layout:

```
docs/superpowers/claude-artifacts/
├── skills/
│   ├── plan-before-patch/SKILL.md
│   ├── restore-first/SKILL.md
│   └── parallel-repo-explorer/SKILL.md   # 1.1
├── hooks/
│   ├── plan-before-patch.sh
│   └── restore-first.sh                  # 1.1
└── settings-snippets/
    └── user-prompt-submit-hooks.json     # paste-into-settings.json reference
```

Install targets:

- Skills → `~/.claude/skills/<name>/SKILL.md`
- Hooks → `~/.claude/hooks/<name>.sh` (executable)
- Settings → entries appended to `~/.claude/settings.json`'s
  `hooks.UserPromptSubmit` array

### Hook contract

Each `UserPromptSubmit` hook:

- Reads JSON from stdin: `{"hook_event_name": "UserPromptSubmit",
  "prompt": "...", ...}`.
- Greps the `prompt` field for the pattern's trigger language.
- On match, prints a short nudge (≤ 5 lines) to stdout. Claude Code
  injects stdout as additional context for the turn.
- On no match, exits 0 with empty stdout. Hook is a no-op.
- Never fails the turn — exit 0 always; never blocks user input.

### 1. `skills/plan-before-patch/SKILL.md`

- Trigger surface: bug / failure / error language in user request.
- Required output before any Edit/Write tool call:
  1. observed symptom
  2. 2–3 candidate root causes ranked by likelihood
  3. file:line evidence for the top hypothesis
  4. falsifier — what you'd see if you're wrong
- Then wait for user confirmation.

### 2. `skills/restore-first/SKILL.md`

- Trigger surface: "deleted", "missing", "gone", "removed" + file or
  directory mentioned.
- First action: `git log --all --oneline -- <path>`.
- Second action: `git show <sha>:<path>` once a candidate sha is found.
- Only regenerate from scratch if the user explicitly says nothing is in
  history.

### 3. `agents/parallel-repo-explorer.md`

- Agent profile for cross-repo fan-out.
- Spawn one agent per repo. Each returns a fixed report:
  - relevant files
  - current behavior
  - integration points
- Synthesize before proposing changes.

## App Surface

### Settings panel — "Claude behaviors"

New section in `/settings`. For each artifact:

- Name + one-line description.
- Source version (hash or mtime of the repo file).
- Installed version (hash of the file at `~/.claude/...`, or "not
  installed").
- Status badge: `installed` / `update available` / `not installed` /
  `modified locally` (installed file diverged from any known source
  version — install button changes to "Overwrite").
- Buttons: **Install** / **Reinstall** / **Uninstall** / **View source**.

No CRUD, no editor — these are read-only from the app's perspective.
Edits happen in the repo, then "Reinstall" picks them up.

### AICommandAside — per-conversation injection

In `src/lib/components/AICommandAside.svelte`, add a small multi-select
("Inject behaviors") listing the three artifact names. Selected artifact
bodies are prepended to the system prompt sent on the next message in
that conversation. Selection resets when the route's context key
changes, like the existing messages reset.

This does NOT touch `~/.claude/`. It's purely in-conversation.

## Backend

One Rust module: `src-tauri/src/commands/claude_artifacts.rs`.

Tauri commands:

- `list_claude_artifacts() -> Vec<ArtifactStatus>`
  Reads `docs/superpowers/claude-artifacts/` (bundled or resolved via a
  configured repo path setting), hashes each file, compares to the
  installed file under `~/.claude/`. Returns name, kind, source hash,
  installed hash, status.
- `install_claude_artifact(name: String) -> ArtifactStatus`
  Copies repo file to the right `~/.claude/` subpath. Refuses if
  `modified locally` unless `force: bool` is passed.
- `uninstall_claude_artifact(name: String) -> ArtifactStatus`
  Deletes the file under `~/.claude/`. Leaves the directory.
- `read_claude_artifact_source(name: String) -> String`
  Returns the source markdown for the "View source" link.

A single struct describes an artifact:

```rust
struct Artifact {
    name: &'static str,    // e.g. "plan-before-patch"
    kind: ArtifactKind,    // Skill | Agent
    source_path: PathBuf,  // in repo
    install_path: PathBuf, // under ~/.claude/
}
```

The list is a compile-time constant — Phase 1 has exactly three.

## Storage / Settings

One new optional setting: `claude_artifacts_repo_path`. Defaults to the
app's own install directory's `docs/superpowers/claude-artifacts/`.
Lets the user point at a working copy if they want to iterate on the
artifacts.

No DB migration required.

## Error Handling

- `~/.claude/skills/<name>/` doesn't exist → create on install.
- Source path missing → return error to UI ("Artifact source not
  found").
- File hash mismatch on uninstall (user hand-edited) → soft-warn, still
  delete.
- Permission errors → propagate; UI shows the OS error string.

## Testing

- Unit: hash comparison logic, status derivation, path resolution.
- Manual smoke: install all three, open a fresh Claude Code session in a
  scratch repo, confirm each triggers on its intended phrasing.

## Out of Scope (becomes Phase 2/3)

- **Phase 2 — Workflow templates:** launch Claude with a chosen
  scaffolding (PR-fix loop, multi-repo parallel feature, repro-and-fix).
- **Phase 3 — Coached orchestration:** orchestrator runtime observes
  sessions and nudges via the inbox when behaviors are skipped. Depends
  on the orchestrator spec landing first.

## Open Questions

None blocking. Open items for later:

- Should artifacts be versioned (e.g., a `version:` field in each
  SKILL.md frontmatter) instead of relying on file hashes? Defer until
  we have more than three.
- Should the in-conversation injection also be available as a default
  per agent profile? Defer until the advisor pane gets more
  per-profile config.
