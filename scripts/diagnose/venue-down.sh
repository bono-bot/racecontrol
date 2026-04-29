#!/bin/bash
# venue-down.sh — Canonical workflow when "kiosk/admin/venue is down"
#
# Run from James (.27). Outputs raw observations only — no verdicts.
# Codifies the order of operations missed in 2026-04-28 debug session
# (see feedback_g9_venue_down_diagnose_workflow_20260428.md).
#
# Usage:  bash scripts/diagnose/venue-down.sh
#
# Order is intentional. Do not reorder.

set -u
SERVER_HOST="${SERVER_HOST:-192.168.31.23}"
SSH_ALIAS="${SSH_ALIAS:-server}"  # ~/.ssh/config sets ADMIN@ + key
TOML_PATH="${TOML_PATH:-D:/racecontrol.toml}"
TIMEOUT=5

hr() { printf '\n=== %s ===\n' "$1"; }

# Step 0 — Memory / LOGBOOK / git / INBOX (text-only; do not skip)
hr "STEP 0 — prior-art (read these before acting)"
echo "memory:    ls ~/.claude/projects/C--Users-bono/memory/ | grep -iE 'admin|kiosk|3201|venue.down'"
echo "logbook:   grep -iE 'admin.down|kiosk.down|3201|watchdog' racecontrol/LOGBOOK.md | tail -10"
echo "git:       git -C racingpoint/racecontrol log --oneline --since=7.days | head -20"
echo "inbox:     head -80 racingpoint/comms-link/INBOX.md"
echo "errcat:    grep -iE 'admin|kiosk|3201|watchdog' racingpoint/racecontrol/docs/ERROR-CATALOG.md"
echo "graphify:  query_graph('<symptom keyword>') on racecontrol graph"

# Step 1 — Canonical port matrix from racecontrol.toml + reference_fleet_port_map.md
hr "STEP 1 — canonical ports (read from $TOML_PATH)"
if [ -r "$TOML_PATH" ]; then
  grep -E '^\s*(host|port)\s*=' "$TOML_PATH" | head -10
else
  echo "WARN: $TOML_PATH not readable from this host — fall back to memory map"
fi
echo "reference (CLAUDE.md Server Services): :80 (frontend) :3200 (web dashboard) :3201 (admin) :3300 (kiosk) :8080 (racecontrol HTTP) :8090 (server_ops, same binary) :8091 (rc-sentry on pods, NOT server)"

# Step 2 — Host reachability
hr "STEP 2 — host reachability"
ping -n 2 "$SERVER_HOST" 2>/dev/null | grep -E "Reply|Lost" || ping -c 2 "$SERVER_HOST"

# Step 3 — Local probe matrix (from James — does NOT prove kiosk works for customer)
hr "STEP 3 — local probe matrix from $(hostname) (NOT customer-side evidence)"
for p in 80 3200 3201 3300 8080 8090 8091 5173; do
  code=$(curl -s -o /dev/null -w '%{http_code}' --max-time $TIMEOUT --connect-timeout $TIMEOUT "http://${SERVER_HOST}:${p}/" 2>/dev/null || echo "ERR")
  printf "  port %-5s -> %s\n" "$p" "$code"
done
echo
for path in /status / /admin /admin/ /portal /kiosk /kiosk/ /api/health /api/auth/login /api/admin/health; do
  code=$(curl -s -o /dev/null -w '%{http_code}' --max-time $TIMEOUT --connect-timeout $TIMEOUT "http://${SERVER_HOST}${path}" 2>/dev/null || echo "ERR")
  printf "  %-22s -> %s\n" "$path" "$code"
done

# Step 4 — Trace redirects (where does /admin and /kiosk actually go?)
hr "STEP 4 — redirect chains"
for path in /admin /kiosk/; do
  printf "  %s : " "$path"
  curl -s -L --max-redirs 5 --max-time $TIMEOUT -o /dev/null \
    -w 'final=%{url_effective} code=%{http_code} redirs=%{num_redirects}\n' \
    "http://${SERVER_HOST}${path}"
done

# Step 5 — Server-side enumeration (requires SSH; prod-SSH read authorization)
hr "STEP 5 — server-side state via 'ssh $SSH_ALIAS' (prod read)"
ssh -o ConnectTimeout=8 "$SSH_ALIAS" "powershell -NoProfile -Command \"\
Write-Output '--- LISTENERS ---'; \
Get-NetTCPConnection -State Listen | Where-Object {\$_.LocalPort -in 80,3200,3201,3300,8080,8090,8091,5173} | Sort LocalPort | Select LocalAddress,LocalPort,OwningProcess | Format-Table -AutoSize | Out-String; \
Write-Output '--- PROCESSES ---'; \
Get-Process | Where-Object {\$_.ProcessName -match 'rc-|admin|node|racecontrol|rc_|frontend-watchdog'} | Select Name,Id,StartTime,CPU,Responding | Format-Table -AutoSize | Out-String; \
Write-Output '--- SCHEDULED TASKS ---'; \
Get-ScheduledTask | Where-Object {\$_.TaskName -match 'racecontrol|rc-|admin|watchdog'} | Select TaskName,State | Format-Table -AutoSize | Out-String; \
Write-Output '--- ADMIN LOG TAIL ---'; \
foreach (\$d in @('D:\\racecontrol\\logs','C:\\racecontrol\\logs','D:\\admin-panel\\logs','C:\\admin-panel\\logs')) { foreach (\$pat in @('admin*.log','frontend-watchdog*.log','admin*.out','admin*.err')) { Get-ChildItem \$d -ErrorAction SilentlyContinue -Filter \$pat | Sort LastWriteTime -Descending | Select -First 1 | ForEach-Object { Write-Output ('=== ' + \$_.FullName + ' ==='); Get-Content \$_.FullName -Tail 30 -ErrorAction SilentlyContinue } } }\
\"" 2>&1 | head -120

# Step 6 — Customer-side evidence (CGP H3: kiosk evidence MUST come from kiosk machine)
hr "STEP 6 — customer-side evidence reminder (NOT auto-run)"
echo "POS browser (chrome :9222):  see reference_pos_chrome_kiosk.md"
echo "  ssh server -L 9222:localhost:9222   then  chrome-devtools-mcp screenshot"
echo "Pod kiosk (rc-watchdog UI):   ssh pod-N    then  Get-Process rc-watchdog"
echo "James-local curl results above are NOT kiosk customer evidence."

hr "DONE — observations only; do not infer verdicts without per-target evidence"
