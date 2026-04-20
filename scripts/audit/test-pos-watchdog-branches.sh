#!/usr/bin/env bash
# test-pos-watchdog-branches.sh - Active-branch soak test for POS Chrome watchdog.
#
# Exercises the 3 fix-active branches of pos-watchdog.ps1 that were UNVERIFIED
# in the 2026-04-20 soak (Edge orphan fix handoff). Each test requires user
# authorization because it briefly disrupts a live POS service.
#
# Branches tested:
#   A) msedge-kill branch   - launch a dummy msedge, verify watchdog kills it
#                             within 2 min + writes "WARN: msedge orphan(s)
#                             detected" to pos-watchdog.log. NON-DESTRUCTIVE to
#                             billing UI (dummy msedge is a new process, not
#                             the Chrome billing kiosk).
#
#   B) rc-agent-restart     - taskkill rc-agent.exe, verify it respawns within
#                             2 min via start-rcagent.bat. POS rc-agent has all
#                             modules disabled (role=pos, not pod) so killing
#                             it only downs :8090 exec endpoint for ~90s;
#                             billing UI (Chrome -> server :3200) is unaffected.
#
#   C) Chrome-launch branch - taskkill Chrome billing kiosk, verify watchdog
#                             relaunches it with billing URL within 2 min.
#                             DESTRUCTIVE: billing UI blank for ~90-120s.
#                             REQUIRES EXPLICIT USER AUTHORIZATION.
#
# Usage:
#   ./test-pos-watchdog-branches.sh --branch A              # msedge-kill only
#   ./test-pos-watchdog-branches.sh --branch B              # rc-agent only
#   ./test-pos-watchdog-branches.sh --branch C --auth-chrome-outage  # Chrome (destructive)
#   ./test-pos-watchdog-branches.sh --branch ALL --auth-chrome-outage  # all 3
#
# Evidence captured per branch:
#   - pre-state tasklist counts
#   - disruption command + timestamp
#   - post-state tasklist counts at t+10s, t+60s, t+150s
#   - pos-watchdog.log tail for the test window
#
# Prerequisites: SSH key auth to POS@192.168.31.20 (from James .27).

set -u

BRANCH=""
AUTH_CHROME_OUTAGE=0
POS_HOST="POS@192.168.31.20"

while [ $# -gt 0 ]; do
    case "$1" in
        --branch) BRANCH="$2"; shift 2 ;;
        --auth-chrome-outage) AUTH_CHROME_OUTAGE=1; shift ;;
        -h|--help) sed -n '2,/^set -u/p' "$0" | sed 's/^#//'; exit 0 ;;
        *) echo "Unknown arg: $1" >&2; exit 2 ;;
    esac
done

if [ -z "$BRANCH" ]; then
    echo "ERROR: --branch is required (A | B | C | ALL)" >&2
    exit 2
fi

case "$BRANCH" in
    A|B|C|ALL) ;;
    *) echo "ERROR: --branch must be A, B, C, or ALL" >&2; exit 2 ;;
esac

# Chrome-outage authorization gate
if { [ "$BRANCH" = "C" ] || [ "$BRANCH" = "ALL" ]; } && [ "$AUTH_CHROME_OUTAGE" -eq 0 ]; then
    echo "ERROR: branch C requires --auth-chrome-outage (billing UI down ~2 min)" >&2
    exit 2
fi

log() { echo "[$(date +%H:%M:%S)] $*"; }

run_on_pos() { ssh -o ConnectTimeout=5 "$POS_HOST" "$@"; }

pre_state() {
    log "--- pre-state ---"
    run_on_pos 'tasklist /NH | findstr /IC:"chrome.exe" /C:"msedge.exe" /C:"rc-agent.exe" | find /V /C "~~~"'
    run_on_pos 'powershell -NoProfile -Command "\"chrome=\"+(Get-Process chrome -EA 0 | Measure-Object).Count;\"msedge=\"+(Get-Process msedge -EA 0 | Measure-Object).Count;\"rc-agent=\"+(Get-Process rc-agent -EA 0 | Measure-Object).Count"'
}

sample_state() {
    local tag="$1"
    log "--- sample $tag ---"
    run_on_pos 'powershell -NoProfile -Command "\"chrome=\"+(Get-Process chrome -EA 0 | Measure-Object).Count;\"msedge=\"+(Get-Process msedge -EA 0 | Measure-Object).Count;\"rc-agent=\"+(Get-Process rc-agent -EA 0 | Measure-Object).Count"'
}

tail_log() {
    log "--- pos-watchdog.log (last 10 lines) ---"
    run_on_pos 'powershell -NoProfile -Command "Get-Content C:\RacingPoint\pos-watchdog.log -Tail 10"'
}

test_A_msedge_kill() {
    log "=== Branch A: msedge-kill ==="
    pre_state
    log "Launching dummy msedge on POS..."
    run_on_pos 'powershell -NoProfile -Command "Start-Process msedge.exe -ArgumentList \"about:blank\" -WindowStyle Hidden"' || log "WARN: msedge launch returned non-zero"
    sleep 5
    sample_state "t+5s (dummy should be alive)"
    log "Waiting 150s for watchdog to fire..."
    sleep 150
    sample_state "t+155s (msedge should be gone if watchdog killed it)"
    tail_log
    log "EXPECT: 'WARN: msedge orphan(s) detected (PIDs X) - killing' line within test window."
}

test_B_rc_agent_restart() {
    log "=== Branch B: rc-agent-restart ==="
    pre_state
    log "Killing rc-agent.exe on POS..."
    run_on_pos 'taskkill /F /IM rc-agent.exe' || log "WARN: taskkill returned non-zero"
    sleep 10
    sample_state "t+10s (rc-agent should be dead)"
    log "Waiting 150s for watchdog to fire..."
    sleep 150
    sample_state "t+160s (rc-agent should be respawned)"
    tail_log
    log "EXPECT: 'rc-agent NOT running, restarting via start-rcagent.bat' line within test window."
    log "Recovery: if rc-agent did NOT restart after 3 min, run: ssh $POS_HOST 'C:\\RacingPoint\\start-rcagent.bat'"
}

test_C_chrome_launch() {
    log "=== Branch C: Chrome-launch (DESTRUCTIVE - billing UI down ~2 min) ==="
    pre_state
    log "Killing Chrome on POS... (billing UI will be blank)"
    run_on_pos 'taskkill /F /IM chrome.exe' || log "WARN: taskkill returned non-zero"
    sleep 10
    sample_state "t+10s (chrome should be dead, UI blank)"
    log "Waiting 150s for watchdog to fire..."
    sleep 150
    sample_state "t+160s (chrome should be relaunched with billing URL)"
    tail_log
    log "EXPECT: 'Chrome kiosk NOT running, restarting with billing URL' + 'Chrome kiosk restarted with billing URL' lines."
    log "MANUAL VERIFY: operator should confirm billing UI is visible on POS screen."
    log "Recovery: if Chrome did NOT relaunch, run: ssh $POS_HOST 'schtasks /Run /TN RestartKiosk'"
}

case "$BRANCH" in
    A)   test_A_msedge_kill ;;
    B)   test_B_rc_agent_restart ;;
    C)   test_C_chrome_launch ;;
    ALL) test_A_msedge_kill; echo; test_B_rc_agent_restart; echo; test_C_chrome_launch ;;
esac

log "Done."
