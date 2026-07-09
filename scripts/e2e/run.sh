#!/usr/bin/env bash
set -euo pipefail
if ! curl -s --max-time 1 http://127.0.0.1:9876/sessions > /dev/null; then
  echo "Orchestrator not reachable on :9876. Start the Tauri app first." >&2
  exit 1
fi
bash "$(dirname "$0")/fake-session.sh"

# Assert loaded_snapshot field is exposed on /sessions response (schema v3)
SESSIONS_RESP="$(curl -s http://127.0.0.1:9876/sessions)"
echo "$SESSIONS_RESP" | python3 -c "
import json, sys
d = json.load(sys.stdin)
assert d['sessions'], 'no sessions on /sessions response'
keys = set(d['sessions'][0].keys())
assert 'loaded_snapshot' in keys, f'loaded_snapshot missing; got: {sorted(keys)}'
print('ok: loaded_snapshot present on /sessions response')
"
