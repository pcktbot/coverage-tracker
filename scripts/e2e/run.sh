#!/usr/bin/env bash
set -euo pipefail
if ! curl -s --max-time 1 http://127.0.0.1:9876/sessions > /dev/null; then
  echo "Orchestrator not reachable on :9876. Start the Tauri app first." >&2
  exit 1
fi
bash "$(dirname "$0")/fake-session.sh"
