#!/usr/bin/env bash
# UserPromptSubmit hook: detect bug/defect language in the user prompt and
# nudge Claude to invoke the plan-before-patch skill before editing.
#
# Contract: stdin = JSON with a "prompt" field. stdout = additional context
# injected into the turn (empty if no match). Exit 0 always.

set -euo pipefail

payload="$(cat)"

# Extract prompt field. Prefer jq if available; fall back to a minimal
# python parser to avoid a hard dependency.
if command -v jq >/dev/null 2>&1; then
  prompt="$(printf '%s' "$payload" | jq -r '.prompt // ""')"
else
  prompt="$(printf '%s' "$payload" | python3 -c 'import json,sys; print(json.loads(sys.stdin.read()).get("prompt",""))' 2>/dev/null || printf '')"
fi

# Lowercase for case-insensitive matching.
lc="$(printf '%s' "$prompt" | tr '[:upper:]' '[:lower:]')"

# Trigger patterns. Tuned to be broad enough to catch most bug reports
# without firing on every mention of the word "test".
pattern='\b(bug|broken|crashes?|crashed|crashing|regression|stack ?trace|traceback|exception|stopped working|not working|doesn'\''t work|does not work|isn'\''t working|is not working|failing|fails to|throws? an error|throwing an error|undefined|null reference|nullpointer|segfault|500 error|404|wrong (output|result|behavior|behaviour)|why (is|does|are|am i))\b'

# Also catch "fix <something>" when paired with a path-ish or function-ish token.
fix_pattern='\bfix(es|ed|ing)?\b.*\b([a-z_]+\.(rb|ts|tsx|js|jsx|py|go|rs|java|kt|cs|sh|sql|yml|yaml|json)|[a-z_][a-z0-9_]*\()'

if printf '%s' "$lc" | grep -Eq "$pattern" || printf '%s' "$lc" | grep -Eq "$fix_pattern"; then
  cat <<'EOF'
[plan-before-patch hook] This prompt looks like a bug report or fix
request. Before invoking Edit/Write, invoke the `plan-before-patch`
skill via the Skill tool and follow its four-section diagnosis output
(symptom, ranked causes, evidence, falsifier). Wait for the user to
confirm the top hypothesis before editing code.
EOF
fi

exit 0
