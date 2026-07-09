#!/usr/bin/env bash
# UserPromptSubmit hook: detect "deleted/missing/gone" language paired with
# a file-or-directory reference and nudge Claude to invoke restore-first.

set -euo pipefail

payload="$(cat)"

if command -v jq >/dev/null 2>&1; then
  prompt="$(printf '%s' "$payload" | jq -r '.prompt // ""')"
else
  prompt="$(printf '%s' "$payload" | python3 -c 'import json,sys; print(json.loads(sys.stdin.read()).get("prompt",""))' 2>/dev/null || printf '')"
fi

lc="$(printf '%s' "$prompt" | tr '[:upper:]' '[:lower:]')"

# Loss-language: needs to imply something existed and no longer does.
loss_pattern='\b(deleted|removed|missing|gone|lost|disappeared|wiped|nuked|accidentally (deleted|removed|rm)|where (did|is) [a-z0-9_./[:space:]-]{1,40} go|used to (have|be) [a-z0-9_./[:space:]-]{1,40}|can'\''t find [a-z0-9_./[:space:]-]{1,40})\b'

# Path-ish or directory-ish token nearby. We accept a slash-bearing path,
# a dotted-extension filename, or a bare ALL_CAPS / kebab dir name.
path_pattern='([a-z0-9_-]+/[a-z0-9_./-]+|[a-z0-9_-]+\.(rb|ts|tsx|js|jsx|py|go|rs|java|kt|cs|sh|sql|yml|yaml|json|md|toml|lock|conf|env)|\b(readme|changelog|license|dockerfile|makefile|gemfile|rakefile|procfile)\b|\b[a-z0-9_-]+(-[a-z0-9_-]+)*[[:space:]]+(folder|directory|module|component|page|route|file|script|migration|migrations|config|settings|spec|test|tests))'

if printf '%s' "$lc" | grep -Eq "$loss_pattern" && printf '%s' "$lc" | grep -Eq "$path_pattern"; then
  cat <<'EOF'
[restore-first hook] This prompt mentions a deleted/missing file or
directory. Before regenerating from scratch, invoke the `restore-first`
skill via the Skill tool. First action should be:
  git log --all --oneline -- <path>
to locate when the item existed, then `git show <sha>:<path>` to
recover. Only regenerate if the user confirms nothing is in history.
EOF
fi

exit 0
