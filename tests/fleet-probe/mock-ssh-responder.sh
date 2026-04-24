#!/bin/bash
# tests/fleet-probe/mock-ssh-responder.sh -- Phase 448 Plan 01
# Stand-in for the ssh binary; reads PROBE_SSH_SCENARIO file:
#   - Lines before "---EXIT---" marker -> written to stdout
#   - First non-empty line after "---EXIT---" marker -> used as exit code
# Usage: Set PROBE_SSH_SCENARIO=/path/to/scenario.txt; probe scripts call $PROBE_SSH in place of ssh.
set -eo pipefail

if [ -z "${PROBE_SSH_SCENARIO:-}" ] || [ ! -f "$PROBE_SSH_SCENARIO" ]; then
  echo "mock-ssh-responder: PROBE_SSH_SCENARIO not set or file missing" >&2
  exit 2
fi

# Output lines before ---EXIT--- marker to stdout
awk '/^---EXIT---$/ { exit_section=1; next } !exit_section { print }' "$PROBE_SSH_SCENARIO"

# Extract exit code from first non-empty line after ---EXIT--- marker
EXIT_CODE=$(awk '/^---EXIT---$/ { exit_section=1; next } exit_section && NF { print; exit }' "$PROBE_SSH_SCENARIO" | head -1)
exit "${EXIT_CODE:-0}"
