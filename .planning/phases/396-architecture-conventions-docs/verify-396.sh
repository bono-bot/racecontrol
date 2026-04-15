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

# check_result <state> <label>
#   state: pass | fail | pending
check_result() {
  local state="$1"
  local label="$2"
  case "$state" in
    pass)    echo "  PASS: $label";    PASS=$((PASS+1)) ;;
    fail)    echo "  FAIL: $label";    FAIL=$((FAIL+1)) ;;
    pending) echo "  PENDING: $label"; PENDING=$((PENDING+1)) ;;
  esac
}

CONV_OK=0
ARCH_OK=0
[ -f "$CONV" ] && CONV_OK=1
[ -f "$ARCH" ] && ARCH_OK=1

# -------- Check 1: both files exist --------
echo "[1/13] Both files exist (gating)"
if [ "$CONV_OK" -eq 1 ] && [ "$ARCH_OK" -eq 1 ]; then
  check_result pass "CONVENTIONS.md + ARCHITECTURE.md exist"
else
  missing=""
  [ "$CONV_OK" -eq 0 ] && missing="CONVENTIONS.md"
  [ "$ARCH_OK" -eq 0 ] && missing="${missing:+$missing, }ARCHITECTURE.md"
  check_result pending "missing: $missing (dependent checks pending)"
fi

# -------- Check 2: line cap --------
echo "[2/13] Line cap (<= 500)"
if [ "$CONV_OK" -eq 1 ] && [ "$ARCH_OK" -eq 1 ]; then
  CL=$(wc -l < "$CONV")
  AL=$(wc -l < "$ARCH")
  if [ "$CL" -le 500 ] && [ "$AL" -le 500 ]; then
    check_result pass "CONVENTIONS.md=$CL lines, ARCHITECTURE.md=$AL lines"
  else
    check_result fail "CONVENTIONS.md=$CL, ARCHITECTURE.md=$AL (cap=500)"
  fi
else
  check_result pending "line cap (docs not present)"
fi

# -------- Check 3: guiding principle verbatim in both --------
echo "[3/13] Guiding principle verbatim in both"
if [ "$CONV_OK" -eq 1 ] && [ "$ARCH_OK" -eq 1 ]; then
  if grep -qF "If a rule is not enforced mechanically" "$CONV" \
     && grep -qF "If a rule is not enforced mechanically" "$ARCH"; then
    check_result pass "guiding principle present in both docs"
  else
    check_result fail "guiding principle missing in one or both docs"
  fi
else
  check_result pending "guiding principle (docs not present)"
fi

# -------- Check 4: Deferred Rules section in CONVENTIONS --------
echo "[4/13] Deferred Rules section present in CONVENTIONS.md"
if [ "$CONV_OK" -eq 1 ]; then
  if grep -q '^## Deferred Rules' "$CONV"; then
    check_result pass "Deferred Rules section present"
  else
    check_result fail "Deferred Rules section missing"
  fi
else
  check_result pending "Deferred Rules section (CONVENTIONS.md missing)"
fi

# -------- Check 5: 8 rules demoted to Deferred --------
echo "[5/13] All 8 rules demoted to Deferred"
if [ "$CONV_OK" -eq 1 ]; then
  COUNT=$(awk '/^## Deferred Rules/{flag=1;next} /^## /{flag=0} flag' "$CONV" | grep -cE '^\| [1-8] ')
  if [ "$COUNT" -eq 8 ]; then
    check_result pass "Deferred Rules contains 8 rows"
  else
    check_result fail "Deferred Rules contains $COUNT rows (expected 8)"
  fi
else
  check_result pending "Deferred rule count (CONVENTIONS.md missing)"
fi

# -------- Check 6: every Deferred row cites 397-412 --------
echo "[6/13] Every Deferred row cites phase 397-412"
if [ "$CONV_OK" -eq 1 ]; then
  COUNT=$(awk '/^## Deferred Rules/{flag=1;next} /^## /{flag=0} flag' "$CONV" | grep -cE '(Phase|phase) (39[7-9]|4[0-1][0-9])')
  if [ "$COUNT" -ge 8 ]; then
    check_result pass "Deferred Rules cites $COUNT creating-phase references (>=8)"
  else
    check_result fail "Deferred Rules cites $COUNT creating-phase references (expected >=8)"
  fi
else
  check_result pending "Deferred phase cites (CONVENTIONS.md missing)"
fi

