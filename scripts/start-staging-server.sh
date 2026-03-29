#!/bin/bash
# start-staging-server.sh — Start HTTP staging server with explicit directory
# MMA consensus fix: 7/9 models agreed --directory flag prevents wrong-CWD bug
#
# Usage:
#   bash scripts/start-staging-server.sh                           # Default: deploy-staging on :18889
#   bash scripts/start-staging-server.sh /path/to/dir 19000        # Custom dir + port
#   bash scripts/start-staging-server.sh --verify rc-agent-abc.exe # Start + verify specific file
#
# Features:
#   - Uses --directory flag (no cd dependency)
#   - Kills any existing server on the port first
#   - Smoke-tests that a known binary is > 1MB (catches 404 HTML pages)
#   - Reports actual served URL for copy-paste

set -euo pipefail

STAGING_DIR="${1:-$(cd "$(dirname "$0")/../deploy-staging" && pwd)}"
PORT="${2:-18889}"
VERIFY_FILE=""

# Parse --verify flag
for arg in "$@"; do
  if [[ "$arg" == "--verify" ]]; then
    VERIFY_FILE="${@: -1}"  # Last argument after --verify
  fi
done

# Resolve to absolute path
STAGING_DIR="$(cd "$STAGING_DIR" 2>/dev/null && pwd || echo "$STAGING_DIR")"

echo "=== Staging Server ==="
echo "Directory: ${STAGING_DIR}"
echo "Port: ${PORT}"

# Verify directory exists and has files
if [ ! -d "$STAGING_DIR" ]; then
  echo "ERROR: Directory does not exist: $STAGING_DIR"
  exit 1
fi

FILE_COUNT=$(ls -1 "$STAGING_DIR"/*.exe 2>/dev/null | wc -l)
echo "Binary files in staging: ${FILE_COUNT}"

# Kill any existing server on this port
pkill -f "http.server ${PORT}" 2>/dev/null || true
sleep 1

# Start server with --directory (Python 3.7+, no cd needed)
python -m http.server "$PORT" --directory "$STAGING_DIR" --bind 0.0.0.0 &
SERVER_PID=$!
sleep 2

# Verify server is running
if ! kill -0 "$SERVER_PID" 2>/dev/null; then
  echo "ERROR: HTTP server failed to start"
  exit 1
fi

echo "Server PID: ${SERVER_PID}"

# Smoke test: verify directory listing loads
HTTP_CODE=$(curl -s -o /dev/null -w "%{http_code}" "http://localhost:${PORT}/" 2>/dev/null)
if [ "$HTTP_CODE" != "200" ]; then
  echo "ERROR: Server not responding (HTTP ${HTTP_CODE})"
  kill "$SERVER_PID" 2>/dev/null
  exit 1
fi

# If --verify specified, check that specific file is serveable and > 1MB
if [ -n "$VERIFY_FILE" ]; then
  ACTUAL_SIZE=$(curl -sI "http://localhost:${PORT}/${VERIFY_FILE}" 2>/dev/null | \
    grep -i content-length | awk '{print $2}' | tr -d '\r')

  if [ -z "$ACTUAL_SIZE" ] || [ "$ACTUAL_SIZE" -lt 1000000 ] 2>/dev/null; then
    echo "ERROR: File '${VERIFY_FILE}' not available or too small (${ACTUAL_SIZE:-0} bytes)"
    echo "This likely means the server is serving from the wrong directory!"
    kill "$SERVER_PID" 2>/dev/null
    exit 1
  fi
  echo "Verified: ${VERIFY_FILE} = ${ACTUAL_SIZE} bytes (OK)"
fi

echo ""
echo "Staging server ready at http://192.168.31.27:${PORT}/"
echo "To stop: kill ${SERVER_PID}"
