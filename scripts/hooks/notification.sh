#!/usr/bin/env bash
set -euo pipefail
source "$(dirname "$0")/_common.sh"
read_stdin
SID="$(hook_field session_id)"
MSG="$(hook_field message)"
post_event "$(build_json kind=notification session_id="$SID" message="$MSG")"
