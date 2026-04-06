#!/usr/bin/env bash
#
# self-audit.sh -- AI Self-Audit: capture screenshots of critical pages
# and generate a structured audit prompt for Claude to review.
#
# Usage:
#   bash tests/page-audit/self-audit.sh              # full run (crawl + prompt)
#   bash tests/page-audit/self-audit.sh --skip-crawl  # use existing screenshots
#
# Output:
#   tests/page-audit/audit-prompt.md   -- structured review instructions
#   tests/page-audit/audit-report.md   -- template for AI findings
#

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
SCREENSHOTS_DIR="$REPO_ROOT/tests/screenshots"
DESCRIPTIONS_DIR="$SCRIPT_DIR/descriptions"
AUDIT_PROMPT="$SCRIPT_DIR/audit-prompt.md"
AUDIT_REPORT="$SCRIPT_DIR/audit-report.md"

SKIP_CRAWL=false
if [[ "${1:-}" == "--skip-crawl" ]]; then
  SKIP_CRAWL=true
fi

# ---------------------------------------------------------------------------
# Critical pages (must match CRITICAL_PAGES in visual.spec.ts)
# ---------------------------------------------------------------------------

WEB_PAGES=("/" "/fleet" "/billing" "/billing/history" "/sessions" "/drivers" "/games")
KIOSK_PAGES=("/kiosk" "/kiosk/staff" "/kiosk/control")

# Map route path to description filename
route_to_desc_file() {
  local app="$1"
  local route="$2"
  case "$app:$route" in
    web:/)                echo "web-home.md" ;;
    web:/fleet)           echo "web-fleet.md" ;;
    web:/billing)         echo "web-billing.md" ;;
    web:/billing/history) echo "web-billing-history.md" ;;
    web:/sessions)        echo "web-sessions.md" ;;
    web:/drivers)         echo "web-drivers.md" ;;
    web:/games)           echo "web-games.md" ;;
    kiosk:/kiosk)         echo "kiosk-landing.md" ;;
    kiosk:/kiosk/staff)   echo "kiosk-staff.md" ;;
    kiosk:/kiosk/control) echo "kiosk-control.md" ;;
    *)                    echo "" ;;
  esac
}

# Sanitize route path to match crawler directory naming
sanitize_route() {
  local route="$1"
  local sanitized
  sanitized=$(echo "$route" | sed 's|^/||; s|/|_|g')
  if [[ -z "$sanitized" ]]; then
    sanitized="root"
  fi
  echo "$sanitized"
}

