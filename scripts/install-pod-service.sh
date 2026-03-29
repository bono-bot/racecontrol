#!/bin/bash
# install-pod-service.sh — Install rc-sentry as Windows Service on pods
# MMA consensus fix (5/5 models): WTSQueryUserToken requires SYSTEM context
#
# What this does:
#   1. Sets AutoAdminLogon registry (ensures Session 1 on boot)
#   2. Installs rc-sentry.exe as a Windows Service (SYSTEM, auto-start)
#   3. Removes rc-sentry HKLM Run key (service replaces it)
#   4. Keeps rc-agent HKLM Run key as backup (service is primary)
#
# Usage:
#   bash scripts/install-pod-service.sh                    # All 8 pods
#   bash scripts/install-pod-service.sh 192.168.31.89      # Single pod
#   bash scripts/install-pod-service.sh --dry-run           # Preview only
#
# Architecture after fix:
#   Boot → SYSTEM Service (Session 0) → rc-sentry watchdog
#     → polls rc-agent health every 5s
#     → on failure: WTSQueryUserToken + CreateProcessAsUser → rc-agent in Session 1
#   Boot → AutoAdminLogon → explorer.exe (Session 1) → HKLM Run → start-rcagent.bat (backup)

set -euo pipefail

ALL_PODS=(
  "pod1:192.168.31.89"
  "pod2:192.168.31.33"
  "pod3:192.168.31.28"
  "pod4:192.168.31.88"
  "pod5:192.168.31.86"
  "pod6:192.168.31.87"
  "pod7:192.168.31.38"
  "pod8:192.168.31.91"
)

DRY_RUN=0
TARGETS=()

# Parse args
while [[ $# -gt 0 ]]; do
  case "$1" in
    --dry-run) DRY_RUN=1; shift ;;
    *) TARGETS+=("custom:$1"); shift ;;
  esac
done
[[ ${#TARGETS[@]} -eq 0 ]] && TARGETS=("${ALL_PODS[@]}")

run_ssh() {
  local ip="$1"
  local cmd="$2"
  if [ "$DRY_RUN" -eq 1 ]; then
    echo "  [DRY-RUN] ssh User@${ip} \"${cmd}\""
    return 0
  fi
  ssh -o ConnectTimeout=5 -o BatchMode=yes "User@${ip}" "$cmd" 2>&1 | \
    grep -v "WARNING\|vulnerable\|upgraded\|openssh" || true
}

echo "=== Pod Service Installer (MMA fix) ==="
echo "Targets: ${#TARGETS[@]} pods | Dry run: ${DRY_RUN}"
echo ""

SUCCESS=0
FAIL=0

for target in "${TARGETS[@]}"; do
  name="${target%%:*}"
  ip="${target##*:}"

  echo "--- ${name} (${ip}) ---"

  # Step 1: AutoAdminLogon
  echo "  [1/4] Setting AutoAdminLogon..."
  run_ssh "$ip" "reg add \"HKLM\\SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\Winlogon\" /v AutoAdminLogon /t REG_SZ /d 1 /f 2>nul & reg add \"HKLM\\SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\Winlogon\" /v DefaultUserName /t REG_SZ /d user /f 2>nul & reg add \"HKLM\\SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\Winlogon\" /v DefaultPassword /t REG_SZ /d \"\" /f 2>nul & reg add \"HKLM\\SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\Winlogon\" /v EnableFirstLogonAnimation /t REG_DWORD /d 0 /f 2>nul"

  # Step 2: Stop existing rc-sentry process (it's running in Session 1, wrong context)
  echo "  [2/4] Stopping user-mode rc-sentry..."
  run_ssh "$ip" "taskkill /F /IM rc-sentry.exe 2>nul & timeout /t 2 >nul"

  # Step 3: Create Windows Service
  echo "  [3/4] Installing rc-sentry as Windows Service..."
  # First remove if exists (idempotent)
  run_ssh "$ip" "sc stop RCSentry 2>nul & sc delete RCSentry 2>nul & timeout /t 2 >nul"
  # Create with auto-start as LocalSystem
  run_ssh "$ip" "sc create RCSentry binPath= \"C:\\RacingPoint\\rc-sentry.exe\" start= auto DisplayName= \"RC Sentry Watchdog\" obj= LocalSystem 2>nul & sc failure RCSentry reset= 86400 actions= restart/5000/restart/10000/restart/30000 2>nul & sc start RCSentry 2>nul"

  # Step 4: Remove rc-sentry from HKLM Run (service replaces it)
  # Keep rc-agent Run key as backup
  echo "  [4/4] Removing rc-sentry HKLM Run key..."
  run_ssh "$ip" "reg delete \"HKLM\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Run\" /v RCSentry /f 2>nul"

  # Verify
  echo "  Verifying..."
  SVC_STATE=$(run_ssh "$ip" "sc query RCSentry 2>nul | findstr STATE" | grep -oP '\d+\s+\w+' | head -1)
  echo "  Service state: ${SVC_STATE:-UNKNOWN}"

  if echo "$SVC_STATE" | grep -qi "RUNNING"; then
    echo "  OK"
    SUCCESS=$((SUCCESS + 1))
  else
    echo "  WARN: Service not running yet"
    FAIL=$((FAIL + 1))
  fi
  echo ""
done

echo "=== Results: ${SUCCESS} OK, ${FAIL} issues ==="
echo ""
echo "Next steps:"
echo "  1. Wait 15-20s for watchdog to detect rc-agent is down"
echo "  2. Watchdog will call spawn_in_session1() with SYSTEM privileges"
echo "  3. rc-agent should start in Session 1 automatically"
echo "  4. Verify: bash scripts/wait-for-pods.sh --pods-only"
