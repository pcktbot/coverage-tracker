#!/usr/bin/env bash
set -euo pipefail
source "$(dirname "$0")/_common.sh"
read_stdin
SID="$(hook_field session_id)"
TRANSCRIPT="$(hook_field transcript_path)"
MSG="$(hook_field message)"
EXTRA=""
[ -n "$TRANSCRIPT" ] && EXTRA="transcript_path=$TRANSCRIPT"
post_event "$(build_json kind=notification session_id="$SID" message="$MSG" $EXTRA)"
