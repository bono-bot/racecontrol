#!/usr/bin/env bash
# Phase 396 — Architecture + Conventions Docs verification script
# Encodes 13 deterministic assertions from 396-VALIDATION.md.
# Guiding principle: If a rule is not enforced mechanically by a hook or CI check,
# we will not follow it — so we will not write it down.
# Deferred Rules / Canonical home: workspace/ (this script lives in the phase dir
# until Phase 398 promotes the docs it verifies into the workspace repo).
set -u

PHASE_DIR="$(cd "$(dirname "$0")" && pwd)"
CONV="$PHASE_DIR/CONVENTIONS.md"
ARCH="$PHASE_DIR/ARCHITECTURE.md"
PASS=0
FAIL=0
PENDING=0

pass() { echo "  PASS: $1"; PASS=$((PASS+1)); }
fail() { echo "  FAIL: $1"; FAIL=$((FAIL+1)); }
pending() { echo "  PENDING: $1"; PENDING=$((PENDING+1)); }

CONV_OK=0
ARCH_OK=0

echo "[1/13] Both files exist (gating)"
if [ -f "$CONV" ]; then
  CONV_OK=1
  pass "CONVENTIONS.md exists"
else
  pending "CONVENTIONS.md missing — dependent checks will be marked pending"
fi
if [ -f "$ARCH" ]; then
  ARCH_OK=1
  pass "ARCHITECTURE.md exists"
else
  pending "ARCHITECTURE.md missing — dependent checks will be marked pending"
fi

echo "[2/13] Line cap (<= 500)"
if [ "$CONV_OK" -eq 1 ]; then
  CONV_LINES=$(wc -l < "$CONV")
  if [ "$CONV_LINES" -le 500 ]; then
    pass "CONVENTIONS.md line count=$CONV_LINES"
  else
    fail "CONVENTIONS.md line count=$CONV_LINES exceeds 500"
  fi
else
  pending "CONVENTIONS.md line cap (file missing)"
fi
if [ "$ARCH_OK" -eq 1 ]; then
  ARCH_LINES=$(wc -l < "$ARCH")
  if [ "$ARCH_LINES" -le 500 ]; then
    pass "ARCHITECTURE.md line count=$ARCH_LINES"
  else
    fail "ARCHITECTURE.md line count=$ARCH_LINES exceeds 500"
  fi
else
  pending "ARCHITECTURE.md line cap (file missing)"
fi

echo "[3/13] Guiding principle verbatim in both"
if [ "$CONV_OK" -eq 1 ]; then
  if grep -qF "If a rule is not enforced mechanically" "$CONV"; then
    pass "CONVENTIONS.md guiding principle present"
  else
    fail "CONVENTIONS.md guiding principle missing"
  fi
else
  pending "CONVENTIONS.md guiding principle (file missing)"
fi
if [ "$ARCH_OK" -eq 1 ]; then
  if grep -qF "If a rule is not enforced mechanically" "$ARCH"; then
    pass "ARCHITECTURE.md guiding principle present"
  else
    fail "ARCHITECTURE.md guiding principle missing"
  fi
else
  pending "ARCHITECTURE.md guiding principle (file missing)"
fi

echo "[4/13] Deferred Rules section present in CONVENTIONS.md"
if [ "$CONV_OK" -eq 1 ]; then
  if grep -q '^## Deferred Rules' "$CONV"; then
    pass "Deferred Rules section present"
  else
    fail "Deferred Rules section missing"
  fi
else
  pending "Deferred Rules section (file missing)"
fi

echo "[5/13] All 8 rules demoted to Deferred"
if [ "$CONV_OK" -eq 1 ]; then
  COUNT=$(awk '/^## Deferred Rules/,/^## [^#]/' "$CONV" | grep -cE '^\| [1-8] ')
  if [ "$COUNT" -eq 8 ]; then
    pass "Deferred Rules contains 8 rows"
  else
    fail "Deferred Rules contains $COUNT rows (expected 8)"
  fi
else
  pending "Deferred rules row count (file missing)"
fi

echo "[6/13] Every Deferred row cites phase 397-412"
if [ "$CONV_OK" -eq 1 ]; then
  COUNT=$(awk '/^## Deferred Rules/,/^## [^#]/' "$CONV" | grep -cE '(Phase|phase) (39[7-9]|4[0-1][0-9])')
  if [ "$COUNT" -ge 8 ]; then
    pass "Deferred Rules cites >=8 creating phases ($COUNT)"
  else
    fail "Deferred Rules cites only $COUNT creating phases (expected >=8)"
  fi
else
  pending "Deferred phase cites (file missing)"
fi

