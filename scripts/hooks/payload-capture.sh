#!/usr/bin/env bash
# Captures every Claude Code hook invocation to /tmp for inspection.
# Install temporarily by adding this script to every hook array in
# ~/.claude/settings.json (passing the event name as the first arg), run one
# short session, then remove. See docs/superpowers/notes/hook-payload-contract.md
# for the captured field shape.
EVENT="${1:-unknown}"
OUT="/tmp/claude-hook-payloads.jsonl"
TS="$(date -u +%FT%TZ)"
PAYLOAD="$(cat)"
printf '{"ts":"%s","event_arg":"%s","payload":%s}\n' "$TS" "$EVENT" "$PAYLOAD" >> "$OUT"
exit 0
