#!/bin/bash
# tests/fleet-probe/smoke-james.sh -- Phase 448 Plan 02
# Live smoke test: runs probe-james.sh in a temp manifest dir, validates output.
# Must exit 0 on a healthy James localhost.
# Usage: bash tests/fleet-probe/smoke-james.sh
set -eo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$REPO_ROOT"

SMOKE_TS="smoke-$(date +%s)"
export MANIFEST_TS="$SMOKE_TS"
mkdir -p "state/fleet-manifest/$SMOKE_TS"

echo "--- smoke-james: running probe-james.sh (MANIFEST_TS=$SMOKE_TS) ---"
STATUS=$(bash scripts/fleet-probe/probe-james.sh | tail -1)
echo "status: $STATUS"

MANIFEST_FILE="state/fleet-manifest/$SMOKE_TS/james_27.json"
if [ ! -f "$MANIFEST_FILE" ]; then
  echo "FAIL: manifest not written at $MANIFEST_FILE" >&2
  exit 1
fi

echo "--- smoke-james: manifest written, running ajv validation ---"
node scripts/fleet-probe/validate-manifest-file.mjs "$MANIFEST_FILE"

echo "--- smoke-james: manifest peek (first 20 lines) ---"
head -n 20 "$MANIFEST_FILE"

echo "--- smoke-james: cleaning up ---"
rm -rf "state/fleet-manifest/$SMOKE_TS"

echo "smoke-james OK"