echo "[7/13] ARCHITECTURE.md has >= 11 artifact-type rows"
if [ "$ARCH_OK" -eq 1 ]; then
  COUNT=$(grep -c '^| \[DEFERRED' "$ARCH")
  if [ "$COUNT" -ge 11 ]; then
    pass "ARCHITECTURE.md has $COUNT [DEFERRED rows"
  else
    fail "ARCHITECTURE.md has $COUNT [DEFERRED rows (expected >=11)"
  fi
else
  pending "ARCHITECTURE.md deferred rows (file missing)"
fi

echo "[8/13] Every ARCHITECTURE decision table row is [DEFERRED — Phase N] or header"
if [ "$ARCH_OK" -eq 1 ]; then
  ORPHANS=$(awk '/^\| /' "$ARCH" | grep -vE '(\[DEFERRED|Enforcer|Destination|---|Artifact type)' | wc -l)
  if [ "$ORPHANS" -eq 0 ]; then
    pass "No orphan decision table rows"
  else
    fail "$ORPHANS orphan decision table rows found"
  fi
else
  pending "ARCHITECTURE.md orphan check (file missing)"
fi

echo "[9/13] No stray live Enforcer citations"
if [ "$CONV_OK" -eq 1 ]; then
  if ! grep -qE '^Enforcer:' "$CONV"; then
    pass "CONVENTIONS.md has no live Enforcer: lines"
  else
    fail "CONVENTIONS.md has live Enforcer: line(s)"
  fi
else
  pending "CONVENTIONS.md enforcer scan (file missing)"
fi
if [ "$ARCH_OK" -eq 1 ]; then
  if ! grep -qE '^Enforcer:' "$ARCH"; then
    pass "ARCHITECTURE.md has no live Enforcer: lines"
  else
    fail "ARCHITECTURE.md has live Enforcer: line(s)"
  fi
else
  pending "ARCHITECTURE.md enforcer scan (file missing)"
fi

echo "[10/13] D-12 footer present in both"
if [ "$CONV_OK" -eq 1 ]; then
  if grep -qF 'Canonical home: workspace/' "$CONV"; then
    pass "CONVENTIONS.md D-12 footer present"
  else
    fail "CONVENTIONS.md D-12 footer missing"
  fi
else
  pending "CONVENTIONS.md D-12 footer (file missing)"
fi
if [ "$ARCH_OK" -eq 1 ]; then
  if grep -qF 'Canonical home: workspace/' "$ARCH"; then
    pass "ARCHITECTURE.md D-12 footer present"
  else
    fail "ARCHITECTURE.md D-12 footer missing"
  fi
else
  pending "ARCHITECTURE.md D-12 footer (file missing)"
fi

echo "[11/13] Rule #9 (canonical-source marker) NOT added per D-05"
if [ "$CONV_OK" -eq 1 ]; then
  if ! grep -q 'canonical-source' "$CONV"; then
    pass "CONVENTIONS.md contains no canonical-source marker"
  elif grep -q 'canonical-source' "$CONV" && grep -q 'not added' "$CONV"; then
    pass "CONVENTIONS.md mentions canonical-source only in a 'not added' note"
  else
    fail "CONVENTIONS.md mentions canonical-source without 'not added' context"
  fi
else
  pending "CONVENTIONS.md canonical-source check (file missing)"
fi

echo "[12/13] 393 back-references"
if [ "$ARCH_OK" -eq 1 ]; then
  C=$(grep -c '(Phase 393' "$ARCH")
  if [ "$C" -ge 3 ]; then
    pass "ARCHITECTURE.md has $C (Phase 393 cites (>=3)"
  else
    fail "ARCHITECTURE.md has $C (Phase 393 cites (expected >=3)"
  fi
else
  pending "ARCHITECTURE.md 393 cites (file missing)"
fi
if [ "$CONV_OK" -eq 1 ]; then
  C=$(grep -c '(Phase 393' "$CONV")
  if [ "$C" -ge 2 ]; then
    pass "CONVENTIONS.md has $C (Phase 393 cites (>=2)"
  else
    fail "CONVENTIONS.md has $C (Phase 393 cites (expected >=2)"
  fi
else
  pending "CONVENTIONS.md 393 cites (file missing)"
fi

echo "[13/13] ARCHITECTURE.md has '## Adding a New Artifact Type' section"
if [ "$ARCH_OK" -eq 1 ]; then
  if grep -q '^## Adding a New Artifact Type' "$ARCH"; then
    pass "Adding a New Artifact Type section present"
  else
    fail "Adding a New Artifact Type section missing"
  fi
else
  pending "Adding a New Artifact Type section (file missing)"
fi

echo ""
echo "======================================"
echo "Phase 396 Verification: $PASS passed / $FAIL failed / $PENDING pending"
echo "======================================"

if [ "$FAIL" -gt 0 ]; then
  exit 2
elif [ "$PENDING" -gt 0 ]; then
  exit 1
else
  exit 0
fi
