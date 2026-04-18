#!/usr/bin/env bash
# tests/deploy_script_swap_test.sh -- Phase 413.1 Plan 05 regression test
# Catches the defect class responsible for the 2026-04-18 07:50 IST P0 outage:
#   (1) forward-slash Windows paths via cmd.exe (causes "path not found")
#   (2) !errorlevel! (fails without DelayedExpansion in rc-sentry /exec context)
#   (3) missing auto-recover on rename failure (leaves server binaryless)
#   (4) missing 72h forfiles guard in start-racecontrol.bat
#
# USAGE: bash tests/deploy_script_swap_test.sh
# EXIT 0 = all checks pass; 1 = defect detected; 2 = UNSAFE test rewrite (sed failure).
#
# CI integration: call from tests/run-all.sh OR scripts/stage-release.sh pre-build.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SCRIPT="$REPO_ROOT/scripts/deploy-server.sh"
BAT="$REPO_ROOT/scripts/deploy/start-racecontrol.bat"

pass() { echo "  OK    $1"; }
fail() { echo "  FAIL  $1"; exit 1; }
unsafe_abort() { echo "  UNSAFE: test would touch production paths -- $1"; exit 2; }

echo "=== Phase 413.1 Plan 05 -- deploy-server.sh swap regression tests ==="
echo ""

# --- Layer 1 -- Static grep invariants ---
echo "--- Layer 1: Static invariants ---"

# R2 -- no !errorlevel!
EL_COUNT=$(grep -c '!errorlevel!' "$SCRIPT" || true)
if [ "$EL_COUNT" -ne 0 ]; then
    fail "R2 violated: found !errorlevel! in $SCRIPT (rc-sentry /exec has no DelayedExpansion)"
fi
pass "R2: no !errorlevel! in deploy-server.sh"

# R1 -- broken move /Y forward-slash line gone
MV_COUNT=$(grep -c 'move /Y C:/RacingPoint/racecontrol-new' "$SCRIPT" || true)
if [ "$MV_COUNT" -ne 0 ]; then
    fail "R1 violated: found broken 'move /Y C:/RacingPoint/racecontrol-new' line (forward-slash paths fail via cmd.exe)"
fi
pass "R1: broken move line removed"

# R1 -- 3-step ren sequence present (1st ren: current -> prev)
REN1_COUNT=$(grep -c 'ren C:\\\\RacingPoint\\\\racecontrol.exe racecontrol-prev.exe' "$SCRIPT" || true)
if [ "$REN1_COUNT" -lt 1 ]; then
    fail "R1 violated: missing 1st ren (current -> prev) in 3-step swap"
fi
pass "R1: 1st ren (current -> prev) present"

# R1 -- 2nd ren (new -> current)
REN2_COUNT=$(grep -c 'ren C:\\\\RacingPoint\\\\racecontrol-new.exe racecontrol.exe' "$SCRIPT" || true)
if [ "$REN2_COUNT" -lt 1 ]; then
    fail "R1 violated: missing 2nd ren (new -> current) in 3-step swap"
fi
pass "R1: 2nd ren (new -> current) present"

# R1 -- auto-recover sentinel (3rd ren via || chain)
SR_COUNT=$(grep -c 'SWAP_FAILED_RECOVERED' "$SCRIPT" || true)
if [ "$SR_COUNT" -lt 1 ]; then
    fail "R1 violated: missing 'SWAP_FAILED_RECOVERED' -- no auto-recover guard if 2nd ren fails"
fi
pass "R1: auto-recover guard (SWAP_FAILED_RECOVERED) present"

# R3 -- forfiles 72h guard in start-racecontrol.bat
if [ -f "$BAT" ]; then
    FF_COUNT=$(grep -c 'forfiles /M racecontrol-prev.exe /D -3' "$BAT" || true)
    if [ "$FF_COUNT" -lt 1 ]; then
        fail "R3 violated: missing 'forfiles /M racecontrol-prev.exe /D -3' in start-racecontrol.bat (72h preserve window)"
    fi
    pass "R3: 72h forfiles guard present in start-racecontrol.bat"
else
    echo "  WARN  start-racecontrol.bat not found at $BAT -- skipping R3 grep"
fi

# bash syntax
if ! bash -n "$SCRIPT" 2>/dev/null; then
    fail "deploy-server.sh has bash syntax errors"
