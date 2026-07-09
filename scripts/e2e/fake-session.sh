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

# --- Inbox push-deliver round-trip ---
# Queue a message for the (now-stopped) session — we can still post to its inbox.
curl -s -X POST "http://127.0.0.1:9876/inbox/$SID" \
  -H 'content-type: application/json' \
  -d '{"from_kind":"human","message":"e2e queued reply"}' > /dev/null

# Fire user-prompt-submit.sh and confirm stdout contains the queued message.
HOOK_OUT="$(printf '%s' "{\"hook_event_name\":\"UserPromptSubmit\",\"session_id\":\"$SID\",\"cwd\":\"/tmp/e2e\",\"prompt\":\"continue\"}" \
  | bash "$HOOKS/user-prompt-submit.sh")"
echo "$HOOK_OUT" | grep -q "e2e queued reply" \
  || { echo "FAIL: push-deliver did not inject queued message"; echo "got: $HOOK_OUT"; exit 1; }

# Verify ack happened — peek should now return empty messages array.
PEEK="$(curl -s "http://127.0.0.1:9876/inbox/$SID?peek=1")"
COUNT=$(printf '%s' "$PEEK" | python3 -c "import json,sys; print(len(json.loads(sys.stdin.read()).get('messages',[])))")
[ "$COUNT" = "0" ] || { echo "FAIL: ack did not drain (peek count=$COUNT, expected 0)"; exit 1; }

# --- Artifact link round-trip ---
curl -s -X POST "http://127.0.0.1:9876/sessions/$SID/link" \
  -H 'content-type: application/json' \
  -d '{"kind":"project","id":"42","title":"e2e project","url":"https://example.com/p/42"}' > /dev/null

LINKED_KIND=$(curl -s http://127.0.0.1:9876/sessions \
  | python3 -c "import json,sys; d=json.loads(sys.stdin.read()); print(next((s.get('artifact_kind','') for s in d['sessions'] if s['id']=='$SID'),''))")
[ "$LINKED_KIND" = "project" ] || { echo "FAIL: link did not persist (kind=$LINKED_KIND)"; exit 1; }

curl -s -X DELETE "http://127.0.0.1:9876/sessions/$SID/link" > /dev/null

LINKED_KIND=$(curl -s http://127.0.0.1:9876/sessions \
  | python3 -c "import json,sys; d=json.loads(sys.stdin.read()); print(next((s.get('artifact_kind') or '' for s in d['sessions'] if s['id']=='$SID'),''))")
[ -z "$LINKED_KIND" ] || { echo "FAIL: unlink did not clear (kind=$LINKED_KIND)"; exit 1; }

echo "OK push-deliver + link round-trip"
