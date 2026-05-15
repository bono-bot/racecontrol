#!/bin/bash
# Dry-test for pre-prompt-stranded-branch-report.js v0.1.0
# Per `feedback_dry_test_patched_script_before_fleet_20260509` — verify on single
# input class before any settings.json wire-in.

HOOK="$(dirname "$0")/pre-prompt-stranded-branch-report.js"

echo "=== TEST 1: normal user prompt ==="
echo '{"hookSpecificInput":{"userMessage":"check on the V2 kiosk theme"}}' | node "$HOOK"
echo
echo

echo "=== TEST 2: empty prompt (cold session-start) ==="
echo '{}' | node "$HOOK"
echo
echo

echo "=== TEST 3: malformed JSON (defensive path) ==="
echo 'not-json' | node "$HOOK"
echo "exit=$?"
echo

echo "=== Sanity: did the hook emit JSON with hookSpecificOutput when stranded branches exist? ==="
out=$(echo '{"hookSpecificInput":{"userMessage":"test"}}' | node "$HOOK")
if echo "$out" | grep -q 'hookSpecificOutput'; then
  echo "OK: emitted hookSpecificOutput envelope"
else
  echo "NOTE: no envelope emitted — either no stranded branches detected OR error path"
fi
echo
echo "=== END TESTS ==="