# Find the latest screenshot for a given app/route
find_latest_screenshot() {
  local app="$1"
  local route="$2"
  local dir="$SCREENSHOTS_DIR/$app/$(sanitize_route "$route")"
  if [[ -d "$dir" ]]; then
    # Find most recent .png file
    local latest
    latest=$(ls -t "$dir"/*.png 2>/dev/null | head -1)
    if [[ -n "$latest" ]]; then
      # Return path relative to repo root
      echo "${latest#$REPO_ROOT/}"
      return 0
    fi
  fi
  return 1
}

# ---------------------------------------------------------------------------
# Step 1: Run page crawler (unless --skip-crawl)
# ---------------------------------------------------------------------------

CRAWLED_COUNT=0

if [[ "$SKIP_CRAWL" == "false" ]]; then
  echo "Step 1: Running page crawler on critical pages..."

  # Crawl web pages
  echo "  Crawling web pages: ${WEB_PAGES[*]}"
  WEB_PAGES_CSV=$(IFS=,; echo "${WEB_PAGES[*]}")
  cd "$REPO_ROOT"
  CRAWL_PAGES="$WEB_PAGES_CSV" CRAWL_APP=web \
    npx playwright test --config tests/page-crawler/playwright.config.ts 2>/dev/null || true
  CRAWLED_COUNT=$((CRAWLED_COUNT + ${#WEB_PAGES[@]}))

  # Crawl kiosk pages
  echo "  Crawling kiosk pages: ${KIOSK_PAGES[*]}"
  KIOSK_PAGES_CSV=$(IFS=,; echo "${KIOSK_PAGES[*]}")
  CRAWL_PAGES="$KIOSK_PAGES_CSV" CRAWL_APP=kiosk \
    npx playwright test --config tests/page-crawler/playwright.config.ts 2>/dev/null || true
  CRAWLED_COUNT=$((CRAWLED_COUNT + ${#KIOSK_PAGES[@]}))

  echo "  Crawl complete: attempted $CRAWLED_COUNT pages"
else
  echo "Step 1: Skipped (--skip-crawl)"
fi

# ---------------------------------------------------------------------------
# Step 2 & 3: Find screenshots and generate audit-prompt.md
# ---------------------------------------------------------------------------

echo "Step 2: Finding latest screenshots..."
echo "Step 3: Generating audit-prompt.md..."

FOUND_COUNT=0
PAGE_NUM=0

{
  cat <<'HEADER'
# Self-Audit: Page Review

For each page below, read the screenshot image and compare against the description.
Flag any anomalies -- layout issues, missing data, error states, unexpected content.

Write findings to `tests/page-audit/audit-report.md`.

HEADER

  # Process web pages
  for route in "${WEB_PAGES[@]}"; do
    PAGE_NUM=$((PAGE_NUM + 1))
    desc_file=$(route_to_desc_file "web" "$route")
    name=$(echo "$route" | sed 's|^/||; s|/| |g')
    [[ -z "$name" ]] && name="Home"
    name="$(echo "$name" | sed 's/\b\(.\)/\u\1/g')"

    screenshot_path=""
    if screenshot_path=$(find_latest_screenshot "web" "$route"); then
      FOUND_COUNT=$((FOUND_COUNT + 1))
    else
      screenshot_path="(no screenshot found -- run without --skip-crawl)"
    fi

    echo "## Page $PAGE_NUM: $name (web $route)"
    echo "- Screenshot: $screenshot_path"
    echo "- Description: tests/page-audit/descriptions/$desc_file"
    echo ""
  done

  # Process kiosk pages
  for route in "${KIOSK_PAGES[@]}"; do
    PAGE_NUM=$((PAGE_NUM + 1))
    desc_file=$(route_to_desc_file "kiosk" "$route")
    name=$(echo "$route" | sed 's|^/kiosk||; s|^/||; s|/| |g')
    [[ -z "$name" ]] && name="Landing"
    name="Kiosk $(echo "$name" | sed 's/\b\(.\)/\u\1/g')"

    screenshot_path=""
    if screenshot_path=$(find_latest_screenshot "kiosk" "$route"); then
      FOUND_COUNT=$((FOUND_COUNT + 1))
    else
      screenshot_path="(no screenshot found -- run without --skip-crawl)"
    fi

    echo "## Page $PAGE_NUM: $name (kiosk $route)"
    echo "- Screenshot: $screenshot_path"
    echo "- Description: tests/page-audit/descriptions/$desc_file"
    echo ""
  done

} > "$AUDIT_PROMPT"

echo "  Found $FOUND_COUNT/$PAGE_NUM screenshots"

# ---------------------------------------------------------------------------
# Step 4: Generate audit-report.md template
# ---------------------------------------------------------------------------

echo "Step 4: Generating audit-report.md template..."

TIMESTAMP=$(date -u +"%Y-%m-%dT%H:%M:%SZ")

cat > "$AUDIT_REPORT" <<REPORT
# Page Audit Report
**Generated:** $TIMESTAMP
**Status:** PENDING -- run self-audit review to populate

## Results
(To be filled by AI after reviewing screenshots against descriptions)
REPORT

# ---------------------------------------------------------------------------
# Summary
# ---------------------------------------------------------------------------

echo ""
echo "=== Self-Audit Summary ==="
echo "  Pages targeted:    $PAGE_NUM"
echo "  Screenshots found: $FOUND_COUNT"
echo "  Audit prompt:      $AUDIT_PROMPT"
echo "  Audit report:      $AUDIT_REPORT"
echo ""

if [[ "$SKIP_CRAWL" == "false" ]]; then
  echo "  Pages crawled:     $CRAWLED_COUNT"
fi

echo "Done. Review audit-prompt.md and run the AI audit."
