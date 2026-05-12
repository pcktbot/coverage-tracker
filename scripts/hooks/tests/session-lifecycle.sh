#!/usr/bin/env bash
set -euo pipefail
HOOKS="$(cd "$(dirname "$0")/.." && pwd)"
REC=/tmp/fake-hook-recording
"$HOOKS/tests/fake-server.sh" 9876 "$REC" &
PID=$!
trap "kill $PID 2>/dev/null || true" EXIT
sleep 0.3

# Use the field names confirmed in Task 7 capture.
printf '%s' '{"hook_event_name":"SessionStart","session_id":"s-test","cwd":"/tmp/work","source":"startup","model":"claude-opus-4-7"}' \
  | "$HOOKS/session-start.sh"
sleep 0.2
grep -q '"kind":"session_start"' "$REC" || { echo "FAIL: session_start kind"; cat "$REC"; exit 1; }
grep -q '"session_id":"s-test"' "$REC" || { echo "FAIL: session_id"; exit 1; }
grep -q '"cwd":"/tmp/work"' "$REC" || { echo "FAIL: cwd"; exit 1; }
grep -qE '"pid":[0-9]+' "$REC" || { echo "FAIL: pid not numeric"; cat "$REC"; exit 1; }

printf '%s' '{"hook_event_name":"UserPromptSubmit","session_id":"s-test","cwd":"/tmp/work","prompt":"hello there"}' \
  | "$HOOKS/user-prompt-submit.sh"
sleep 0.2
grep -q '"kind":"user_prompt_submit"' "$REC" || { echo "FAIL: user_prompt_submit kind"; exit 1; }
grep -q '"prompt":"hello there"' "$REC" || { echo "FAIL: prompt"; exit 1; }

printf '%s' '{"hook_event_name":"Stop","session_id":"s-test","cwd":"/tmp/work","stop_hook_active":false,"last_assistant_message":"goodbye"}' \
  | "$HOOKS/stop.sh"
sleep 0.2
grep -q '"kind":"stop"' "$REC" || { echo "FAIL: stop kind"; exit 1; }
grep -q '"session_id":"s-test"' "$REC" || { echo "FAIL: stop session_id"; exit 1; }

printf '%s' '{"hook_event_name":"PreToolUse","session_id":"s-test","cwd":"/tmp/work","tool_name":"Bash","tool_input":{"command":"ls"}}' \
  | "$HOOKS/pre-tool-use.sh"
sleep 0.2
grep -q '"kind":"pre_tool_use"' "$REC" || { echo "FAIL: pre_tool_use kind"; exit 1; }
grep -q '"tool":"Bash"' "$REC" || { echo "FAIL: tool"; exit 1; }

printf '%s' '{"hook_event_name":"Notification","session_id":"s-test","cwd":"/tmp/work","message":"awaiting input","notification_type":"idle_prompt"}' \
  | "$HOOKS/notification.sh"
sleep 0.2
grep -q '"kind":"notification"' "$REC" || { echo "FAIL: notification kind"; exit 1; }
grep -q '"message":"awaiting input"' "$REC" || { echo "FAIL: message"; exit 1; }

printf '%s' '{"hook_event_name":"SessionStart","session_id":"s-test-2","cwd":"/tmp/work","transcript_path":"/tmp/captured.jsonl"}' \
  | "$HOOKS/session-start.sh"
sleep 0.2
grep -q '"transcript_path":"/tmp/captured.jsonl"' "$REC" || { echo "FAIL: transcript_path not posted"; cat "$REC"; exit 1; }

echo "OK"
