#!/bin/bash
set -euo pipefail
# scripts/audit-handwritten-vs-generated.sh
# Phase 445 Wave 0 Task 2 Step C — D-12 drift audit tool.
#
# Purpose (CONTEXT.md D-12): for each type being migrated in Wave 3, audit
# hand-written vs generated for structural drift BEFORE flipping the
# src/index.ts re-export. The worst-case regression is a type whose
# generated shape differs from the hand-written shape in a way that Zod
# validators would reject at runtime.
#
# Behaviour:
#   - Pre-Plan-02 (generated/ dir has only .gitkeep + .whitelist.txt): exit 0
#     with SKIPPED message.
#   - Post-Plan-02: for each type name in .whitelist.txt that exists as a
#     generated .ts file, diff the normalized field list against the
#     hand-written version in packages/shared-types/src/*.ts.
#
# Output: human-readable report per type. PARITY OK = fields match; DRIFT =
# fields differ (manual review required before flipping re-export).
#
# Usage:
#   bash scripts/audit-handwritten-vs-generated.sh
#
# Exit codes:
#   0 = every audited pair is PARITY OK (or pre-Plan-02 skip)
#   1 = one or more DRIFT pairs found (blocks re-export flip)

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$REPO_ROOT"

GEN_DIR="packages/shared-types/generated"
SRC_DIR="packages/shared-types/src"
WHITELIST="$GEN_DIR/.whitelist.txt"

# Pre-Plan-02 gate: skip cleanly if no generated .ts files exist yet.
if ! compgen -G "$GEN_DIR/*.ts" > /dev/null; then
    echo "SKIPPED — no generated types yet, run Plan 02 first"
    exit 0
fi

if [ ! -f "$WHITELIST" ]; then
    echo "SKIPPED — whitelist missing ($WHITELIST); run scripts/enumerate-admin-types.sh"
    exit 0
fi

echo "=== D-12 DRIFT AUDIT ==="
DRIFT_FOUND=0
PAIRS_AUDITED=0

while IFS= read -r TYPE_NAME; do
    [[ "$TYPE_NAME" =~ ^[[:space:]]*# ]] && continue
    [[ -z "${TYPE_NAME// }" ]] && continue

    GEN_HIT="$GEN_DIR/$TYPE_NAME.ts"
    # Hand-written re-exports use `export type { X, Y, Z } from './file'` —
    # scan for the type name inside any src/*.ts.
    SRC_HIT=$(grep -l -E "\\b$TYPE_NAME\\b" "$SRC_DIR"/*.ts 2>/dev/null | head -1 || true)

    if [ -f "$GEN_HIT" ] && [ -n "$SRC_HIT" ]; then
        PAIRS_AUDITED=$((PAIRS_AUDITED + 1))
        echo "--- $TYPE_NAME ---"
        echo "  src: $SRC_HIT"
        echo "  gen: $GEN_HIT"

        # Normalize both sides to a sorted list of field-like lines.
        SRC_NORM=$(grep -E "^[[:space:]]*[a-z_][A-Za-z0-9_]*[[:space:]]*\??:" "$SRC_HIT" 2>/dev/null \
                   | grep -v '^[[:space:]]*//' \
                   | sed -E 's/[[:space:]]+//g' \
                   | sort -u || true)
        GEN_NORM=$(grep -E "^[[:space:]]*[a-z_][A-Za-z0-9_]*[[:space:]]*\??:" "$GEN_HIT" 2>/dev/null \
                   | grep -v '^[[:space:]]*//' \
                   | sed -E 's/[[:space:]]+//g' \
                   | sort -u || true)

        if [ "$SRC_NORM" = "$GEN_NORM" ]; then
            echo "  STATUS: PARITY OK"
        else
            echo "  STATUS: DRIFT — fields differ; review before flipping re-export"
            diff <(echo "$SRC_NORM") <(echo "$GEN_NORM") || true
            DRIFT_FOUND=1
        fi
    fi
done < "$WHITELIST"

echo ""
echo "=== SUMMARY ==="
echo "Pairs audited: $PAIRS_AUDITED"
echo "Drift found:   $DRIFT_FOUND (0 = OK to flip re-exports, 1 = review required)"

exit "$DRIFT_FOUND"
