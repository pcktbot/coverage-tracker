#!/usr/bin/env bash
set -euo pipefail
source "$(dirname "$0")/_common.sh"
read_stdin
SID="$(hook_field session_id)"
TOOL="$(hook_field tool_name)"
post_event "$(build_json kind=pre_tool_use session_id="$SID" tool="$TOOL")"
