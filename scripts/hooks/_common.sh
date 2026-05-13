#!/usr/bin/env bash
# Sourced by hook scripts. Provides post_event, hook_field, build_json.
ORCH_URL="${ORCHESTRATOR_URL:-http://127.0.0.1:9876}"

read_stdin() { HOOK_INPUT="$(cat)"; }

hook_field() {
  printf '%s' "$HOOK_INPUT" \
    | python3 -c 'import json,sys; d=json.loads(sys.stdin.read()); print(d.get(sys.argv[1],""))' "$1" 2>/dev/null
}

post_event() {
  curl --silent --output /dev/null --max-time 3 \
    -X POST -H 'content-type: application/json' \
    --data "$1" "$ORCH_URL/event" || true
}

# build_json k=v k:int=v k:bool=v ...   — type-suffix produces typed JSON.
build_json() {
  python3 - "$@" <<'PY'
import json, sys
out = {}
for a in sys.argv[1:]:
    k, _, v = a.partition('=')
    if k.endswith(':int'):
        k = k[:-4]
        try: v = int(v)
        except ValueError: v = 0
    elif k.endswith(':bool'):
        k = k[:-5]
        v = v.lower() in ('1','true','yes')
    out[k] = v
print(json.dumps(out, separators=(',', ':')))
PY
}
