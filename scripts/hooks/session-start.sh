#!/usr/bin/env bash
set -euo pipefail
source "$(dirname "$0")/_common.sh"
read_stdin
SID="$(hook_field session_id)"
CWD="$(hook_field cwd)"
LABEL="$(basename "$CWD")"
PID="$PPID"
post_event "$(build_json kind=session_start session_id="$SID" cwd="$CWD" pid:int="$PID" label="$LABEL")"
