#!/usr/bin/env bash
set -euo pipefail
source "$(dirname "$0")/_common.sh"
read_stdin
SID="$(hook_field session_id)"
TRANSCRIPT="$(hook_field transcript_path)"
CWD="$(hook_field cwd)"
LABEL="$(basename "$CWD")"
PID="$PPID"

# Best-effort snapshot; never block session start on failure.
SNAPSHOT="$("$(dirname "$0")/_loaded_snapshot.py" "$CWD" 2>/dev/null || echo '{}')"

EXTRA=()
[ -n "$TRANSCRIPT" ] && EXTRA+=("transcript_path=$TRANSCRIPT")
[ -n "$SNAPSHOT" ] && EXTRA+=("loaded_snapshot=$SNAPSHOT")

post_event "$(build_json kind=session_start session_id="$SID" cwd="$CWD" pid:int="$PID" label="$LABEL" "${EXTRA[@]}")"
