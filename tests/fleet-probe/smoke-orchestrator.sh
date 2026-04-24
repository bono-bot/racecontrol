#!/bin/bash
# tests/fleet-probe/smoke-orchestrator.sh -- Phase 448 Plan 07
# Integration smoke test for probe-all.sh.
# Verifies: --dry-run enumeration + --help + --canary end-to-end (mock network).
# Must exit 0 on a clean repo checkout.
# Usage: bash tests/fleet-probe/smoke-orchestrator.sh
set -eo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$REPO_ROOT"

PYTHON_CMD="${PROBE_PYTHON:-$(command -v python 2>/dev/null || echo python3)}"
if [ "$(uname -s 2>/dev/null || true)" != "Linux" ]; then
  # On Windows Git Bash, prefer 'python' (real Python 3.12) over 'python3' (Store stub)
  PYTHON_CMD="${PROBE_PYTHON:-python}"
fi
export PROBE_PYTHON="$PYTHON_CMD"

echo "-- smoke 1: --dry-run enumerates 15 targets --"
LINES=$(bash scripts/fleet-probe/probe-all.sh --dry-run | wc -l | tr -d ' ')
if [ "$LINES" != "15" ]; then
  echo "FAIL: dry-run emitted $LINES lines (expected 15)" >&2
  exit 1
fi
echo "  ok: 15 targets enumerated"

echo "-- smoke 2: --help prints Usage: --"
bash scripts/fleet-probe/probe-all.sh --help | grep -q "Usage:" || {
  echo "FAIL: --help output did not contain 'Usage:'" >&2
  exit 1
}
echo "  ok: --help prints usage"

echo "-- smoke 3: --canary produces manifest set with _meta.json --"
# Use a deterministic MANIFEST_TS so we can clean up afterwards.
SMOKE_TS="smoke-canary-$(date +%s)"
export MANIFEST_TS="$SMOKE_TS"

# Redirect probe-server.sh SSH to the mock-ssh-responder.sh with a timeout scenario.
# This avoids any real Tailscale SSH attempt.
export PROBE_SSH="$REPO_ROOT/tests/fleet-probe/mock-ssh-responder.sh"
export PROBE_SSH_SCENARIO="$REPO_ROOT/tests/fleet-probe/fixtures/server-ssh-timeout.txt"
# Skip HTTP probe on server (curl would also hit unreachable target)
export PROBE_SKIP_HTTP=1

# For probe-pod.sh 8: leave SENTRY_KEY unset so the probe immediately marks probe_failed
# without making any network calls (see probe-pod.sh auth pre-check branch).
unset SENTRY_KEY

# Run canary -- orchestrator must always exit 0 even if all probes return probe_failed
bash scripts/fleet-probe/probe-all.sh --canary

DIR="$REPO_ROOT/state/fleet-manifest/$SMOKE_TS"
if [ ! -d "$DIR" ]; then
  echo "FAIL: manifest dir $DIR was not created" >&2
  exit 1
fi

if [ ! -f "$DIR/_meta.json" ]; then
  echo "FAIL: _meta.json not written in $DIR" >&2
  exit 1
fi

if [ ! -f "$DIR/server_23.json" ]; then
  echo "FAIL: server_23.json not written in $DIR" >&2
  exit 1
fi

if [ ! -f "$DIR/pod_8.json" ]; then
  echo "FAIL: pod_8.json not written in $DIR" >&2
  exit 1
fi

TARGET_COUNT=$(cat "$DIR/_meta.json" | "$PYTHON_CMD" -c "import json,sys; m=json.load(sys.stdin); print(m['target_count'])")
if [ "$TARGET_COUNT" != "2" ]; then
  echo "FAIL: canary _meta.json target_count=$TARGET_COUNT (expected 2)" >&2
  exit 1
fi

# Validate all per-target manifests against the fleet-manifest schema
for f in "$DIR"/*.json; do
  base=$(basename "$f")
  if [ "$base" = "_meta.json" ]; then
    continue
  fi
  node scripts/fleet-probe/validate-manifest-file.mjs "$f" || {
    echo "FAIL: $f failed schema validation" >&2
    exit 1
  }
done
echo "  ok: canary dir has 2 per-target manifests + _meta.json, all schema-valid"

# Verify status_summary exists and contains the required keys
cat "$DIR/_meta.json" | "$PYTHON_CMD" -c "
import json, sys
m = json.load(sys.stdin)
ss = m.get('status_summary', {})
assert 'ok' in ss, 'status_summary missing ok'
assert 'partial' in ss, 'status_summary missing partial'
assert 'probe_failed' in ss, 'status_summary missing probe_failed'
"
echo "  ok: _meta.json status_summary contains ok/partial/probe_failed keys"

# Clean up smoke dir
rm -rf "$DIR"
echo "smoke-orchestrator OK"
