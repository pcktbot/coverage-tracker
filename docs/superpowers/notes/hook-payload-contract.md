# Claude Code hook payload contract (captured 2026-05-11)

Captured by `scripts/hooks/payload-capture.sh` against Claude Code running
Opus 4.7. All hooks receive a single JSON object on stdin. All fields below
were observed at least once; types are inferred from the values seen.

## Fields common to every event

- `hook_event_name`: string — `SessionStart` | `UserPromptSubmit` |
  `PreToolUse` | `Notification` | `Stop`
- `session_id`: string — UUID-shaped
- `transcript_path`: string — path to `~/.claude/projects/<slug>/<sid>.jsonl`
- `cwd`: string

## `SessionStart`

- `source`: string — observed `startup`. Likely also `resume`, `clear`.
- `model`: string — observed `claude-opus-4-7[1m]`.

## `UserPromptSubmit`

- `prompt`: string — full user message
- `permission_mode`: string — observed `default`, `auto`

## `PreToolUse`

- `tool_name`: string — `Bash`, `Read`, `Edit`, `Agent`, etc.
- `tool_input`: object — tool-specific arguments (e.g., `{command, run_in_background, ...}` for Bash)
- `tool_use_id`: string — `toolu_...`
- `permission_mode`: string
- When the tool is `Agent`, also: `agent_id`, `agent_type`

## `Notification`

- `message`: string — observed `"Claude is waiting for your input"`
- `notification_type`: string — observed `idle_prompt`

## `Stop`

- `stop_hook_active`: bool — whether the Stop hook itself triggered the stop
- `last_assistant_message`: string — text of the final assistant turn
- `permission_mode`: string
- `effort`: object — `{level: "medium" | ...}`
- **No `stop_reason`, `reason`, or `error` field.** The Stop hook appears to
  fire only on successful session end. Failures don't appear to surface here.

## Implications for hook scripts (Task 8 plan adjustment)

- `stop.sh`: do NOT try to derive `error` from a `stop_reason` field — it
  doesn't exist. Post `kind=stop session_id=$SID reason=""` (the
  `EventBody::Stop` deserializer already treats both `error` and `reason` as
  `Option`, so omitting them is fine). Until Claude exposes error info via a
  different mechanism, every recorded Stop will become status `done`. The
  status state machine handles `Stop { error: false }` correctly.
- `pre-tool-use.sh`: payload key is `tool_name` (matches plan).
- `notification.sh`: payload key is `message` (matches plan).
- `user-prompt-submit.sh`: payload key is `prompt` (matches plan).
- `session-start.sh`: payload keys `session_id` and `cwd` (match plan).
- Bonus: every event includes `cwd` — could be used as a fallback labeling
  source if the SessionStart event was missed. Not required for v1.

## Future improvements (out of scope for Phase 1)

- Capture `tool_input` summary on `PreToolUse` to make the timeline richer
  (e.g., show the actual Bash command, not just `Bash`).
- Use `transcript_path` as an artifact attachment when a session ends.
- Use `SessionStart.source = "resume"` to distinguish fresh vs. resumed
  sessions in the UI.
