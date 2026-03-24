#!/usr/bin/env bash
# Standing Rules Compliance Check
# Usage: bash check-rules-compliance.sh
# Exits 0 if all repos compliant, 1 otherwise

BASE="C:/Users/bono/racingpoint"
FAILURES=()

check_section() {
  local repo="$1"
  local section="$2"
  local claude_md="$BASE/$repo/CLAUDE.md"

  if [ ! -f "$claude_md" ]; then
    FAILURES+=("$repo: CLAUDE.md missing entirely")
    return
  fi

  if ! grep -q "^### $section" "$claude_md" 2>/dev/null; then
    FAILURES+=("$repo: missing '### $section' section")
  fi
}

# Node.js repos: Code Quality + Process + Comms
for repo in comms-link racingpoint-admin racingpoint-api-gateway racingpoint-discord-bot \
            racingpoint-google racingpoint-mcp-calendar racingpoint-mcp-drive \
            racingpoint-mcp-gmail racingpoint-mcp-sheets racingpoint-whatsapp-bot \
            rc-ops-mcp whatsapp-bot people-tracker; do
  check_section "$repo" "Code Quality"
  check_section "$repo" "Process"
  check_section "$repo" "Comms"
done

# Rust repo: Code Quality + Deploy + Debugging
check_section "pod-agent" "Code Quality"
check_section "pod-agent" "Deploy"
check_section "pod-agent" "Debugging"

# Ops repo: Deploy + Process
check_section "deploy-staging" "Deploy"
check_section "deploy-staging" "Process"

# Report
if [ ${#FAILURES[@]} -eq 0 ]; then
  echo "All repos compliant"
  exit 0
else
  echo "COMPLIANCE FAILURES:"
  for f in "${FAILURES[@]}"; do
    echo "  - $f"
  done
  exit 1
fi
