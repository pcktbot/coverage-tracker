#!/usr/bin/env bash
set -euo pipefail
source "$(dirname "$0")/_common.sh"
read_stdin
SID="$(hook_field session_id)"
PROMPT="$(printf '%s' "$HOOK_INPUT" | python3 -c "import json,sys; print(json.loads(sys.stdin.read()).get('prompt','')[:500])" 2>/dev/null || printf '')"
TRANSCRIPT="$(hook_field transcript_path)"

# Peek + print + ack the inbox. Graceful degradation if orchestrator unreachable.
INBOX_JSON="$(curl --silent --max-time 1 "$ORCH_URL/inbox/$SID?peek=1" 2>/dev/null || true)"
if [ -z "$INBOX_JSON" ]; then
    echo "[orchestrator inbox unreachable]" >&2
else
    # Process inbox: produce two outputs — context block (stdout) and ID list (file).
    IDS_FILE="$(mktemp)"
    printf '%s' "$INBOX_JSON" | python3 - "$IDS_FILE" <<'PY' 2>/dev/null || true
import json, sys
ids_path = sys.argv[1]
try:
    d = json.loads(sys.stdin.read())
    msgs = d.get("messages", [])
    if not msgs:
        sys.exit(0)
    print(f"[Inbox ({len(msgs)} queued message(s) from orchestrator)]")
    for m in msgs:
        from_k = m.get("from_kind", "?")
        ts = m.get("ts", 0)
        text = m.get("message", "")
        print(f"— [{from_k} @ {ts}] {text}")
    ids = [str(m.get("id")) for m in msgs if m.get("id") is not None]
    with open(ids_path, "w") as f:
        f.write(",".join(ids))
except Exception:
    pass
PY
    IDS_TO_ACK="$(cat "$IDS_FILE" 2>/dev/null || printf '')"
    rm -f "$IDS_FILE"
    if [ -n "$IDS_TO_ACK" ]; then
        curl --silent --output /dev/null --max-time 1 \
            -X POST -H 'content-type: application/json' \
            --data "{\"ids\":[$IDS_TO_ACK]}" \
            "$ORCH_URL/inbox/$SID/ack" 2>/dev/null || true
    fi
fi

EXTRA=""
[ -n "$TRANSCRIPT" ] && EXTRA="transcript_path=$TRANSCRIPT"
post_event "$(build_json kind=user_prompt_submit session_id="$SID" prompt="$PROMPT" $EXTRA)"
