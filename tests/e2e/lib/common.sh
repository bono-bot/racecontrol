#!/bin/bash
# lib/common.sh
# Shared helpers for all RaceControl E2E test scripts.
# Source this file at the top of every .sh test script.
# DO NOT add 'set' options here — let callers manage their own error handling.

# Colors — only emit ANSI codes when stdout is a terminal
if [ -t 1 ]; then
    GREEN='\033[0;32m'
    RED='\033[0;31m'
    YELLOW='\033[0;33m'
    CYAN='\033[0;36m'
    NC='\033[0m'
else
    GREEN='' RED='' YELLOW='' CYAN='' NC=''
fi

# Counters
PASS=0
FAIL=0
SKIP=0

pass() { PASS=$((PASS+1)); echo -e "  ${GREEN}PASS${NC}  $1"; }
fail() { FAIL=$((FAIL+1)); echo -e "  ${RED}FAIL${NC}  $1"; }
skip() { SKIP=$((SKIP+1)); echo -e "  ${YELLOW}SKIP${NC}  $1"; }
info() { echo -e "  ${CYAN}INFO${NC}  $1"; }

# Call as the last line of every test script.
# Prints a summary and exits with the number of failures.
summary_exit() {
    local total=$((PASS + FAIL + SKIP))
    echo ""
    echo "========================================"
    echo -e "Results: ${GREEN}${PASS} passed${NC}, ${RED}${FAIL} failed${NC}, ${YELLOW}${SKIP} skipped${NC} (${total} total)"
    echo "========================================"
    if [ "$FAIL" -gt 0 ]; then
        echo -e "${RED}FAILED${NC}"
    else
        echo -e "${GREEN}PASSED${NC}"
    fi
    exit "$FAIL"
}
