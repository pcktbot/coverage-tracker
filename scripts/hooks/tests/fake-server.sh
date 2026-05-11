#!/usr/bin/env bash
set -euo pipefail
PORT="${1:-9876}"
REC="${2:-/tmp/fake-hook-recording}"
: > "$REC"
while true; do
  { printf 'HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok'; } | nc -l "$PORT" >> "$REC" 2>/dev/null || true
done
