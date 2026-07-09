#!/usr/bin/env bash
set -euo pipefail
source "$(dirname "$0")/_common.sh"
read_stdin
SID="$(hook_field session_id)"
TRANSCRIPT="$(hook_field transcript_path)"
# Claude Code's Stop event has no error/reason field as of capture 2026-05-11.
# We post a stop event with empty reason; status state machine treats this
# as `done`. If Claude later exposes an error mechanism, plumb it here.
EXTRA=""
[ -n "$TRANSCRIPT" ] && EXTRA="transcript_path=$TRANSCRIPT"
post_event "$(build_json kind=stop session_id="$SID" reason="" $EXTRA)"
