#!/bin/bash
set -euo pipefail
# scripts/check-gen-types-determinism.sh
# Phase 445 Wave 0 Task 2 Step A — determinism harness for the gen-types binary.
#
# Purpose (RESEARCH.md § "Pitfall 6"): utoipa uses BTreeMap internally for
# OpenAPI components, but route-registration order must also be deterministic
# for CI's `git diff --exit-code` gate to be meaningful. Intermittent CI
# failures from HashMap iteration order would erode trust in the gate.
#
# Strategy: run gen-types 3× back-to-back, sha256 the union of generated TS
# files + openapi.generated.yaml, assert all 3 hashes are byte-identical.
#
# Pre-Plan-01 behavior: gen-types binary does not yet exist. Exit 0 with
# SKIP message so this harness can be wired into CI/pre-commit BEFORE
# Plan 01 lands the binary. Once Plan 01 ships, this script arms automatically.
#
# Usage:
#   bash scripts/check-gen-types-determinism.sh
#
# Exit codes:
#   0 = deterministic across 3 runs (or pre-Plan-01 skip)
#   1 = non-deterministic (drift between runs)
#   2 = build failure

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$REPO_ROOT"

BIN_WIN="target/release/gen-types.exe"
BIN_NIX="target/release/gen-types"

if [ ! -f "$BIN_WIN" ] && [ ! -f "$BIN_NIX" ]; then
    echo "SKIP: gen-types binary not built yet (Plan 01 prerequisite)"
    echo "      Once Plan 01 lands, build with:"
    echo "      cargo build --release --bin gen-types --features gen-types"
    exit 0
fi

run_and_hash() {
    # Produce a single sha256 over the sorted union of generated files.
    cargo run --release --bin gen-types --features gen-types --quiet >/dev/null 2>&1 || return 2
    {
        find packages/shared-types/generated -type f -name '*.ts' 2>/dev/null \
            | sort \
            | xargs sha256sum 2>/dev/null
        sha256sum docs/openapi.generated.yaml 2>/dev/null
    } | sha256sum | awk '{print $1}'
}

HASH1=$(run_and_hash) || { echo "FAIL: gen-types build/run failed on run 1"; exit 2; }
HASH2=$(run_and_hash) || { echo "FAIL: gen-types build/run failed on run 2"; exit 2; }
HASH3=$(run_and_hash) || { echo "FAIL: gen-types build/run failed on run 3"; exit 2; }

if [ "$HASH1" = "$HASH2" ] && [ "$HASH2" = "$HASH3" ]; then
    echo "DETERMINISTIC: $HASH1"
    exit 0
else
    echo "NON-DETERMINISTIC: hash1=$HASH1 hash2=$HASH2 hash3=$HASH3"
    echo "See RESEARCH.md § 'Pitfall 6' for common root causes (HashMap iteration)."
    exit 1
fi
