#!/usr/bin/env bash
# Phase 347 Pre-Deploy Gate
# Verifies Phase 343 Plans 01+02+04 are in git history before allowing Phase 347 deploy.
# Per DEP-01: Phase 343 Plans 01+02+04 executed and deployed.
# Per DEP-04: Pre-deploy script greps git log for Phase 343 merge commits
# and hard-fails Phase 347 deploy if missing.
set -e

echo "=== Phase 347 Pre-Deploy Gate ==="
echo ""

PASS=true

# Check for Phase 343 Plan 01 (cloud-authority 409 guard)
# Commit: b31c38e0
if git log --oneline | grep -q "343-01\|b31c38e0\|cloud.authority\|343.*Plan.*01"; then
  echo "[PASS] Phase 343 Plan 01 found in git history"
else
  echo "[FAIL] Phase 343 Plan 01 NOT found in git history"
  echo "       Phase 343-01 (cloud-authority 409 guard) must be committed before Phase 347 deploys."
  PASS=false
fi

# Check for Phase 343 Plan 02 (post-write verify + delayed sync verify)
# Commit: 6c870f99
if git log --oneline | grep -q "343-02\|6c870f99\|post.write.verify\|343.*Plan.*02"; then
  echo "[PASS] Phase 343 Plan 02 found in git history"
else
  echo "[FAIL] Phase 343 Plan 02 NOT found in git history"
  echo "       Phase 343-02 (post-write verify + delayed sync verify) must be committed before Phase 347 deploys."
  PASS=false
fi

# Check for Phase 343 Plan 04 (per DEP-01: Plans 01+02+04 required)
# Commit: 4074bb0d
if git log --oneline | grep -q "343-04\|4074bb0d\|343.*Plan.*04"; then
  echo "[PASS] Phase 343 Plan 04 found in git history"
else
  echo "[FAIL] Phase 343 Plan 04 NOT found in git history"
  echo "       Phase 343-04 must be committed before Phase 347 deploys (per DEP-01)."
  PASS=false
fi

# Check that change_staff_pin_safe endpoint exists in codebase
if grep -q "change_staff_pin_safe" crates/racecontrol/src/api/routes.rs 2>/dev/null; then
  echo "[PASS] change_staff_pin_safe handler found in routes.rs"
else
  echo "[FAIL] change_staff_pin_safe handler NOT found in routes.rs"
  echo "       Phase 347-01 must be completed before deploy."
  PASS=false
fi

# Check feature flag default (should be off)
if grep -rq 'FEATURE_STAFF_PIN_UI.*=.*"on"' racingpoint-admin/.env.production* 2>/dev/null; then
  echo "[WARN] FEATURE_STAFF_PIN_UI is set to 'on' in .env.production"
  echo "       This should only be enabled AFTER Phase 343 is live-deployed to venue + cloud."
else
  echo "[PASS] FEATURE_STAFF_PIN_UI is not enabled (correct default per STAFF-10)"
fi

echo ""

if [ "$PASS" = true ]; then
  echo "=== Phase 347 Pre-Deploy Gate: PASSED ==="
  exit 0
else
  echo "=== Phase 347 Pre-Deploy Gate: FAILED ==="
  echo ""
  echo "Phase 343 Plans 01+02+04 must be committed AND deployed to venue (.23) + cloud (Bono VPS)"
  echo "before Phase 347 can ship. See DEP-01 in REQUIREMENTS.md."
  exit 1
fi