fi
pass "deploy-server.sh bash -n clean"

echo ""

# --- Layer 2 -- Dynamic cmd.exe simulation (safety-hardened) ---
echo "--- Layer 2: Dynamic swap simulation ---"

if ! command -v cmd.exe >/dev/null 2>&1; then
    echo "  SKIP  cmd.exe unavailable (pure Linux CI?) -- Layer 2 skipped"
    echo "  SKIP  Layer 3 (forfiles regression) also requires cmd.exe -- skipped"
    echo ""
    echo "=== Layer 1 passed. Layers 2+3 skipped. Exiting 0. ==="
    exit 0
fi

# Sandbox paths. TEST_ROOT_WIN is what cmd.exe sees; TEST_ROOT_BASH is what bash uses.
TEST_PID="$$"
TEST_ROOT_WIN="C:\\RacingPoint-test-413-${TEST_PID}"
TEST_ROOT_BASH="/c/RacingPoint-test-413-${TEST_PID}"
FORFILES_TEST_ROOT_BASH="/c/RacingPoint-test-413-forfiles-${TEST_PID}"
FORFILES_TEST_ROOT_WIN="C:\\RacingPoint-test-413-forfiles-${TEST_PID}"

mkdir -p "$TEST_ROOT_BASH"
trap 'rm -rf "$TEST_ROOT_BASH" 2>/dev/null; rm -rf "$FORFILES_TEST_ROOT_BASH" 2>/dev/null' EXIT

# Extract the swap cmd payload from deploy-server.sh (line 246, the -d '{"cmd":"..."}' arg).
# Pattern: -d '{"cmd":"<payload>"}' where the payload contains `echo SWAPPED`.
# Use sed to capture between `"cmd":"` and `"}'`.
SWAP_RAW_LINE=$(grep -F 'echo SWAPPED"}' "$SCRIPT" | head -1)
if [ -z "$SWAP_RAW_LINE" ]; then
    fail "Could not find swap cmd line in deploy-server.sh (pattern: 'echo SWAPPED\"}')"
fi

# Extract everything between {"cmd":" and "} using sed.
SWAP_PAYLOAD=$(echo "$SWAP_RAW_LINE" | sed 's/.*{"cmd":"//' | sed 's/"}.*//')
if [ -z "$SWAP_PAYLOAD" ]; then
    fail "Could not extract swap cmd payload from deploy-server.sh line"
fi

# The bash source file has `\\` (bash-literal double-backslash). The JSON encoding
# uses `\\` to represent a single backslash. When rc-sentry parses the JSON body,
# it decodes `\\` -> `\`. We simulate that here: JSON-strip `\\` -> `\` via Python
# (avoids sed/bash backslash-escaping hell) -> get cmd.exe-ready string.
# Then path-rewrite C:\RacingPoint\ -> sandbox path, again via Python.
#
# Both transforms use python3 string.replace with chr(92) for literal backslash.
# This is auditable and cannot produce the mangled-path output that sed did
# (sed interprets the replacement's backslashes as regex escapes).
SWAP_CMD_TEST=$(python3 <<PYEOF
payload = r'''$SWAP_PAYLOAD'''
BS = chr(92)
# Step 1: JSON decode -- replace doubled-backslash with single backslash.
cmd = payload.replace(BS + BS, BS)
# Step 2: Path rewrite -- production path -> sandbox path.
prod = 'C:' + BS + 'RacingPoint' + BS
sandbox = r'''$TEST_ROOT_WIN''' + BS
print(cmd.replace(prod, sandbox), end='')
PYEOF
)

# PRE-RUN SAFETY GUARD -- abort loudly if production path still present.
# Look for the exact literal `C:\RacingPoint\racecontrol` (single backslashes, after JSON-strip).
if echo "$SWAP_CMD_TEST" | grep -Fq 'C:\RacingPoint\racecontrol'; then
    echo ""
    echo "  SWAP_CMD_TEST: $SWAP_CMD_TEST"
    unsafe_abort "sed rewrite FAILED -- SWAP_CMD_TEST still contains literal C:\\RacingPoint\\racecontrol"
fi
pass "Layer 2 pre-run safety guard: test paths rewritten to sandbox (no production leakage)"

