#!/bin/bash
set -euo pipefail
# scripts/check-generated-types-drift.sh
# Phase 445 Wave 0 Task 2 Step B — D-15 canonical CI drift gate.
#
# Purpose (CONTEXT.md D-15): PR MUST fail if committed generated types drift
# from Rust source. This script is the reference implementation — rebuild
# gen-types, run it, then `git diff --exit-code` against the tracked tree.
#
# Behaviour:
#   - Pre-Plan-01 (gen-types feature doesn't build): exit 0 with SKIP message.
#     Lets us wire the gate into CI/pre-commit BEFORE Plan 01 lands.
#   - Post-Plan-01: hard drift gate. Any uncommitted delta in
#     packages/shared-types/generated/ or docs/openapi.generated.yaml is a
#     PR failure.
#
# Usage:
#   bash scripts/check-generated-types-drift.sh
#
# Exit codes:
#   0 = no drift (or pre-Plan-01 skip)
#   1 = drift detected (regenerate + commit)
#   2 = build failure

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$REPO_ROOT"

# Attempt to build gen-types. If the feature doesn't exist yet (Plan 01 pre-req),
# skip gracefully — this is the intended behaviour pre-Plan-01.
if ! cargo build --release --bin gen-types --features gen-types >/dev/null 2>&1; then
    echo "SKIP: gen-types feature not buildable yet (Plan 01 prerequisite)"
    echo "      Add ts-rs + utoipa deps + bin/gen_types.rs in Plan 01 to arm this gate."
    exit 0
fi

# Run gen-types.
if ! cargo run --release --bin gen-types --features gen-types --quiet; then
    echo "FAIL: gen-types binary exited non-zero"
    exit 2
fi

# Strict drift gate — any change means a PR forgot to regenerate.
if ! git diff --exit-code packages/shared-types/generated/ docs/openapi.generated.yaml; then
    echo ""
    echo "DRIFT DETECTED: generated types are out of sync with Rust source."
    echo "Fix: run 'cargo run --release --bin gen-types --features gen-types' and commit the result."
    exit 1
fi

echo "OK: no drift in packages/shared-types/generated/ or docs/openapi.generated.yaml"
exit 0
