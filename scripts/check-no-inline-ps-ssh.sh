#!/usr/bin/env bash
# check-no-inline-ps-ssh.sh — mechanical gate against inline-PowerShell-over-SSH WRITES.
#
# RCA 2026-06-01 (.planning/audits/RCA-windows-registry-powershell-deploy-2026-06-01.md) §3:
# the 2026-04-08 fleet-wipe rule ("NEVER edit remote files via inline PowerShell over SSH")
# was PROSE-ONLY and the banned pattern proliferated to 6 files. Encoding rules that became
# MECHANICAL (.gitattributes) held with zero recurrence. This gate makes the inline-PS-over-SSH
# rule mechanical too.
#
# Detects Tier-A (dangerous WRITE) inline PS over SSH / rc-sentry /exec:
#   ssh ... "powershell ... (Set-Content|Out-File|Remove-Item|Rename-Item|New-Item)"
#   /exec  ... "cmd":"powershell ... (Set-Content|Remove-Item|Rename-Item)"
# Tier-B read-only inline PS (Get-Content/Test-Path/Get-Item/Get-Process) is NOT flagged.
#
# BASELINE-AWARE: known pre-existing Tier-A files are recorded below as P2 debt; the gate
# BLOCKS only NEW violations (files not in the baseline). Fix a baseline file -> remove it here.
#
# Exit 0 = clean (no new violations). Exit 1 = NEW inline-PS-over-SSH write introduced.
# Wire into test/run-all.sh security suite + .git/hooks/pre-commit.
set -uo pipefail
cd "$(dirname "$0")/.." || exit 2

# P2 debt baseline — Tier-A files known at RCA time (2026-06-01). Convert per RCA §5 P2; then delete the line.
BASELINE=(
  "scripts/deploy-sentry.sh"
  "scripts/deploy/deploy-nextjs.sh"
  "scripts/audit/cleanup-pod-edge-stale-ps1.sh"
  "scripts/audit/cleanup-pos-edge-backups.sh"
)

# Tier-A detector: a line that pipes/calls a PS WRITE verb through ssh OR an /exec json body.
# Two-stage: line must mention powershell AND a write verb AND (ssh OR /exec OR $*_SSH).
PS_WRITE='Set-Content|Out-File|Remove-Item|Rename-Item|New-Item -ItemType'
HITS=$(grep -rnE "powershell" scripts deploy-staging 2>/dev/null \
  | grep -vE '\.(md|json|txt):' \
  | grep -v 'check-no-inline-ps-ssh.sh' \
  | grep -iE "$PS_WRITE" \
  | grep -iE 'ssh |_SSH |/exec|"cmd"' || true)

VIOLATIONS=""
while IFS= read -r line; do
  [ -z "$line" ] && continue
  file="${line%%:*}"
  skip=0
  for b in "${BASELINE[@]}"; do [ "$file" = "$b" ] && skip=1 && break; done
  [ "$skip" -eq 1 ] && continue
  VIOLATIONS+="$line"$'\n'
done <<< "$HITS"

if [ -n "${VIOLATIONS// /}" ]; then
  echo "FAIL: NEW inline-PowerShell-over-SSH WRITE detected (banned per 2026-04-08 fleet-wipe RCA):"
  echo "$VIOLATIONS"
  echo "Fix: write a .ps1 locally (CRLF via .gitattributes) -> scp -> ssh host 'powershell -ExecutionPolicy Bypass -File script.ps1'."
  echo "See .planning/audits/RCA-windows-registry-powershell-deploy-2026-06-01.md §4-§5."
  exit 1
fi

echo "OK: no new inline-PS-over-SSH writes. (baseline P2 debt: ${#BASELINE[@]} files tracked in RCA §5 P2)"
exit 0