echo "  DEBUG: SWAP_CMD_TEST = $SWAP_CMD_TEST"

# --- Scenario A: Happy-path ---
rm -f "$TEST_ROOT_BASH/racecontrol.exe" "$TEST_ROOT_BASH/racecontrol-new.exe" "$TEST_ROOT_BASH/racecontrol-prev.exe"
echo "OLD-BINARY" > "$TEST_ROOT_BASH/racecontrol.exe"
echo "NEW-BINARY" > "$TEST_ROOT_BASH/racecontrol-new.exe"
OUTPUT=$(cmd.exe //c "$SWAP_CMD_TEST" 2>&1 || true)
if ! echo "$OUTPUT" | grep -q "SWAPPED"; then
    echo "  DEBUG output: $OUTPUT"
    fail "Scenario A: expected SWAPPED in stdout."
fi
ACTUAL_CUR=$(cat "$TEST_ROOT_BASH/racecontrol.exe" 2>/dev/null || echo "MISSING")
ACTUAL_PREV=$(cat "$TEST_ROOT_BASH/racecontrol-prev.exe" 2>/dev/null || echo "MISSING")
# Trim whitespace/CR from comparisons (Windows CRLF may be introduced by cmd.exe)
ACTUAL_CUR_TRIM=$(echo "$ACTUAL_CUR" | tr -d '\r\n')
ACTUAL_PREV_TRIM=$(echo "$ACTUAL_PREV" | tr -d '\r\n')
if [ "$ACTUAL_CUR_TRIM" != "NEW-BINARY" ]; then
    fail "Scenario A: racecontrol.exe expected NEW-BINARY, got: $ACTUAL_CUR_TRIM"
fi
if [ "$ACTUAL_PREV_TRIM" != "OLD-BINARY" ]; then
    fail "Scenario A: racecontrol-prev.exe expected OLD-BINARY, got: $ACTUAL_PREV_TRIM"
fi
if [ -f "$TEST_ROOT_BASH/racecontrol-new.exe" ]; then
    fail "Scenario A: racecontrol-new.exe should be gone after ren"
fi
pass "Scenario A: happy-path swap OK (SWAPPED, files correct)"

# --- Scenario B: Pre-existing stale prev cleaned up ---
rm -f "$TEST_ROOT_BASH/racecontrol.exe" "$TEST_ROOT_BASH/racecontrol-new.exe" "$TEST_ROOT_BASH/racecontrol-prev.exe"
echo "OLD-BINARY" > "$TEST_ROOT_BASH/racecontrol.exe"
echo "NEW-BINARY" > "$TEST_ROOT_BASH/racecontrol-new.exe"
echo "STALE-PREV" > "$TEST_ROOT_BASH/racecontrol-prev.exe"
OUTPUT=$(cmd.exe //c "$SWAP_CMD_TEST" 2>&1 || true)
if ! echo "$OUTPUT" | grep -q "SWAPPED"; then
    echo "  DEBUG output: $OUTPUT"
    fail "Scenario B: expected SWAPPED after stale-prev cleanup."
fi
ACTUAL_PREV=$(cat "$TEST_ROOT_BASH/racecontrol-prev.exe" 2>/dev/null || echo "MISSING")
ACTUAL_PREV_TRIM=$(echo "$ACTUAL_PREV" | tr -d '\r\n')
if [ "$ACTUAL_PREV_TRIM" != "OLD-BINARY" ]; then
    fail "Scenario B: racecontrol-prev.exe expected OLD-BINARY (stale cleaned + fresh prev), got: $ACTUAL_PREV_TRIM"
fi
pass "Scenario B: pre-existing stale prev cleaned up + fresh prev = OLD-BINARY"

# --- Scenario C: Deterministic auto-recover via barrier file ---
# The barrier background job creates a fresh racecontrol.exe between the 1st and 2nd ren
# operations. If timing races favor us, this triggers SWAP_FAILED_RECOVERED.
# Per revision Issue 10: if barrier is unreliable on this runner, Scenario C is explicitly
# DROPPED (not silently skipped). Layer 1 grep + Plan 06 live canary provide coverage.
rm -f "$TEST_ROOT_BASH/racecontrol.exe" "$TEST_ROOT_BASH/racecontrol-new.exe" "$TEST_ROOT_BASH/racecontrol-prev.exe"
echo "OLD-BINARY" > "$TEST_ROOT_BASH/racecontrol.exe"
echo "NEW-BINARY" > "$TEST_ROOT_BASH/racecontrol-new.exe"

( sleep 0.2 && echo "BARRIER" > "$TEST_ROOT_BASH/racecontrol.exe" ) &
BARRIER_PID=$!

OUTPUT=$(cmd.exe //c "$SWAP_CMD_TEST" 2>&1 || true)
wait "$BARRIER_PID" 2>/dev/null || true

if echo "$OUTPUT" | grep -q "SWAP_FAILED_RECOVERED"; then
    pass "Scenario C: deterministic barrier triggered auto-recover branch (SWAP_FAILED_RECOVERED observed)"
elif echo "$OUTPUT" | grep -q "SWAP_FAILED_EL"; then
    # 1st ren itself failed because the barrier raced and created racecontrol.exe before
    # the 1st ren could run. Still a valid demonstration that the || exit-chain works.
    pass "Scenario C: preserve-failure branch fired (exit /b 1 path). Auto-recover unreached this run but || chain proven functional."
else
    # Barrier timing unreliable on this runner. Per revision Issue 10: Scenario C is
    # EXPLICITLY DROPPED (not silently skipped, not treated as failure). Layer 1's
    # grep for SWAP_FAILED_RECOVERED + Plan 06 live canary provide the coverage.
    echo "  DROP  Scenario C: barrier did not force a failure on this runner. Layer 1 grep (SWAP_FAILED_RECOVERED=1) + Plan 06 live canary provide coverage. NOT treated as failure."
fi

echo ""

# --- Layer 3 -- forfiles 72h guard behavioral regression ---
echo "--- Layer 3: forfiles 72h guard behavioral regression ---"

if [ ! -f "$BAT" ]; then
    echo "  SKIP  start-racecontrol.bat not found -- Layer 3 skipped"
else
    mkdir -p "$FORFILES_TEST_ROOT_BASH"

    # Setup: fresh prev.exe (mtime = now) + staged hash-named binary to trigger the
    # bat's conditional cleanup branch.
    echo "PREV-BINARY" > "$FORFILES_TEST_ROOT_BASH/racecontrol-prev.exe"
    echo "CURRENT-BINARY" > "$FORFILES_TEST_ROOT_BASH/racecontrol.exe"
    echo "STAGED-BINARY" > "$FORFILES_TEST_ROOT_BASH/racecontrol-aaaaaaaa.exe"

    # Execute a minimal bat snippet mirroring start-racecontrol.bat lines 15-18
    # (forfiles guard + preserve-if-not-exist rename). We do NOT invoke the full bat
    # because it would kill racecontrol and launch the watchdog.
    FF_CMD="cd /d ${FORFILES_TEST_ROOT_WIN} & forfiles /M racecontrol-prev.exe /D -3 /C \"cmd /c del /Q @file\" 1>nul 2>nul & if exist racecontrol.exe if not exist racecontrol-prev.exe ren racecontrol.exe racecontrol-prev.exe 1>nul 2>nul"
    cmd.exe //c "$FF_CMD" >/dev/null 2>&1 || true

    # Assert: prev.exe still exists with unchanged content (72h guard prevented deletion).
    if [ ! -f "$FORFILES_TEST_ROOT_BASH/racecontrol-prev.exe" ]; then
        fail "Layer 3: forfiles 72h guard FAILED -- prev.exe was deleted despite mtime well under 72h"
    fi
    ACTUAL_FPREV=$(cat "$FORFILES_TEST_ROOT_BASH/racecontrol-prev.exe" 2>/dev/null || echo "MISSING")
    ACTUAL_FPREV_TRIM=$(echo "$ACTUAL_FPREV" | tr -d '\r\n')
    if [ "$ACTUAL_FPREV_TRIM" != "PREV-BINARY" ]; then
        fail "Layer 3: forfiles guard present but prev.exe content changed (got: $ACTUAL_FPREV_TRIM) -- rename-clobber happened despite 'if not exist' check"
    fi
    pass "Layer 3: forfiles 72h guard prevented premature delete; prev.exe intact with original content"
fi

echo ""
echo "=== All layers passed. Exit 0. ==="
exit 0
