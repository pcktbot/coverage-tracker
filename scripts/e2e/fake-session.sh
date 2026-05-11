#!/usr/bin/env bash
# Simulates a Claude session lifecycle by invoking the installed hook
# scripts with fabricated payloads, then asserts the orchestrator's session
# row progresses through the expected statuses.
set -euo pipefail

HOOKS="$(cd "$(dirname "$0")/../hooks" && pwd)"
SID="e2e-$(date +%s)-$$"

emit() {
  printf '%s' "$2" | "$HOOKS/$1"
  sleep 0.2
}

status_for() {
  curl -s http://127.0.0.1:9876/sessions \
    | python3 -c "import json,sys; d=json.loads(sys.stdin.read()); print(next((s['status'] for s in d['sessions'] if s['id']=='$1'),'absent'))"
}

emit session-start.sh      "{\"hook_event_name\":\"SessionStart\",\"session_id\":\"$SID\",\"cwd\":\"/tmp/e2e\"}"
emit user-prompt-submit.sh "{\"hook_event_name\":\"UserPromptSubmit\",\"session_id\":\"$SID\",\"prompt\":\"hello\"}"
emit pre-tool-use.sh       "{\"hook_event_name\":\"PreToolUse\",\"session_id\":\"$SID\",\"tool_name\":\"Bash\"}"
emit notification.sh       "{\"hook_event_name\":\"Notification\",\"session_id\":\"$SID\",\"message\":\"confirm?\"}"

STATUS="$(status_for "$SID")"
[ "$STATUS" = "needs_input" ] || { echo "FAIL: expected needs_input, got $STATUS"; exit 1; }

emit stop.sh "{\"hook_event_name\":\"Stop\",\"session_id\":\"$SID\",\"stop_hook_active\":false,\"last_assistant_message\":\"bye\"}"

STATUS="$(status_for "$SID")"
[ "$STATUS" = "done" ] || { echo "FAIL: expected done, got $STATUS"; exit 1; }

echo "OK ($SID)"
