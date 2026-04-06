#!/usr/bin/env bash
# Visual regression test runner with 3 modes:
#   baseline     — Capture current state as baselines
#   compare      — Compare current state against baselines (default)
#   before-after — Interactive: capture before, wait for fix, compare after
#
# Usage:
#   bash scripts/visual-regression.sh baseline
#   bash scripts/visual-regression.sh compare
#   bash scripts/visual-regression.sh before-after

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
VR_CONFIG="tests/visual-regression/playwright.config.ts"
SCREENSHOTS_DIR="tests/visual-regression/__screenshots__"

MODE="${1:-compare}"

case "$MODE" in
  baseline)
    echo "[VR] Capturing baselines..."
    cd "$REPO_ROOT"
    npx playwright test --config "$VR_CONFIG" --update-snapshots
    echo ""
    echo "[VR] Baselines saved to $SCREENSHOTS_DIR/"
    echo "[VR] Commit these with:"
    echo "  git add $SCREENSHOTS_DIR && git commit -m 'test: update visual baselines'"
    ;;

  compare)
    echo "[VR] Comparing against baselines..."
    cd "$REPO_ROOT"
    npx playwright test --config "$VR_CONFIG"
    echo ""
    echo "[VR] All visual regression tests passed."
    ;;

  before-after)
    echo "[VR] === BEFORE/AFTER WORKFLOW ==="
    echo ""
    echo "[VR] Step 1: Capturing BEFORE baselines..."
    cd "$REPO_ROOT"
    npx playwright test --config "$VR_CONFIG" --update-snapshots
    echo ""
    echo "[VR] Baselines captured. Make your frontend fix now."
    echo "[VR] When ready, press ENTER to capture AFTER and compare..."
    read -r
    echo "[VR] Step 2: Comparing AFTER against BEFORE baselines..."
    npx playwright test --config "$VR_CONFIG" 2>&1 || true
    echo ""
    echo "[VR] Check test-results/visual-regression/ for diff images."
    echo "[VR] If the changes look correct, update baselines:"
    echo "  bash scripts/visual-regression.sh baseline"
    ;;

  *)
    echo "Usage: bash scripts/visual-regression.sh [baseline|compare|before-after]"
    echo ""
    echo "  baseline     -- Capture current state as baselines"
    echo "  compare      -- Compare current state against baselines (default)"
    echo "  before-after -- Interactive: capture before, wait for fix, compare after"
    exit 1
    ;;
esac
