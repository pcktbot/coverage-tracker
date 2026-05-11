#!/usr/bin/env bash
set -euo pipefail
source "$(dirname "$0")/_common.sh"
read_stdin
SID="$(hook_field session_id)"
PROMPT="$(printf '%s' "$HOOK_INPUT" | python3 -c "import json,sys; d=json.loads(sys.stdin.read()); print(d.get('prompt','')[:500])" 2>/dev/null || printf '')"
post_event "$(build_json kind=user_prompt_submit session_id="$SID" prompt="$PROMPT")"