# -------- Check 7: ARCHITECTURE >= 11 deferred artifact-type rows --------
echo "[7/13] ARCHITECTURE.md has >= 11 artifact-type rows"
if [ "$ARCH_OK" -eq 1 ]; then
  COUNT=$(grep -E '^\|' "$ARCH" | grep -c '\[DEFERRED')
  if [ "$COUNT" -ge 11 ]; then
    check_result pass "ARCHITECTURE.md has $COUNT DEFERRED rows (>=11)"
  else
    check_result fail "ARCHITECTURE.md has $COUNT DEFERRED rows (expected >=11)"
  fi
else
  check_result pending "ARCHITECTURE.md deferred rows (file missing)"
fi

# -------- Check 8: no orphan decision-table rows --------
echo "[8/13] Every ARCHITECTURE decision table row is [DEFERRED — Phase N] or header"
if [ "$ARCH_OK" -eq 1 ]; then
  ORPHANS=$(awk '/^\| /' "$ARCH" | grep -vE '(\[DEFERRED|Enforcer|Destination|---|Artifact type)' | wc -l)
  if [ "$ORPHANS" -eq 0 ]; then
    check_result pass "no orphan decision table rows"
  else
    check_result fail "$ORPHANS orphan decision table rows"
  fi
else
  check_result pending "ARCHITECTURE.md orphan check (file missing)"
fi

# -------- Check 9: no stray live Enforcer citations --------
echo "[9/13] No stray live Enforcer citations"
if [ "$CONV_OK" -eq 1 ] && [ "$ARCH_OK" -eq 1 ]; then
  if ! grep -qE '^Enforcer:' "$CONV" && ! grep -qE '^Enforcer:' "$ARCH"; then
    check_result pass "no live Enforcer: lines in either doc"
  else
    check_result fail "live Enforcer: line found in one or both docs"
  fi
else
  check_result pending "enforcer scan (docs not present)"
fi

# -------- Check 10: D-12 footer present in both --------
echo "[10/13] D-12 footer present in both"
if [ "$CONV_OK" -eq 1 ] && [ "$ARCH_OK" -eq 1 ]; then
  if grep -qF 'Canonical home: workspace/' "$CONV" \
     && grep -qF 'Canonical home: workspace/' "$ARCH"; then
    check_result pass "D-12 footer present in both docs"
  else
    check_result fail "D-12 footer missing in one or both docs"
  fi
else
  check_result pending "D-12 footer (docs not present)"
fi

# -------- Check 11: Rule #9 (canonical-source marker) NOT added --------
echo "[11/13] Rule #9 (canonical-source marker) NOT added per D-05"
if [ "$CONV_OK" -eq 1 ]; then
  if ! grep -q 'canonical-source' "$CONV"; then
    check_result pass "CONVENTIONS.md contains no canonical-source marker"
  elif grep -q 'canonical-source' "$CONV" && grep -q 'not added' "$CONV"; then
    check_result pass "canonical-source mentioned only in 'not added' context"
  else
    check_result fail "canonical-source mentioned without 'not added' context"
  fi
else
  check_result pending "canonical-source check (CONVENTIONS.md missing)"
fi

# -------- Check 12: 393 back-references --------
echo "[12/13] 393 back-references"
if [ "$CONV_OK" -eq 1 ] && [ "$ARCH_OK" -eq 1 ]; then
  CC=$(grep -c '(Phase 393' "$CONV")
  AC=$(grep -c '(Phase 393' "$ARCH")
  if [ "$AC" -ge 3 ] && [ "$CC" -ge 2 ]; then
    check_result pass "ARCHITECTURE.md=$AC (>=3), CONVENTIONS.md=$CC (>=2) '(Phase 393' cites"
  else
    check_result fail "ARCHITECTURE.md=$AC (expected >=3), CONVENTIONS.md=$CC (expected >=2)"
  fi
else
  check_result pending "393 back-references (docs not present)"
fi

# -------- Check 13: ARCHITECTURE Adding a New Artifact Type section --------
echo "[13/13] ARCHITECTURE.md has '## Adding a New Artifact Type' section"
if [ "$ARCH_OK" -eq 1 ]; then
  if grep -q '^## Adding a New Artifact Type' "$ARCH"; then
    check_result pass "Adding a New Artifact Type section present"
  else
    check_result fail "Adding a New Artifact Type section missing"
  fi
else
  check_result pending "Adding a New Artifact Type section (ARCHITECTURE.md missing)"
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
