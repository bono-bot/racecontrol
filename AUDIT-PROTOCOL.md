# Racing Point Operations Audit Protocol — 60 Phases

**Version:** 3.0 | **Created:** 2026-03-23 | **Updated:** 2026-03-25 | **Author:** James Vowles
**Coverage:** 100% — all 173+ runtime modules, 200+ standing rules, 241 API endpoints, 12 E2E journeys
**Standing Rule:** Run this audit before shipping any milestone, after major incidents, or weekly during operations.

**Prerequisites:**
- Convert ALL timestamps: racecontrol logs are UTC, operations are IST (UTC+5:30)
- Exclude your own actions from event counts (deploys, test kills)
- Fix during audit, don't just catalog — apply smallest reversible fix immediately
- Obtain auth session token for protected endpoints (used across multiple phases)

```bash
# Auth token — reuse across all phases that need it
SESSION=$(curl -s -X POST http://192.168.31.23:8080/api/v1/terminal/auth \
  -H "Content-Type: application/json" -d '{"pin":"261121"}' | jq -r '.session')

# Pod IP array — reuse everywhere
PODS="192.168.31.89 192.168.31.33 192.168.31.28 192.168.31.88 192.168.31.86 192.168.31.87 192.168.31.38 192.168.31.91"
```

---

# TIER 1: Infrastructure Foundation (Phases 1-10)

## Phase 1: Fleet Inventory
**What:** Every binary, build_id, uptime, process count across all machines.
**Verify:** All builds match expected HEAD. If stale, `touch build.rs` and rebuild.

```bash
# Server .23
curl -s http://192.168.31.23:8080/api/v1/health
curl -s http://192.168.31.23:8090/health

# All 8 pods — rc-agent :8090 + rc-sentry :8091
for IP in $PODS; do
  echo "=== $IP ===" && curl -s http://$IP:8090/health && curl -s http://$IP:8091/health
done

# James .27
node --version
curl -s http://localhost:11434/api/tags        # Ollama
curl -s http://localhost:8766/relay/health      # comms-link
curl -s http://localhost:1984/api/streams       # go2rtc

# Bono VPS
curl -s http://100.70.177.44:8080/api/v1/health
```

**Fix loop trigger:** Any build_id mismatch, any service DOWN.

---

## Phase 2: Config Integrity
**What:** All TOML config files are valid, not corrupted by SSH banners or stale edits.
**Standing rule:** "Never pipe SSH output into config files."

```bash
# Server .23 — verify TOML starts with [ (not SSH banner)
curl -s -X POST http://192.168.31.23:8090/exec \
  -d '{"cmd":"findstr /N /R \"^\" C:\\RacingPoint\\racecontrol.toml | findstr /R \"^1:\""}'

# Verify no conflicting duplicate keys (e.g., two enabled= lines in same section)
curl -s -X POST http://192.168.31.23:8090/exec \
  -d '{"cmd":"findstr /C:\"enabled\" C:\\RacingPoint\\racecontrol.toml"}'

# Pod TOML — verify pod_number matches actual pod
for IP in $PODS; do
  curl -s -X POST http://$IP:8090/exec \
    -d '{"cmd":"type C:\\RacingPoint\\rc-agent.toml"}'
done

# James — comms-link config
cat C:/Users/bono/racingpoint/comms-link/.env 2>/dev/null || echo "NO .env"
```

**Fix loop trigger:** TOML first line is not `[`, duplicate conflicting keys, pod_number mismatch.

---

## Phase 3: Network & Tailscale
**What:** Tailscale connected on all nodes. LAN connectivity to server. Tailscale mesh complete.
**Note:** 3 desktop-* nodes are leaderboard displays — do NOT flag as stale.

```bash
# James Tailscale — full status
tailscale status

# Verify server .23 reachable via LAN from all pods
for IP in $PODS; do
  curl -s -X POST http://$IP:8090/exec -d '{"cmd":"ping -n 1 192.168.31.23"}'
done

# Server .23 → Bono VPS via Tailscale
curl -s -X POST http://192.168.31.23:8090/exec \
  -d '{"cmd":"curl.exe -s -m 5 http://100.70.177.44:8080/api/v1/health"}'

# POS PC reachable
curl -s http://192.168.31.20:8090/health 2>/dev/null || echo "POS OFFLINE"

# Known leaderboard nodes (DO NOT REMOVE):
# desktop-e3dn32l (100.122.215.124), desktop-q1bbl73 (100.98.92.17), desktop-q2mcek4 (100.99.109.79)
```

**Fix loop trigger:** Tailscale offline on any node, server unreachable from any pod, Bono VPS unreachable.

---

## Phase 4: Firewall & Port Security
**What:** Windows Firewall enabled on all profiles. Only expected ports open.

```bash
# Server .23 — firewall status
curl -s -X POST http://192.168.31.23:8090/exec \
  -d '{"cmd":"netsh advfirewall show allprofiles state"}'

# Server .23 — listening ports (expected: 8080, 8090, 3200, 3300)
curl -s -X POST http://192.168.31.23:8090/exec \
  -d '{"cmd":"netstat -an | findstr LISTENING | findstr /R \"8080 8090 3200 3300\""}'

# James — listening ports (expected: 8766, 1984, 11434, 9998, 9999)
netstat -an | grep LISTEN | grep -E "8766|1984|11434|9998|9999"

# Pods — only 8090 (agent) + 8091 (sentry) expected
for IP in $PODS; do
  curl -s -X POST http://$IP:8090/exec \
    -d '{"cmd":"netstat -an | findstr LISTENING"}'
done
```

**Fix loop trigger:** Firewall disabled on any profile, unexpected ports listening.

---

## Phase 5: Pod Power & WoL
**What:** All 8 pods are powered on. WoL capability verified for offline pods.

```bash
# Quick ping sweep — all 8 pods
for IP in $PODS; do
  curl -s -m 3 http://$IP:8090/health > /dev/null 2>&1 && echo "$IP: UP" || echo "$IP: DOWN"
done

# For any DOWN pod — attempt WoL via server
# (requires MAC addresses from CLAUDE.md Network Map)
# curl -s -X POST http://192.168.31.23:8080/api/v1/fleet/wol -d '{"pod_number": N}'

# Verify uptime on all online pods (detect recent unexpected reboots)
for IP in $PODS; do
  curl -s http://$IP:8090/health | jq '{ip: "'$IP'", uptime_secs: .uptime_secs}'
done
```

**Fix loop trigger:** Any pod DOWN without known reason. Any pod uptime < 300s (unexpected reboot).

---

## Phase 6: Orphan Processes
**What:** No leaked PowerShell, Variable_dump, stale game processes, or duplicate agents.
**Standing rule:** "PowerShell DETACHED_PROCESS leaks ~90MB per restart."

```bash
# Check for orphan PowerShell on all pods (should be 0-1)
for IP in $PODS; do
  echo "=== $IP ===" && curl -s -X POST http://$IP:8090/exec \
    -d '{"cmd":"tasklist /NH | find /C \"powershell.exe\""}'
done

# Check for Variable_dump.exe (should be killed on boot)
for IP in $PODS; do
  curl -s -X POST http://$IP:8090/exec \
    -d '{"cmd":"tasklist /NH | findstr /I \"Variable_dump\""}'
done

# Duplicate rc-agent instances (should be exactly 1)
for IP in $PODS; do
  curl -s -X POST http://$IP:8090/exec \
    -d '{"cmd":"tasklist /NH | find /C \"rc-agent.exe\""}'
done

# Server — orphan watchdog PowerShell (should be 0-1)
curl -s -X POST http://192.168.31.23:8090/exec \
  -d '{"cmd":"tasklist /NH | find /C \"powershell.exe\""}'
```

**Fix loop trigger:** PowerShell > 1 on any pod, Variable_dump running, duplicate rc-agent.

---

## Phase 7: Process Guard & Allowlist
**What:** Guard scanning, violation count trending down, allowlist populated.

```bash
# Pod violations (24h counter — should be declining, not 100+ everywhere)
curl -s http://192.168.31.23:8080/api/v1/fleet/health | jq '.pods[] | {pod_number, violation_count_24h}'

# Allowlist count per pod (should be > 100 entries if populated)
for N in 1 2 3 4 5 6 7 8; do
  COUNT=$(curl -s http://192.168.31.23:8080/api/v1/guard/whitelist/pod-$N | jq 'length')
  echo "Pod $N allowlist: $COUNT entries"
done

# Server-guard violations in logs
curl -s "http://192.168.31.23:8080/api/v1/logs?level=warn&lines=30" | grep "server-guard"

# Verify Variable_dump.exe is NOT in allowlist
curl -s http://192.168.31.23:8080/api/v1/guard/whitelist/pod-1 | jq '.[] | select(test("Variable_dump"; "i"))'
```

**Fix loop trigger:** All pods at violation_count=100 (empty allowlist), Variable_dump in allowlist, server-guard violations.

---

## Phase 8: Sentinel Files & Stale State
**What:** No stale MAINTENANCE_MODE, GRACEFUL_RELAUNCH, or restart sentinels.
**Standing rule:** "MAINTENANCE_MODE is a silent pod killer."

```bash
for IP in $PODS; do
  echo "=== $IP ===" && curl -s -X POST http://$IP:8091/exec \
    -d '{"cmd":"dir C:\\RacingPoint\\MAINTENANCE_MODE C:\\RacingPoint\\GRACEFUL_RELAUNCH C:\\RacingPoint\\rcagent-restart-sentinel.txt 2>nul || echo CLEAN"}'
done

# Also check server
curl -s -X POST http://192.168.31.23:8090/exec \
  -d '{"cmd":"dir C:\\RacingPoint\\MAINTENANCE_MODE 2>nul || echo CLEAN"}'
```

**Fix loop trigger:** Any sentinel file present — delete immediately, then restart rc-agent via schtasks.

---

## Phase 9: Self-Monitor & Self-Heal
**What:** rc-agent's self_monitor, self_heal, and failure_monitor modules are active.

```bash
# Check rc-agent logs for self-monitor heartbeat (should be recent)
for IP in $PODS; do
  curl -s -X POST http://$IP:8090/exec \
    -d '{"cmd":"findstr /C:\"self_monitor\" C:\\RacingPoint\\rc-agent-*.jsonl 2>nul | findstr /C:\"heartbeat\" | findstr /V /C:\"debug\" "}'
done

# Check failure_monitor — no pods in repeated crash loop
for IP in $PODS; do
  curl -s -X POST http://$IP:8090/exec \
    -d '{"cmd":"findstr /C:\"failure_monitor\" /C:\"restart_count\" C:\\RacingPoint\\rc-agent-*.jsonl 2>nul"}'
done

# Verify safe_mode is NOT active on any pod
for IP in $PODS; do
  curl -s -X POST http://$IP:8090/exec \
    -d '{"cmd":"findstr /C:\"safe_mode\" /C:\"SAFE_MODE\" C:\\RacingPoint\\rc-agent-*.jsonl 2>nul | findstr /V /C:\"disabled\""}'
done
```

**Fix loop trigger:** No heartbeat in last 10 min, safe_mode active, restart_count > 3.

---

## Phase 10: AI Healer / Watchdog
**What:** rc-watchdog monitoring all 10 services, failure state clean, Ollama responsive.

```bash
# Failure state
cat C:/Users/bono/.claude/watchdog-state.json

# Recent healer runs (today)
grep -E "INFO|WARN" C:/Users/bono/.claude/rc-watchdog.log.$(date +%Y-%m-%d) | tail -10

# Verify 10 services checked
grep "starting check run" C:/Users/bono/.claude/rc-watchdog.log.$(date +%Y-%m-%d) | tail -1

# Ollama responding (used by AI healer for diagnosis)
curl -s http://localhost:11434/api/tags | jq '.models | length'

# Verify Ollama has expected models (qwen2.5:3b, llama3.1:8b)
curl -s http://localhost:11434/api/tags | jq '.models[].name'
```

**Fix loop trigger:** Any service in failure state, less than 10 services checked, Ollama down or missing models.

---

# TIER 2: Core Services (Phases 11-16)

## Phase 11: API Data Integrity
**What:** Every API endpoint returns correct DATA, not just HTTP 200.
**Standing rule:** "Verify the EXACT behavior path, not proxies."

```bash
# Fleet health — pods array with ws/http status
curl -s http://192.168.31.23:8080/api/v1/fleet/health | jq '.pods | length'

# Logs API — must return .jsonl file, not .log
curl -s "http://192.168.31.23:8080/api/v1/logs?lines=1" | jq '.file'

# Error logs — check for active errors
curl -s "http://192.168.31.23:8080/api/v1/logs?level=error&lines=3"

# App health (added in recent milestone)
curl -s http://192.168.31.23:8080/api/v1/app-health

# Server health includes all expected fields
curl -s http://192.168.31.23:8080/api/v1/health | jq 'keys'
```

**Fix loop trigger:** Empty responses, stale log file name, active errors, missing endpoints (404).

---

## Phase 12: WebSocket Flows
**What:** Dashboard WS, agent WS connections alive and flowing data.

```bash
# WS endpoints exist (returns 400 = upgrade required, NOT 404)
curl -s -o /dev/null -w "%{http_code}" http://192.168.31.23:8080/ws/dashboard
curl -s -o /dev/null -w "%{http_code}" http://192.168.31.23:8080/ws/agent

# Fleet health — ws_connected must be true for all online pods
curl -s http://192.168.31.23:8080/api/v1/fleet/health | jq '.pods[] | {pod_number, ws_connected}'

# WS latency warnings in logs
curl -s "http://192.168.31.23:8080/api/v1/logs?level=warn&lines=50" | grep "round-trip slow"

# UDP heartbeat — verify pods are sending heartbeats
curl -s "http://192.168.31.23:8080/api/v1/logs?lines=20" | grep "udp.*heartbeat"
```

**Fix loop trigger:** WS returns 404, any pod ws_connected=false, excessive latency warnings.

---

## Phase 13: rc-agent Exec Capability
**What:** Every pod can execute commands via rc-agent :8090.

```bash
for IP in $PODS; do
  echo "=== $IP ===" && curl -s -X POST http://$IP:8090/exec \
    -H "Content-Type: application/json" -d '{"cmd":"hostname"}' | jq '.stdout'
done

# Verify exec_slots_available > 0 (not exhausted)
for IP in $PODS; do
  curl -s http://$IP:8090/health | jq '{ip: "'$IP'", slots: .exec_slots_available}'
done

# G2: Debug server :18924 endpoint reachable on each pod
for IP in $PODS; do
  curl -s -m 2 http://$IP:18924/debug > /dev/null 2>&1 && echo "$IP debug:18924 UP" || echo "$IP debug:18924 DOWN"
done
```

**Fix loop trigger:** Any pod returns exit_code != 0, empty stdout, 0 exec slots, or debug endpoint unreachable.

---

## Phase 14: rc-sentry Health
**What:** Every pod's rc-sentry is running and can detect + restart rc-agent.

```bash
# Sentry health on all pods
for IP in $PODS; do
  echo "=== $IP ===" && curl -s http://$IP:8091/health
done

# Sentry exec capability (separate from rc-agent exec)
for IP in $PODS; do
  curl -s -X POST http://$IP:8091/exec -d '{"cmd":"hostname"}' | jq '.stdout'
done

# Verify rc-sentry can see rc-agent process
for IP in $PODS; do
  curl -s -X POST http://$IP:8091/exec \
    -d '{"cmd":"tasklist /NH | findstr rc-agent.exe"}'
done
```

**Fix loop trigger:** Any sentry DOWN, sentry can't exec, rc-agent not visible to sentry.

---

## Phase 15: Preflight Checks
**What:** All rc-agent preflight checks pass on every pod.

```bash
# Check preflight results in rc-agent logs (look for FAILs)
for IP in $PODS; do
  echo "=== $IP ===" && curl -s -X POST http://$IP:8090/exec \
    -d '{"cmd":"findstr /C:\"preflight\" /C:\"FAIL\" C:\\RacingPoint\\rc-agent-*.jsonl 2>nul"}'
done
```

**Preflight checks verified:**
- DISP-01: Monitor resolution (1920x1080 per display, or 7680x1440 Surround)
- DISP-02: Lock screen/blanking active
- DISP-03: Popup overlay processes killed
- NET-01: Server .23 reachable
- NET-02: WebSocket connected
- HW-01: Wheelbase USB detected (VID:1209 PID:FFB0)
- HW-02: Pedal input responsive
- PROC-01: No duplicate rc-agent instances
- PROC-02: Game process not stale from previous session
- PROC-03: Popup window pre-flight (kill overlay processes)

**Fix loop trigger:** Any FAIL in preflight results.

---

## Phase 16: Cascade Guard & Recovery
**What:** Cascade guard preventing fleet-wide failures. Recovery paths functional.

```bash
# Check cascade guard logs — should be < 1 trigger/day in normal ops
curl -s "http://192.168.31.23:8080/api/v1/logs?lines=100" | grep -i "cascade_guard"

# Pod healer — check for recent healing actions
curl -s "http://192.168.31.23:8080/api/v1/logs?lines=100" | grep -i "pod_healer"

# Recovery module — no active recovery operations stuck
curl -s "http://192.168.31.23:8080/api/v1/logs?lines=50" | grep -i "recovery::"
```

**Fix loop trigger:** Cascade guard firing > 3x/day, healer stuck in loop, recovery operation hung.

---

# TIER 3: Display & UX (Phases 17-20)

## Phase 17: Lock Screen & Blanking
**What:** Every idle pod shows the correct lock/blanking screen.
**Standing rule:** "Audit what the CUSTOMER sees, not what the API returns."

```bash
# Check if Edge/kiosk is running as foreground on each pod
for IP in $PODS; do
  echo "=== $IP ===" && curl -s -X POST http://$IP:8090/exec \
    -d '{"cmd":"tasklist /V /FO CSV /NH | findstr /C:\"kiosk\" /C:\"Edge\" /C:\"chrome\""}'
done

# Check Edge process count (> 5 = stacking bug)
for IP in $PODS; do
  COUNT=$(curl -s -X POST http://$IP:8090/exec -d '{"cmd":"tasklist /NH | find /C \"msedge.exe\""}' | jq -r '.stdout' | tr -d '[:space:]')
  echo "$IP: $COUNT Edge processes"
done
```

**Fix loop trigger:** No Edge/kiosk as foreground, Edge count > 5. Ask user for physical verification.

---

## Phase 18: Overlay Suppression
**What:** No unwanted overlays on any pod (Copilot, NVIDIA, Widgets, OneDrive, Steam).

```bash
for IP in $PODS; do
  echo "=== $IP ===" && curl -s -X POST http://$IP:8090/exec \
    -d '{"cmd":"tasklist /V /FO CSV /NH | findstr /I /C:\"Copilot\" /C:\"NVIDIA Overlay\" /C:\"AMD DVR\" /C:\"OneDrive\" /C:\"Widgets\" /C:\"Steam\" /C:\"GameBar\""}'
done
```

**Fix loop trigger:** Any overlay process found on any pod — kill and verify registry key disabled.

---

## Phase 19: Display Resolution
**What:** All pods running correct display resolution. NVIDIA Surround not collapsed.
**Standing rule:** "NEVER restart explorer.exe on pods with NVIDIA Surround."

```bash
# Check resolution via SystemInfo or registry
for IP in $PODS; do
  curl -s -X POST http://$IP:8090/exec \
    -d '{"cmd":"wmic path Win32_VideoController get CurrentHorizontalResolution,CurrentVerticalResolution /value"}'
done

# Pods 1-7: expect 7680x1440 (NVIDIA Surround) or 1920x1080 per display
# Pod 8: may still be 1024x768 (NVIDIA Surround not configured — needs physical setup)
```

**Fix loop trigger:** Any pod showing 1024x768 (Surround collapsed). Pod 8 at 1024x768 = known issue, flag but don't fix remotely.

---

## Phase 20: Kiosk Browser Health
**What:** Edge kiosk mode running correctly, no popups, correct URL loaded.

```bash
# Verify Edge command line contains kiosk URL
for IP in $PODS; do
  curl -s -X POST http://$IP:8090/exec \
    -d '{"cmd":"wmic process where \"name='\''msedge.exe'\''\" get CommandLine /value 2>nul | findstr /C:\"kiosk\" /C:\"3300\""}'
done

# Verify kiosk page is accessible from pod's perspective
for IP in $PODS; do
  curl -s -X POST http://$IP:8090/exec \
    -d '{"cmd":"curl.exe -s -o nul -w \"%{http_code}\" http://192.168.31.23:3300/kiosk"}'
done
```

**Fix loop trigger:** Edge not in kiosk mode, kiosk URL not loading from pod.

---

# TIER 4: Billing & Commerce (Phases 21-25)

## Phase 21: Pricing & Billing Sessions
**What:** Pricing tiers loaded, billing sessions can start and end, active sessions tracked.

```bash
# Pricing tiers (must return tiers with amounts)
curl -s http://192.168.31.23:8080/api/v1/pricing -H "x-terminal-session: $SESSION" | jq 'length'

# Active sessions (may be 0 if venue closed — check count, not content)
curl -s http://192.168.31.23:8080/api/v1/billing/sessions/active -H "x-terminal-session: $SESSION"

# Recent completed sessions (verify billing is recording)
curl -s "http://192.168.31.23:8080/api/v1/billing/sessions?limit=3" -H "x-terminal-session: $SESSION"
```

**Fix loop trigger:** No pricing tiers, billing endpoint returns 500, no sessions in last 24h during operating hours.

---

## Phase 22: Wallet & Payments
**What:** Wallet system functional — balance queries, topup, debit work.

```bash
# Wallet endpoint responds
curl -s http://192.168.31.23:8080/api/v1/wallets -H "x-terminal-session: $SESSION" | jq 'length'

# Check for debit_intents stuck in pending
curl -s "http://192.168.31.23:8080/api/v1/logs?lines=50" | grep -i "debit_intent.*pending\|wallet.*error"
```

**Fix loop trigger:** Wallet endpoint 500, stuck debit_intents > 1h old.

---

## Phase 23: Pod Reservation & Booking
**What:** Reservation system prevents double-booking, handles cancellation.

```bash
# Reservations endpoint responds
curl -s http://192.168.31.23:8080/api/v1/reservations -H "x-terminal-session: $SESSION" | jq 'length'

# Check for expired reservations not cleaned up
curl -s "http://192.168.31.23:8080/api/v1/logs?lines=50" | grep -i "reservation.*expir"
```

**Fix loop trigger:** Reservation endpoint 500, expired reservations not cleaned.

---

## Phase 24: Accounting & Reconciliation
**What:** Accounting module tracks revenue, refunds reconcile, no orphan transactions.

```bash
# Accounting endpoint
curl -s http://192.168.31.23:8080/api/v1/accounting -H "x-terminal-session: $SESSION" 2>/dev/null

# Check for refund errors
curl -s "http://192.168.31.23:8080/api/v1/logs?lines=50" | grep -i "refund.*error\|accounting.*mismatch"
```

**Fix loop trigger:** Accounting mismatch errors, orphan refund records.

---

## Phase 25: Cafe Menu & Inventory
**What:** Cafe menu loads, inventory tracked, stock alerts fire, orders process.

```bash
# Menu items loaded
curl -s http://192.168.31.23:8080/api/v1/cafe/menu -H "x-terminal-session: $SESSION" | jq 'length'

# Inventory alerts (low stock)
curl -s "http://192.168.31.23:8080/api/v1/logs?lines=50" | grep -i "cafe_alert\|low.stock\|inventory"

# Recent orders (may be 0 if cafe closed)
curl -s http://192.168.31.23:8080/api/v1/cafe/orders -H "x-terminal-session: $SESSION" | jq 'length'

# Promos loaded
curl -s http://192.168.31.23:8080/api/v1/cafe/promos -H "x-terminal-session: $SESSION" | jq 'length'
```

**Fix loop trigger:** Menu endpoint empty, inventory alerts failing, orders endpoint 500.

---

# TIER 5: Games & Hardware (Phases 26-29)

## Phase 26: Game Catalog & Launcher
**What:** All games listed in catalog, launcher can start games on pods.

```bash
# Game catalog
curl -s http://192.168.31.23:8080/api/v1/games -H "x-terminal-session: $SESSION" | jq '.[].name'

# Game catalog count (should match configured games)
curl -s http://192.168.31.23:8080/api/v1/games/catalog -H "x-terminal-session: $SESSION" | jq 'length'

# Verify game exe exists on at least one pod (spot check)
SPOT_POD=$(echo $PODS | awk '{print $1}')
curl -s -X POST http://$SPOT_POD:8090/exec \
  -d '{"cmd":"dir \"C:\\Program Files (x86)\\Steam\\steamapps\\common\\assettocorsa\\AssettoCorsa.exe\" 2>nul || echo MISSING"}'

# G16: Auto-switch config — ConspitLink game config applied on pod boot
curl -s -X POST http://$SPOT_POD:8090/exec \
  -d '{"cmd":"findstr /C:\"auto_switch\" /C:\"conspit\" C:\\RacingPoint\\rc-agent-*.jsonl 2>nul | findstr /V /C:\"debug\""}'
```

**Fix loop trigger:** Catalog empty, game exe missing on pod, auto-switch config errors in logs.

---

## Phase 27: AC Server & Telemetry
**What:** Assetto Corsa server process, telemetry UDP ports, lap data flowing.

```bash
# Check if AC server is running on server .23
curl -s -X POST http://192.168.31.23:8090/exec \
  -d '{"cmd":"tasklist /NH | findstr /I \"AssettoCorsa\""}'

# Telemetry UDP ports listening on pods (9996 for AC)
for IP in $PODS; do
  curl -s -X POST http://$IP:8090/exec \
    -d '{"cmd":"netstat -an | findstr /C:\"9996\" /C:\"20777\" /C:\"5300\" | findstr UDP"}'
done

# Recent lap data in logs
curl -s "http://192.168.31.23:8080/api/v1/logs?lines=50" | grep -i "lap_tracker\|telemetry\|lap_time"

# G11: Port allocator — no conflicting port assignments
# Check that game UDP ports (9996, 20777, 5300, 6789, 5555) don't conflict with system services
for IP in $PODS; do
  curl -s -X POST http://$IP:8090/exec \
    -d '{"cmd":"netstat -an | findstr /C:\"9996\" /C:\"20777\" /C:\"5300\" /C:\"6789\" /C:\"5555\" | findstr LISTENING"}'
done
# If any game port shows TCP LISTENING (not UDP), there's a port conflict

# AC camera gimbal control active (if AC server running)
curl -s "http://192.168.31.23:8080/api/v1/logs?lines=50" | grep -i "camera_control\|gimbal\|ac_camera"
```

**Fix loop trigger:** No telemetry ports listening during active sessions, lap data not recording, TCP port conflict on game UDP ports, camera gimbal errors.

---

## Phase 28: FFB & Hardware Detection
**What:** Wheelbase USB detected, pedals responsive, driving_detector active.

```bash
# HW-01: Wheelbase USB on each pod (VID:1209 PID:FFB0)
for IP in $PODS; do
  echo "=== $IP ===" && curl -s -X POST http://$IP:8090/exec \
    -d '{"cmd":"wmic path Win32_PnPEntity where \"DeviceID like '\''%1209%FFB0%'\''\" get Name /value 2>nul || echo NO_WHEELBASE"}'
done

# Check driving_detector logs (should detect steering input)
for IP in $PODS; do
  curl -s -X POST http://$IP:8090/exec \
    -d '{"cmd":"findstr /C:\"driving_detector\" C:\\RacingPoint\\rc-agent-*.jsonl 2>nul | findstr /V /C:\"debug\""}'
done
```

**Fix loop trigger:** Wheelbase not detected on any pod, driving_detector not logging.

---

## Phase 29: Multiplayer & Friends
**What:** Multiplayer sessions can be created, friends system functional.

```bash
# Multiplayer endpoint
curl -s http://192.168.31.23:8080/api/v1/multiplayer -H "x-terminal-session: $SESSION" 2>/dev/null

# Friends endpoint
curl -s http://192.168.31.23:8080/api/v1/friends -H "x-terminal-session: $SESSION" 2>/dev/null

# Check for multiplayer errors in logs
curl -s "http://192.168.31.23:8080/api/v1/logs?lines=30" | grep -i "multiplayer.*error"
```

**Fix loop trigger:** Multiplayer endpoint 500, friend lookup errors.

---

# TIER 6: Notifications & Marketing (Phases 30-34)

## Phase 30: WhatsApp Alerter
**What:** Evolution API connected, phone numbers correct, messages can send.

```bash
# Check WhatsApp alerter config in TOML
curl -s -X POST http://192.168.31.23:8090/exec \
  -d '{"cmd":"findstr /C:\"whatsapp\" /C:\"evolution\" C:\\RacingPoint\\racecontrol.toml"}'

# Check for WhatsApp send errors in logs
curl -s "http://192.168.31.23:8080/api/v1/logs?lines=50" | grep -i "whatsapp\|evolution.*error\|wa_send"

# Phone number mapping check:
# Staff alerts: 7075778180
# Customer bot 1: 9059833001
# Customer bot 2: 9054548180
```

**Fix loop trigger:** Evolution API connection errors, phone number mismatch in config, send failures.

---

## Phase 31: Email Alerts
**What:** Gmail OAuth token fresh, email script exists, alert thresholds configured.

```bash
# Email alert config
curl -s -X POST http://192.168.31.23:8090/exec \
  -d '{"cmd":"findstr /C:\"email\" /C:\"gmail\" /C:\"smtp\" C:\\RacingPoint\\racecontrol.toml"}'

# Check for email send errors
curl -s "http://192.168.31.23:8080/api/v1/logs?lines=50" | grep -i "email.*error\|gmail.*token\|smtp.*fail"

# Verify email script path exists
curl -s -X POST http://192.168.31.23:8090/exec \
  -d '{"cmd":"dir C:\\RacingPoint\\send-email.ps1 2>nul || echo MISSING"}'
```

**Fix loop trigger:** OAuth token expired (403 in logs), email script missing, send failures.

---

## Phase 32: Discord Integration
**What:** Discord webhook/token valid, race results posting.

```bash
# Discord config in TOML
curl -s -X POST http://192.168.31.23:8090/exec \
  -d '{"cmd":"findstr /C:\"discord\" C:\\RacingPoint\\racecontrol.toml"}'

# Check for Discord errors in logs
curl -s "http://192.168.31.23:8080/api/v1/logs?lines=50" | grep -i "discord.*error\|webhook.*fail"
```

**Fix loop trigger:** Discord token invalid, webhook errors, no posts in 7+ days during active operations.

---

## Phase 33: Cafe Marketing & PNG Generation
**What:** Marketing content generates, WhatsApp broadcast works, promo engine evaluates.

```bash
# Check cafe_marketing module active
curl -s "http://192.168.31.23:8080/api/v1/logs?lines=50" | grep -i "cafe_marketing\|png.*generat\|broadcast"

# Promo engine evaluation
curl -s "http://192.168.31.23:8080/api/v1/logs?lines=50" | grep -i "cafe_promo.*evaluat\|promo.*applied"
```

**Fix loop trigger:** Marketing generation errors, promo evaluation failures.

---

## Phase 34: Psychology & Gamification
**What:** Badge system awarding, notification dispatch, progress tracking, reward cycles.

```bash
# Psychology engine running
curl -s "http://192.168.31.23:8080/api/v1/logs?lines=100" | grep -i "psychology\|badge.*award\|streak\|reward"

# Check badge/notification dispatch
curl -s "http://192.168.31.23:8080/api/v1/logs?lines=100" | grep -i "notification.*dispatch\|badge.*criteria"

# Bot coordinator (orchestrates WhatsApp/Discord/email)
curl -s "http://192.168.31.23:8080/api/v1/logs?lines=50" | grep -i "bot_coordinator"
```

**Fix loop trigger:** Psychology engine not logging, badges never awarding, notifications stuck.

---

# TIER 7: Data & Sync (Phases 35-38)

## Phase 35: Cloud Sync Bidirectional
**What:** Push AND pull verified with actual data counts.

```bash
# Recent sync in venue logs
curl -s "http://192.168.31.23:8080/api/v1/logs?lines=50" | grep -i "sync push\|sync pull\|upserted"

# Recent sync errors
curl -s "http://192.168.31.23:8080/api/v1/logs?level=error&lines=5"

# Bono side sync (via relay — preferred over SSH)
curl -s -X POST http://localhost:8766/relay/exec/run \
  -d '{"command":"pm2_logs","args":"--lines 10","reason":"audit sync check"}'

# Build ID match (venue vs cloud)
LOCAL=$(curl -s http://192.168.31.23:8080/api/v1/health | jq -r '.build_id')
CLOUD=$(curl -s http://100.70.177.44:8080/api/v1/health | jq -r '.build_id')
echo "Venue: $LOCAL | Cloud: $CLOUD"
[ "$LOCAL" = "$CLOUD" ] && echo "MATCH" || echo "MISMATCH — redeploy"
```

**Fix loop trigger:** No recent sync, active sync errors, build mismatch, sync only one direction.

---

## Phase 36: Database Schema & Migrations
**What:** All tables have required columns. Migration versions match. No schema drift between venue and cloud.

```bash
# Cloud DB schema check — all sync tables have updated_at
ssh root@100.70.177.44 "for t in drivers wallets billing_sessions pricing_tiers \
  kiosk_experiences reservations debit_intents kiosk_settings cafe_orders \
  cafe_menu_items cafe_promos feature_flags activity_log; do \
  echo -n \"\$t: \"; sqlite3 /root/racecontrol/data/racecontrol.db \
  \"PRAGMA table_info(\$t)\" | grep -c updated_at; done"

# Venue DB — same check
curl -s -X POST http://192.168.31.23:8090/exec \
  -d '{"cmd":"for %t in (drivers wallets billing_sessions pricing_tiers feature_flags) do sqlite3 C:\\RacingPoint\\data\\racecontrol.db \"PRAGMA table_info(%t)\" | findstr updated_at"}'

# Migration version table — compare venue vs cloud
curl -s -X POST http://192.168.31.23:8090/exec \
  -d '{"cmd":"sqlite3 C:\\RacingPoint\\data\\racecontrol.db \"SELECT * FROM _sqlx_migrations ORDER BY version DESC LIMIT 3\""}'
```

**Fix loop trigger:** Any table missing updated_at, migration version mismatch between venue and cloud.

---

## Phase 37: Activity Log & Compliance
**What:** Audit trail recording, PII not leaked in logs, retention policy enforced.

```bash
# Activity log has recent entries
curl -s -X POST http://192.168.31.23:8090/exec \
  -d '{"cmd":"sqlite3 C:\\RacingPoint\\data\\racecontrol.db \"SELECT COUNT(*) FROM activity_log WHERE created_at > datetime('\''now'\'', '\''-24 hours'\'')\""}'

# Check for PII in log files (phone numbers, emails should not appear in plaintext)
curl -s "http://192.168.31.23:8080/api/v1/logs?lines=100" | grep -oE "[0-9]{10}" | head -5
# If phone numbers appear in log output, flag for PII masking

# DPDP compliance — check deletion/retention config exists
curl -s -X POST http://192.168.31.23:8090/exec \
  -d '{"cmd":"findstr /C:\"retention\" /C:\"deletion\" /C:\"dpdp\" C:\\RacingPoint\\racecontrol.toml"}'
```

**Fix loop trigger:** No activity log entries in 24h, PII in plaintext logs, no retention config.

---

## Phase 38: Bono Relay & Failover
**What:** Bono relay bidirectional, failover graceful when cloud is down.

```bash
# Relay health — must be REALTIME
curl -s http://localhost:8766/relay/health | jq '.connectionMode'

# Bidirectional test — exec on Bono
curl -s -X POST http://localhost:8766/relay/exec/run \
  -d '{"command":"node_version","reason":"audit"}'

# Bono → James (reverse direction — check INBOX.md for recent entries)
git -C C:/Users/bono/racingpoint/comms-link log --oneline -3

# Test graceful degrade: what happens when cloud is unreachable?
# (Don't actually kill cloud — just verify logs show graceful handling)
curl -s "http://192.168.31.23:8080/api/v1/logs?lines=50" | grep -i "cloud.*unreachable\|sync.*retry\|fallback"

# G1: remote_terminal — server polls cloud for exec commands (reverse direction)
curl -s "http://192.168.31.23:8080/api/v1/logs?lines=50" | grep -i "remote_terminal\|cloud.*exec.*poll\|pending.*action"
```

**Fix loop trigger:** connectionMode != REALTIME, exec fails, no recent bidirectional comms, remote_terminal errors in logs.

---

# TIER 8: Advanced Systems (Phases 39-42)

## Phase 39: Feature Flags (v22.0)
**What:** Feature flags table populated, rc-agent fetching flags, overrides working.

```bash
# Server flags endpoint
curl -s http://192.168.31.23:8080/api/v1/flags -H "x-terminal-session: $SESSION"

# Feature flags table populated
curl -s -X POST http://192.168.31.23:8090/exec \
  -d '{"cmd":"sqlite3 C:\\RacingPoint\\data\\racecontrol.db \"SELECT name, enabled FROM feature_flags\""}'

# rc-agent flag fetch logs (should fetch at startup + periodic)
SPOT_POD=$(echo $PODS | awk '{print $1}')
curl -s -X POST http://$SPOT_POD:8090/exec \
  -d '{"cmd":"findstr /C:\"feature_flag\" C:\\RacingPoint\\rc-agent-*.jsonl 2>nul"}'
```

**Fix loop trigger:** Flags endpoint empty/500, rc-agent not fetching flags, flag cache stale.

---

## Phase 40: Scheduler & Action Queue
**What:** Scheduled tasks processing, action queue draining, no stale items.

```bash
# Scheduler activity in logs
curl -s "http://192.168.31.23:8080/api/v1/logs?lines=100" | grep -i "scheduler.*execute\|scheduler.*tick\|action_queue"

# Check for stuck/failed actions
curl -s -X POST http://192.168.31.23:8090/exec \
  -d '{"cmd":"sqlite3 C:\\RacingPoint\\data\\racecontrol.db \"SELECT status, COUNT(*) FROM action_queue GROUP BY status\""}'

# Old pending items (> 1 hour old)
curl -s -X POST http://192.168.31.23:8090/exec \
  -d '{"cmd":"sqlite3 C:\\RacingPoint\\data\\racecontrol.db \"SELECT COUNT(*) FROM action_queue WHERE status='\''pending'\'' AND created_at < datetime('\''now'\'', '\''-1 hour'\'')\""}'
```

**Fix loop trigger:** Scheduler not ticking, action queue > 10 pending items, stale items > 1h.

---

## Phase 41: Config Push & OTA
**What:** Config distribution to pods working, OTA pipeline state machine healthy.

```bash
# Config push logs
curl -s "http://192.168.31.23:8080/api/v1/logs?lines=50" | grep -i "config_push\|ota_pipeline"

# OTA pipeline state (v22.0 — may still be in development)
curl -s "http://192.168.31.23:8080/api/v1/logs?lines=50" | grep -i "ota.*state\|ota.*transition"

# Verify pod configs are consistent (spot check 2 pods)
POD1=$(echo $PODS | awk '{print $1}')
POD2=$(echo $PODS | awk '{print $2}')
curl -s -X POST http://$POD1:8090/exec -d '{"cmd":"type C:\\RacingPoint\\rc-agent.toml"}' | jq -r '.stdout' > /tmp/pod1.toml 2>/dev/null
curl -s -X POST http://$POD2:8090/exec -d '{"cmd":"type C:\\RacingPoint\\rc-agent.toml"}' | jq -r '.stdout' > /tmp/pod2.toml 2>/dev/null
```

**Fix loop trigger:** Config push errors, OTA stuck in transition state.

---

## Phase 42: Error Aggregator & Fleet Alerts
**What:** Error rates tracked, fleet alerts dispatching, escalation chain working.

```bash
# Error aggregator active
curl -s "http://192.168.31.23:8080/api/v1/logs?lines=100" | grep -i "error_aggregator\|error_rate"

# Fleet alert dispatch
curl -s "http://192.168.31.23:8080/api/v1/logs?lines=100" | grep -i "fleet_alert.*dispatch\|fleet_alert.*send"

# Current error rate (should be < 10/hour)
curl -s "http://192.168.31.23:8080/api/v1/logs?level=error&lines=1" | jq '.filtered'

# Current warn rate (should be < 100/hour)
curl -s "http://192.168.31.23:8080/api/v1/logs?level=warn&lines=1" | jq '.filtered'
```

**Fix loop trigger:** Error rate > 10/hour, warn rate > 100/hour, fleet alerts not dispatching.

---

# TIER 9: Cameras & AI (Phases 43-44)

## Phase 43: Camera Pipeline
**What:** go2rtc streams all 13 cameras. NVR reachable. Streams serving.
**Standing rule:** "Verify monitoring targets against running system, not docs."

```bash
# go2rtc on James :1984 — stream count
curl -s http://localhost:1984/api/streams | jq 'length'

# List all streams (should be 13 cameras + possible extras)
curl -s http://localhost:1984/api/streams | jq 'keys'

# NVR reachable
curl -s -m 5 http://192.168.31.18 > /dev/null 2>&1 && echo "NVR: UP" || echo "NVR: DOWN"

# go2rtc process running
tasklist 2>/dev/null | grep go2rtc || echo "go2rtc NOT RUNNING"
```

**Fix loop trigger:** Stream count < 13, NVR unreachable, go2rtc dead.

---

## Phase 44: Face Detection & People Counter
**What:** rc-sentry-ai running, detecting faces on 3 cameras, audit log fresh.

```bash
# rc-sentry-ai process running on James
tasklist 2>/dev/null | grep rc-sentry-ai || echo "rc-sentry-ai NOT RUNNING"

# Face audit log — recent entries
wc -l C:/RacingPoint/logs/face-audit.jsonl 2>/dev/null || echo "NO AUDIT LOG"
tail -1 C:/RacingPoint/logs/face-audit.jsonl 2>/dev/null

# Check rc-sentry-ai log for errors
tail -20 C:/RacingPoint/rc-sentry-ai.log 2>/dev/null | grep -i "error\|fail\|panic"

# People counter (port 8095)
curl -s http://localhost:8095/health 2>/dev/null || echo "People counter NOT RUNNING"
```

**Fix loop trigger:** rc-sentry-ai dead, no face detections in 1h during operating hours, people counter down.

---

# TIER 10: Ops & Compliance (Phases 45-47)

## Phase 45: Log Health & Rotation
**What:** Log files not bloated, rotation working, no flooding.

```bash
# Server log size today
curl -s -X POST http://192.168.31.23:8090/exec \
  -d '{"cmd":"for %f in (C:\\RacingPoint\\logs\\racecontrol-*.jsonl) do echo %f %~zf"}'

# Pod log sizes (spot check 2 pods)
for IP in $(echo $PODS | awk '{print $1, $4}'); do
  echo "=== $IP ===" && curl -s -X POST http://$IP:8090/exec \
    -d '{"cmd":"for %f in (C:\\RacingPoint\\rc-agent-*.jsonl) do echo %f %~zf"}'
done

# James logs — rc-sentry-ai rotation
ls -la C:/RacingPoint/rc-sentry-ai.log C:/RacingPoint/rc-sentry-ai-old.log 2>/dev/null

# Error rate (should be < 10/hour)
curl -s "http://192.168.31.23:8080/api/v1/logs?level=error&lines=1" | jq '.filtered'

# Warn rate (should be < 100/hour)
curl -s "http://192.168.31.23:8080/api/v1/logs?level=warn&lines=1" | jq '.filtered'
```

**Fix loop trigger:** Any log > 50MB/day, error rate > 10/hour, warn rate > 100/hour.

---

## Phase 46: Comms-Link E2E
**What:** Single exec, chain, health — all pass per Ultimate Rule.

```bash
# Single exec
curl -s -X POST http://localhost:8766/relay/exec/run \
  -d '{"command":"node_version","reason":"audit"}'

# Chain
curl -s -X POST http://localhost:8766/relay/chain/run \
  -d '{"steps":[{"command":"node_version"},{"command":"uptime"}]}'

# Health
curl -s http://localhost:8766/relay/health

# Quality gate (if time permits)
# cd C:/Users/bono/racingpoint/comms-link && COMMS_PSK="85d1d06c806b3cc5159676bbed35e29ef0a60661e442a683c2c5a345f2036df0" bash test/run-all.sh
```

**Fix loop trigger:** Any non-zero exitCode, connectionMode != REALTIME.

---

## Phase 47: Standing Rules Compliance
**What:** Auto-push clean, Bono synced, rules synced across all 3 files.

```bash
# Unpushed commits
cd C:/Users/bono/racingpoint/racecontrol && git status -sb && cd -
cd C:/Users/bono/racingpoint/comms-link && git status -sb && cd -

# Rules sync check — cascade update rule in all 3 locations
grep "data formats" C:/Users/bono/racingpoint/racecontrol/CLAUDE.md \
  C:/Users/bono/racingpoint/comms-link/CLAUDE.md \
  C:/Users/bono/.claude/projects/C--Users-bono/memory/standing-rules.md 2>/dev/null

# Check LOGBOOK.md has recent entries
tail -5 C:/Users/bono/racingpoint/racecontrol/LOGBOOK.md
```

**Fix loop trigger:** Unpushed commits, rules out of sync, LOGBOOK stale.

---

# TIER 11: E2E Journeys (Phases 48-50)

## Phase 48: Customer Journey E2E
**What:** Complete customer path: kiosk → pod select → game → telemetry → billing end.
**Standing rule:** "Shipped Means Works For The User."

**Automated smoke test:**
```bash
# Kiosk HTML loads with Next.js markers
KIOSK_OK=$(curl -s http://192.168.31.23:3300/kiosk | grep -c "__NEXT")
echo "Kiosk: $KIOSK_OK Next.js markers"

# Dashboard HTML loads
DASH_OK=$(curl -s http://192.168.31.23:3200 | grep -c "__NEXT")
echo "Dashboard: $DASH_OK Next.js markers"

# Admin HTML loads
ADMIN_OK=$(curl -s http://192.168.31.23:3100 2>/dev/null | grep -c "__NEXT")
echo "Admin: $ADMIN_OK Next.js markers"

# All 3 must be > 0
```

**Manual steps (requires physical or remote verification):**
1. Open kiosk at `:3300/kiosk` — 8 pod grid visible?
2. Click available pod — PIN modal opens?
3. Start billing session on a pod — timer starts?
4. Launch game — pod status changes to "launching" → "running"?
5. Telemetry flows — speed/RPM visible on dashboard?
6. End session — pod returns to idle, billing record created?

**Fix loop trigger:** Any page doesn't load, any manual step fails.

---

## Phase 49: Staff / POS Journey E2E
**What:** POS operations work — billing from dashboard, refunds, session management.

```bash
# POS PC rc-agent alive
curl -s http://192.168.31.20:8090/health 2>/dev/null || echo "POS OFFLINE"

# Dashboard accessible from POS perspective (verify from James browser)
curl -s -o /dev/null -w "%{http_code}" http://192.168.31.23:3200

# Admin dashboard accessible
curl -s -o /dev/null -w "%{http_code}" http://192.168.31.23:3100 2>/dev/null

# WhatsApp receipt capability (verify recent sends)
curl -s "http://192.168.31.23:8080/api/v1/logs?lines=50" | grep -i "whatsapp.*receipt\|pdf.*receipt"
```

**Manual steps:**
1. Open dashboard from POS/James — pods visible with live status?
2. Start/end a test session — timer + billing correct?
3. Issue a refund — refund record created, wallet updated?

**Fix loop trigger:** POS offline, dashboard not loading from non-server machine, receipt sends failing.

---

## Phase 50: Security & Auth E2E
**What:** PIN auth works, JWT tokens have correct expiry, admin endpoints protected.

```bash
# PIN auth — valid PIN returns session token
curl -s -X POST http://192.168.31.23:8080/api/v1/terminal/auth \
  -H "Content-Type: application/json" -d '{"pin":"261121"}' | jq '.session | length'

# Invalid PIN — must return 401/403
curl -s -o /dev/null -w "%{http_code}" -X POST http://192.168.31.23:8080/api/v1/terminal/auth \
  -H "Content-Type: application/json" -d '{"pin":"000000"}'

# Protected endpoint without auth — must return 401
curl -s -o /dev/null -w "%{http_code}" http://192.168.31.23:8080/api/v1/billing/sessions/active

# Public endpoints work without auth (allowlist, health)
curl -s -o /dev/null -w "%{http_code}" http://192.168.31.23:8080/api/v1/health
curl -s -o /dev/null -w "%{http_code}" http://192.168.31.23:8080/api/v1/guard/whitelist/pod-1

# JWT secret entropy check (should not be "secret" or "password")
curl -s -X POST http://192.168.31.23:8090/exec \
  -d '{"cmd":"findstr /C:\"jwt_secret\" /C:\"auth_secret\" C:\\RacingPoint\\racecontrol.toml"}'

# G13: network_source middleware — verify LAN vs Tailscale IP classification
# Request from LAN IP (.27) should be classified as local
curl -s http://192.168.31.23:8080/api/v1/health -H "X-Forwarded-For: 192.168.31.27" | jq '.network_source // "not exposed"'
# Request via Tailscale should be classified as tailscale
curl -s http://100.125.108.37:8080/api/v1/health | jq '.network_source // "not exposed"'
# Verify cloud endpoints are accessible via Tailscale but not external
curl -s -m 5 http://100.70.177.44:8080/api/v1/health > /dev/null 2>&1 && echo "Cloud via TS: OK" || echo "Cloud via TS: FAIL"
```

**Fix loop trigger:** Valid PIN rejected, invalid PIN accepted, protected endpoints return 200 without auth, JWT secret weak/default, network_source misclassifying IPs.

---

# TIER 12: Code Quality & Static Analysis (Phases 51-53)

## Phase 51: Static Code Analysis
**What:** Automated grep for standing rule violations in codebase. Zero-tolerance anti-patterns.
**Standing rules:** CQ-01 (no unwrap), CQ-05 (no any), SEC-03 (no secrets), TST-23 (no explorer restart on pods).

```bash
cd C:/Users/bono/racingpoint/racecontrol

# CQ-01: No .unwrap() in production Rust (exclude tests, build.rs, examples)
UNWRAP_COUNT=$(grep -rn "\.unwrap()" crates/racecontrol/src/ crates/rc-agent/src/ crates/rc-common/src/ \
  --include="*.rs" | grep -v "#\[cfg(test)\]" | grep -v "test.rs" | grep -v "tests/" | wc -l)
echo "Unwrap violations: $UNWRAP_COUNT"

# CQ-05: No `any` in TypeScript (exclude node_modules, .next)
ANY_COUNT=$(grep -rn ": any" kiosk/src/ pwa/src/ web/src/ --include="*.ts" --include="*.tsx" \
  | grep -v node_modules | grep -v ".d.ts" | wc -l)
echo "TypeScript any violations: $ANY_COUNT"

# SEC-03: No secrets committed to git
SECRET_FILES=$(git ls-files | grep -iE "\.env$|credential|\.secret|\.key$|token\.json" \
  | grep -v ".env.example" | grep -v ".env.local.example")
echo "Secret files in git: ${SECRET_FILES:-NONE}"

# CQ-09: bat files are ASCII+CRLF (no UTF-8 BOM)
for BAT in deploy-staging/*.bat C:/RacingPoint/start-*.bat; do
  [ -f "$BAT" ] && {
    FIRST=$(xxd -l 3 "$BAT" 2>/dev/null | grep -c "efbb bf")
    [ "$FIRST" -gt 0 ] && echo "BOM FOUND: $BAT" || echo "OK: $BAT"
  }
done

# CQ-10: bat files have no parentheses in if/else (use goto labels)
grep -rn "if.*(" deploy-staging/*.bat 2>/dev/null | grep -v "REM\|::" | grep -v "findstr\|find\|tasklist"

# TST-23: No explorer.exe restart in any script targeting pods
grep -rni "explorer" deploy-staging/*.bat deploy-staging/*.ps1 2>/dev/null | grep -i "stop\|kill\|restart"
```

**Fix loop trigger:** Any unwrap > 0 (new since last audit), any TypeScript `any`, secret files in git, BOM in bat, parentheses in bat if/else, explorer restart in pod scripts.

---

## Phase 52: Frontend Deploy Integrity
**What:** Next.js builds are structurally complete. All NEXT_PUBLIC_ vars set. Standalone deploy correct.
**Standing rules:** DBG-12 (NEXT_PUBLIC_ completeness), DBG-13 (.next/standalone structure), CQ-14 (no hardcoded UI), CQ-17 (Edge stacking).

```bash
# DBG-12: All NEXT_PUBLIC_ vars referenced in code exist in .env.production.local
for APP in kiosk pwa web; do
  APP_DIR="C:/Users/bono/racingpoint/racecontrol/$APP"
  [ -d "$APP_DIR/src" ] || continue
  echo "=== $APP ==="
  # Find all NEXT_PUBLIC_ references in source
  VARS=$(grep -roh "NEXT_PUBLIC_[A-Z_]*" "$APP_DIR/src/" 2>/dev/null | sort -u)
  # Check each exists in .env.production.local
  for VAR in $VARS; do
    grep -q "$VAR" "$APP_DIR/.env.production.local" 2>/dev/null \
      && echo "  OK: $VAR" || echo "  MISSING: $VAR"
  done
done

# DBG-13: .next/standalone has .next/static copied (required for standalone deploy)
for APP in kiosk pwa web; do
  APP_DIR="C:/Users/bono/racingpoint/racecontrol/$APP"
  [ -d "$APP_DIR/.next/standalone" ] || { echo "$APP: NO STANDALONE BUILD"; continue; }
  [ -d "$APP_DIR/.next/standalone/.next/static" ] \
    && echo "$APP: standalone/static OK" \
    || echo "$APP: MISSING .next/static in standalone!"
done

# DBG-13b: Runtime static file serving verification (CRITICAL)
# Checks that _next/static/* files are actually served by each Next.js app.
# Root cause (2026-03-25): Next.js standalone embeds build-machine absolute paths
# in required-server-files.json (appDir field) and server.js (outputFileTracingRoot).
# When deployed to a different machine, these stale paths cause static file 404s
# even though the files exist on disk. Pages render (SSR works) but CSS/JS/fonts
# all return 404, making the UI unstyled and non-functional.
# Fix: Set outputFileTracingRoot: path.join(__dirname) in each next.config.ts
# AND verify appDir in deployed required-server-files.json matches deploy path.

echo "=== Runtime Static File Check ==="

# Kiosk (basePath: /kiosk)
KIOSK_CSS=$(curl -s http://192.168.31.23:3300/kiosk 2>/dev/null | grep -oP 'href="/kiosk/_next/static/chunks/[^"]+\.css"' | head -1 | grep -oP '/kiosk/_next/static/chunks/[^"]+')
if [ -n "$KIOSK_CSS" ]; then
  KIOSK_STATIC=$(curl -s -o /dev/null -w "%{http_code}" "http://192.168.31.23:3300${KIOSK_CSS}")
  [ "$KIOSK_STATIC" = "200" ] && echo "kiosk: static CSS OK" || echo "kiosk: STATIC FILE 404 — check appDir in required-server-files.json"
else
  echo "kiosk: no CSS reference found in HTML — app may be down"
fi

# Web dashboard (no basePath)
WEB_JS=$(curl -s http://192.168.31.23:3200 2>/dev/null | grep -oP 'src="/_next/static/chunks/[^"]+\.js"' | head -1 | grep -oP '/_next/static/chunks/[^"]+')
if [ -n "$WEB_JS" ]; then
  WEB_STATIC=$(curl -s -o /dev/null -w "%{http_code}" "http://192.168.31.23:3200${WEB_JS}")
  [ "$WEB_STATIC" = "200" ] && echo "web: static JS OK" || echo "web: STATIC FILE 404 — check appDir in required-server-files.json"
else
  echo "web: no JS reference found in HTML — app may be down"
fi

# Admin (no basePath, port 3201)
ADMIN_JS=$(curl -s -L http://192.168.31.23:3201 2>/dev/null | grep -oP 'src="/_next/static/chunks/[^"]+\.js"' | head -1 | grep -oP '/_next/static/chunks/[^"]+')
if [ -n "$ADMIN_JS" ]; then
  ADMIN_STATIC=$(curl -s -o /dev/null -w "%{http_code}" "http://192.168.31.23:3201${ADMIN_JS}")
  [ "$ADMIN_STATIC" = "200" ] && echo "admin: static JS OK" || echo "admin: STATIC FILE 404 — check appDir in required-server-files.json"
else
  echo "admin: no JS reference found in HTML — app may be down"
fi

# CQ-14: No hardcoded camera arrays in UI (should fetch from API)
grep -rn "cam[0-9]\|camera.*=.*\[" kiosk/src/ pwa/src/ web/src/ --include="*.tsx" --include="*.ts" \
  2>/dev/null | grep -v "import\|//\|node_modules" | head -10

# CQ-17: Edge stacking — close_browser kills BOTH msedge AND msedgewebview2
grep -A2 "close_browser\|kill.*edge\|taskkill.*msedge" \
  C:/Users/bono/racingpoint/racecontrol/crates/rc-agent/src/*.rs 2>/dev/null \
  | grep -c "msedgewebview2"
# Should be > 0 (webview2 kill exists)
```

**Fix loop trigger:** Any NEXT_PUBLIC_ var MISSING, standalone/.next/static absent, hardcoded camera arrays found, msedgewebview2 not killed alongside msedge.

---

## Phase 53: Binary Consistency & Watchdog
**What:** All 8 pods run identical binary. Server watchdog singleton enforced.
**Standing rules:** DEP-18 (single binary hash), DEP-20 (watchdog singleton mutex).

```bash
# DEP-18: All pods must have SAME binary_sha256 (single-binary-tier policy)
HASHES=""
for IP in $PODS; do
  HASH=$(curl -s http://$IP:8090/health | jq -r '.binary_sha256 // .build_id')
  HASHES="$HASHES\n$IP: $HASH"
done
echo -e "$HASHES"
UNIQUE=$(echo -e "$HASHES" | awk -F': ' '{print $2}' | sort -u | wc -l)
echo "Unique binaries: $UNIQUE (should be 1)"

# DEP-20: Server watchdog PowerShell — must be exactly 0 or 1 instance
SERVER_PS=$(curl -s -X POST http://192.168.31.23:8090/exec \
  -d '{"cmd":"tasklist /NH | find /C \"powershell.exe\""}' | jq -r '.stdout' | tr -d '[:space:]')
echo "Server PowerShell instances: $SERVER_PS (should be 0-1)"

# Verify server watchdog has singleton mutex
curl -s -X POST http://192.168.31.23:8090/exec \
  -d '{"cmd":"findstr /C:\"Global\\RaceControlWatchdog\" C:\\RacingPoint\\start-racecontrol-watchdog.ps1 2>nul || echo NO_MUTEX"}'
```

**Fix loop trigger:** Unique binaries > 1 (pod running different build), PowerShell > 1 on server (watchdog multiplication), no singleton mutex in watchdog script.

---

# TIER 13: Registry & Relay Integrity (Phase 54)

## Phase 54: Command Registry & Shell Relay
**What:** Comms-link command registry populated, dynamic registration works, shell relay allowlist enforced.
**Standing rules:** COM-08 (static registry), COM-09 (dynamic), COM-11 (shell allowlist).

```bash
cd C:/Users/bono/racingpoint/comms-link

# COM-08: Static command registry has all expected commands
REGISTRY=$(curl -s http://localhost:8766/relay/registry 2>/dev/null)
echo "$REGISTRY" | jq 'keys' 2>/dev/null || echo "REGISTRY ENDPOINT DOWN"

# Verify core commands exist
for CMD in git_pull git_status node_version health_check pm2_status uptime; do
  echo "$REGISTRY" | jq -e ".\"$CMD\"" > /dev/null 2>&1 \
    && echo "  OK: $CMD" || echo "  MISSING: $CMD"
done

# COM-09: Dynamic registration works (register + verify + unregister)
curl -s -X POST http://localhost:8766/relay/registry/register \
  -H "Content-Type: application/json" \
  -d '{"name":"audit_test","command":"echo audit_ok","tier":"PUBLIC"}' 2>/dev/null
FOUND=$(curl -s http://localhost:8766/relay/registry 2>/dev/null | jq -e '.audit_test' 2>/dev/null && echo "YES" || echo "NO")
echo "Dynamic registration: $FOUND"
# Cleanup
curl -s -X DELETE http://localhost:8766/relay/registry/audit_test 2>/dev/null

# COM-11: Shell relay binary allowlist enforced
ALLOWLIST=$(grep -o "ALLOWED_BINARIES.*=.*\[.*\]" shared/shell-relay-handler.js 2>/dev/null \
  || grep -o "allowedBinaries.*=.*\[.*\]" shared/shell-relay-handler.js 2>/dev/null)
echo "Shell allowlist: $ALLOWLIST"
# Must contain: node, git, pm2, cargo, systemctl, curl, sqlite3
for BIN in node git pm2 cargo curl sqlite3; do
  echo "$ALLOWLIST" | grep -q "$BIN" && echo "  OK: $BIN" || echo "  MISSING: $BIN"
done
```

**Fix loop trigger:** Registry endpoint down, core commands missing, dynamic registration fails, shell allowlist missing expected binaries.

---

# TIER 14: Data Integrity Deep (Phases 55-56)

## Phase 55: DB Migration Completeness
**What:** Every column used in sync/query code has a corresponding ALTER TABLE migration for existing DBs.
**Standing rule:** PRO-04 (migrations must cover ALL consumers with ALTER TABLE).

```bash
cd C:/Users/bono/racingpoint/racecontrol

# List all migrations
ls -la migrations/ 2>/dev/null || echo "No migrations dir"

# Check: every CREATE TABLE has matching ALTER TABLE for key columns
# Focus on sync-critical columns: updated_at, synced_at, deleted_at
for COL in updated_at synced_at deleted_at; do
  CREATE_COUNT=$(grep -rn "$COL" migrations/ --include="*.sql" | grep -ci "CREATE TABLE")
  ALTER_COUNT=$(grep -rn "$COL" migrations/ --include="*.sql" | grep -ci "ALTER TABLE.*ADD")
  echo "$COL: CREATE mentions=$CREATE_COUNT, ALTER mentions=$ALTER_COUNT"
done

# Check venue DB — all sync tables have updated_at
curl -s -X POST http://192.168.31.23:8090/exec \
  -d '{"cmd":"for %t in (drivers wallets billing_sessions pricing_tiers cafe_orders cafe_menu_items feature_flags activity_log reservations debit_intents) do @sqlite3 C:\\RacingPoint\\data\\racecontrol.db \"SELECT CASE WHEN COUNT(*)>0 THEN '\''OK'\'' ELSE '\''MISSING'\'' END FROM pragma_table_info('\''%t'\'') WHERE name='\''updated_at'\''\" 2>nul"}'

# Cross-check: columns referenced in cloud_sync.rs exist in DB
grep -oP "\"(\w+)\"" crates/racecontrol/src/cloud_sync.rs 2>/dev/null | sort -u | head -20
```

**Fix loop trigger:** Any sync table missing updated_at, CREATE TABLE without matching ALTER TABLE for columns used in sync code.

---

## Phase 56: LOGBOOK & OpenAPI Freshness
**What:** LOGBOOK has entries for recent commits. OpenAPI spec matches actual routes.
**Standing rules:** PRO-10 (LOGBOOK per commit), cascade (OpenAPI freshness).

```bash
cd C:/Users/bono/racingpoint/racecontrol

# PRO-10: Recent commits should have LOGBOOK entries
# Get last 10 commit hashes
RECENT=$(git log --oneline -10 | awk '{print $1}')
for HASH in $RECENT; do
  grep -q "$HASH" LOGBOOK.md 2>/dev/null \
    && echo "  LOGGED: $HASH" || echo "  MISSING: $HASH"
done

# OpenAPI freshness: count endpoints in spec vs actual routes.rs
SPEC_ENDPOINTS=$(grep -c "^\s\+/" docs/openapi.yaml 2>/dev/null || echo 0)
ROUTE_ENDPOINTS=$(grep -c "\.route\|\.get\|\.post\|\.put\|\.delete\|\.patch" \
  crates/racecontrol/src/api/routes.rs 2>/dev/null || echo 0)
echo "OpenAPI endpoints: $SPEC_ENDPOINTS | Code endpoints: $ROUTE_ENDPOINTS"
[ "$SPEC_ENDPOINTS" -lt "$ROUTE_ENDPOINTS" ] && echo "OpenAPI STALE — fewer than code" || echo "OpenAPI OK"

# Shared types freshness: check shared-types package has recent exports
SHARED_EXPORTS=$(grep -c "export" packages/shared-types/src/index.ts 2>/dev/null || echo 0)
echo "Shared type exports: $SHARED_EXPORTS"

# Check last modified dates
ls -la docs/openapi.yaml packages/shared-types/src/index.ts 2>/dev/null
```

**Fix loop trigger:** > 3 recent commits missing from LOGBOOK, OpenAPI endpoint count < code endpoint count, shared-types not updated in > 7 days.

---

# TIER 15: Full Test Suites (Phase 57)

## Phase 57: Racecontrol E2E Test Suite
**What:** Run the full racecontrol E2E test suite (5 phases, 50+ tests). Separate from comms-link tests (Phase 46).
**Standing rules:** TST-06 through TST-13 (E2E phases 1-5, exit code = failure count).

```bash
cd C:/Users/bono/racingpoint/racecontrol

# Phase 1: Preflight — smoke tests (API reachable, JSON valid)
bash tests/e2e/smoke.sh 2>/dev/null && echo "SMOKE: PASS" || echo "SMOKE: FAIL"

# Phase 1b: Cross-process checks
bash tests/e2e/cross-process.sh 2>/dev/null && echo "CROSS-PROCESS: PASS" || echo "CROSS-PROCESS: FAIL"

# Phase 2: API tests — billing lifecycle, game launch
for TEST in tests/e2e/api/*.sh; do
  [ -f "$TEST" ] && {
    NAME=$(basename "$TEST")
    bash "$TEST" 2>/dev/null && echo "API $NAME: PASS" || echo "API $NAME: FAIL"
  }
done

# Phase 4: Deploy verification
bash tests/e2e/deploy/verify.sh 2>/dev/null && echo "DEPLOY VERIFY: PASS" || echo "DEPLOY VERIFY: FAIL"

# Phase 5: Fleet health — all 8 pods
bash tests/e2e/fleet/pod-health.sh 2>/dev/null && echo "FLEET HEALTH: PASS" || echo "FLEET HEALTH: FAIL"

# Full suite (if individual phases pass)
# bash tests/e2e/run-all.sh 2>/dev/null
# EXIT_CODE=$?
# echo "E2E Suite: $EXIT_CODE failures"

# Also run Rust unit tests
cargo test -p rc-common 2>/dev/null && echo "rc-common tests: PASS" || echo "rc-common tests: FAIL"
cargo test -p rc-agent 2>/dev/null && echo "rc-agent tests: PASS" || echo "rc-agent tests: FAIL"
cargo test -p racecontrol 2>/dev/null && echo "racecontrol tests: PASS" || echo "racecontrol tests: FAIL"
```

**Fix loop trigger:** Any E2E phase FAIL, any cargo test failure, exit code > 0.

---

# TIER 16: Cloud & Cross-Boundary E2E (Phase 58)

## Phase 58: Cloud Path E2E
**What:** Cloud PWA path works end-to-end. Bono VPS serves correctly. Cloud sync conflict resolution verified.

```bash
# Cloud racecontrol health
CLOUD_HEALTH=$(curl -s -m 10 http://100.70.177.44:8080/api/v1/health)
echo "Cloud health: $CLOUD_HEALTH" | jq '{status: .status, build_id: .build_id}'

# Cloud PWA reachable (via Bono relay)
curl -s -X POST http://localhost:8766/relay/exec/run \
  -d '{"command":"pm2_status","reason":"audit cloud apps"}'

# Cloud sync — verify BOTH directions
# Venue → Cloud: check last sync push timestamp
curl -s "http://192.168.31.23:8080/api/v1/logs?lines=30" | grep -i "sync.*push\|cloud.*upsert" | tail -3

# Cloud → Venue: check last sync pull
curl -s "http://192.168.31.23:8080/api/v1/logs?lines=30" | grep -i "sync.*pull\|fetched.*drivers\|fetched.*pricing" | tail -3

# Conflict resolution: venue-authoritative tables should not be overwritten by cloud
# Check billing_sessions source — should always be venue
curl -s "http://192.168.31.23:8080/api/v1/logs?lines=100" | grep -i "conflict\|merge.*billing\|overwrite" | head -5

# Cloud DB driver count vs venue DB driver count (should be close)
VENUE_DRIVERS=$(curl -s http://192.168.31.23:8080/api/v1/drivers -H "x-terminal-session: $SESSION" | jq 'length' 2>/dev/null)
CLOUD_DRIVERS=$(curl -s -X POST http://localhost:8766/relay/exec/run \
  -d '{"command":"shell","args":"sqlite3 /root/racecontrol/data/racecontrol.db \"SELECT COUNT(*) FROM drivers\"","reason":"audit"}' \
  | jq -r '.result.stdout' 2>/dev/null | tr -d '[:space:]')
echo "Drivers — Venue: $VENUE_DRIVERS | Cloud: $CLOUD_DRIVERS"
[ "$VENUE_DRIVERS" = "$CLOUD_DRIVERS" ] && echo "SYNC OK" || echo "DRIVER COUNT MISMATCH"

# Bono comms-link relay chain (bidirectional proof)
curl -s -X POST http://localhost:8766/relay/chain/run \
  -d '{"steps":[{"command":"node_version"},{"command":"git_status"}]}' | jq '.success'
```

**Fix loop trigger:** Cloud health down, sync not flowing both directions, driver count mismatch > 5, relay chain fails.

---

# TIER 17: Customer & Staff Flow E2E (Phase 59)

## Phase 59: Customer Flow E2E
**What:** Complete customer flows that cross system boundaries — QR registration, PIN redeem, WhatsApp receipt, cafe order.

```bash
# QR Registration page loads (/register)
QR_OK=$(curl -s http://192.168.31.23:8080/register | grep -c "html\|DOCTYPE\|register")
echo "QR Registration page: $([ $QR_OK -gt 0 ] && echo 'OK' || echo 'MISSING')"

# PIN Redeem endpoint responds (POST with test PIN should return error, not 500)
PIN_STATUS=$(curl -s -o /dev/null -w "%{http_code}" -X POST http://192.168.31.23:8080/api/v1/customer/redeem-pin \
  -H "Content-Type: application/json" -d '{"pin":"000000"}')
echo "PIN Redeem endpoint: $PIN_STATUS (expect 400/404, NOT 500)"

# WhatsApp receipt flow — check PDF generation capability
curl -s "http://192.168.31.23:8080/api/v1/logs?lines=100" | grep -i "receipt.*pdf\|pdf.*generat\|whatsapp.*receipt" | tail -3

# Cafe order endpoint responds
CAFE_MENU=$(curl -s http://192.168.31.23:8080/api/v1/cafe/menu -H "x-terminal-session: $SESSION" | jq 'length' 2>/dev/null)
echo "Cafe menu items: $CAFE_MENU"

# Cafe order creation (dry check — verify endpoint accepts POST structure)
CAFE_ORDER_STATUS=$(curl -s -o /dev/null -w "%{http_code}" -X POST http://192.168.31.23:8080/api/v1/cafe/orders \
  -H "x-terminal-session: $SESSION" -H "Content-Type: application/json" \
  -d '{"items":[],"payment_method":"cash"}')
echo "Cafe order endpoint: $CAFE_ORDER_STATUS (expect 400 for empty items, NOT 500)"

# Lock screen HTTP server on pods (customer PIN/QR entry)
SPOT_POD=$(echo $PODS | awk '{print $1}')
curl -s -X POST http://$SPOT_POD:8090/exec \
  -d '{"cmd":"netstat -an | findstr LISTENING | findstr 18923"}'
# Port may vary — check rc-agent config for lock_screen port

# Overlay HUD endpoint on pods (:18925)
curl -s -X POST http://$SPOT_POD:8090/exec \
  -d '{"cmd":"netstat -an | findstr LISTENING | findstr 18925"}'
echo "(overlay HUD port — should be LISTENING during active sessions)"
```

**Fix loop trigger:** QR page missing, PIN redeem returns 500, cafe order returns 500, lock screen port not listening, overlay port not listening during sessions.

---

# TIER 18: Cross-System Chain E2E (Phase 60)

## Phase 60: Cross-System Chain E2E
**What:** Multi-module data flows that span 3+ systems — the chains that break silently.

```bash
# CHAIN 1: Feature Flag → Config Push → Pod Reload
# Check a flag exists, then verify pod sees it
FLAG_NAME=$(curl -s http://192.168.31.23:8080/api/v1/flags -H "x-terminal-session: $SESSION" \
  | jq -r '.[0].name // empty' 2>/dev/null)
if [ -n "$FLAG_NAME" ]; then
  echo "Testing flag chain: $FLAG_NAME"
  # Verify pod has fetched this flag
  SPOT_POD=$(echo $PODS | awk '{print $1}')
  curl -s -X POST http://$SPOT_POD:8090/exec \
    -d "{\"cmd\":\"findstr /C:\\\"$FLAG_NAME\\\" C:\\\\RacingPoint\\\\rc-agent-*.jsonl 2>nul\"}" \
    | jq -r '.stdout' | tail -1
else
  echo "No flags defined — chain test SKIPPED"
fi

# CHAIN 2: Game Launch → Telemetry → Lap Record → Leaderboard
# (Can only fully test during active sessions — check log evidence)
curl -s "http://192.168.31.23:8080/api/v1/logs?lines=200" \
  | grep -i "game.*launch\|telemetry.*frame\|lap.*record\|leaderboard.*update" | tail -5
echo "(Chain 2: need active session for full E2E — log evidence above)"

# CHAIN 3: Badge Award → Psychology Dispatcher → Notification → WhatsApp/Discord
curl -s "http://192.168.31.23:8080/api/v1/logs?lines=200" \
  | grep -i "badge.*award\|nudge_queue\|notification.*dispatch\|bot_coordinator\|whatsapp.*send\|discord.*post" | tail -5
echo "(Chain 3: need customer activity for full E2E — log evidence above)"

# CHAIN 4: Session End → Safe State → FFB Zero → Billing Complete → Activity Log
curl -s "http://192.168.31.23:8080/api/v1/logs?lines=200" \
  | grep -i "session.*end\|safe_state\|ffb.*zero\|billing.*complete\|activity_log.*insert" | tail -5

# CHAIN 5: Error Rate → Error Aggregator → Fleet Alert → WhatsApp Escalation
curl -s "http://192.168.31.23:8080/api/v1/logs?lines=200" \
  | grep -i "error_rate\|error_aggregator\|fleet_alert\|escalat" | tail -5

# CHAIN 6: Refund → Wallet Update → Activity Log entry
curl -s "http://192.168.31.23:8080/api/v1/logs?lines=200" \
  | grep -i "refund.*process\|wallet.*debit\|wallet.*credit\|activity.*refund" | tail -3

# CHAIN 7: OTA Pipeline state (if v22.0 Phase 179 is active)
curl -s "http://192.168.31.23:8080/api/v1/logs?lines=50" \
  | grep -i "ota.*state\|ota.*transition\|ota.*download\|ota.*rollback" | tail -3
echo "(Chain 7: OTA pipeline — Phase 179 may still be in development)"

# CHAIN 8: Remote Terminal (cloud → venue exec)
curl -s -X POST http://localhost:8766/relay/exec/run \
  -d '{"command":"health_check","reason":"audit chain 8: remote terminal"}'
echo "(Chain 8: Bono → venue exec round-trip)"

# Webterm alive (Uday's phone terminal)
curl -s -m 3 http://localhost:9999 > /dev/null 2>&1 && echo "Webterm :9999: UP" || echo "Webterm :9999: DOWN"

# People tracker alive
curl -s -m 3 http://localhost:8095/health 2>/dev/null && echo "People tracker :8095: UP" || echo "People tracker :8095: DOWN"
```

**Fix loop trigger:** Any chain showing zero log evidence during operating hours, flag not reaching pods, relay exec fails, webterm/people-tracker DOWN during business hours.

---

# Audit Summary Template

```
AUDIT DATE: _______________
AUDITOR: _______________
DURATION: _______________

TIER 1: INFRASTRUCTURE (1-10)
| # | Phase                    | Status | Notes |
|---|--------------------------|--------|-------|
| 1 | Fleet Inventory          |        |       |
| 2 | Config Integrity         |        |       |
| 3 | Network & Tailscale      |        |       |
| 4 | Firewall & Ports         |        |       |
| 5 | Pod Power & WoL          |        |       |
| 6 | Orphan Processes         |        |       |
| 7 | Process Guard & Allowlist|        |       |
| 8 | Sentinel Files           |        |       |
| 9 | Self-Monitor & Self-Heal |        |       |
|10 | AI Healer / Watchdog     |        |       |

TIER 2: CORE SERVICES (11-16)
| # | Phase                    | Status | Notes |
|---|--------------------------|--------|-------|
|11 | API Data Integrity       |        |       |
|12 | WebSocket Flows          |        |       |
|13 | rc-agent Exec            |        |       |
|14 | rc-sentry Health         |        |       |
|15 | Preflight Checks         |        |       |
|16 | Cascade Guard & Recovery |        |       |

TIER 3: DISPLAY & UX (17-20)
| # | Phase                    | Status | Notes |
|---|--------------------------|--------|-------|
|17 | Lock Screen & Blanking   |        |       |
|18 | Overlay Suppression      |        |       |
|19 | Display Resolution       |        |       |
|20 | Kiosk Browser Health     |        |       |

TIER 4: BILLING & COMMERCE (21-25)
| # | Phase                    | Status | Notes |
|---|--------------------------|--------|-------|
|21 | Pricing & Billing        |        |       |
|22 | Wallet & Payments        |        |       |
|23 | Reservations & Booking   |        |       |
|24 | Accounting               |        |       |
|25 | Cafe Menu & Inventory    |        |       |

TIER 5: GAMES & HARDWARE (26-29)
| # | Phase                    | Status | Notes |
|---|--------------------------|--------|-------|
|26 | Game Catalog & Launcher  |        |       |
|27 | AC Server & Telemetry    |        |       |
|28 | FFB & Hardware           |        |       |
|29 | Multiplayer & Friends    |        |       |

TIER 6: NOTIFICATIONS & MARKETING (30-34)
| # | Phase                    | Status | Notes |
|---|--------------------------|--------|-------|
|30 | WhatsApp Alerter         |        |       |
|31 | Email Alerts             |        |       |
|32 | Discord Integration      |        |       |
|33 | Cafe Marketing & PNG     |        |       |
|34 | Psychology & Gamification|        |       |

TIER 7: DATA & SYNC (35-38)
| # | Phase                    | Status | Notes |
|---|--------------------------|--------|-------|
|35 | Cloud Sync Bidirectional |        |       |
|36 | DB Schema & Migrations   |        |       |
|37 | Activity Log & Compliance|        |       |
|38 | Bono Relay & Failover    |        |       |

TIER 8: ADVANCED SYSTEMS (39-42)
| # | Phase                    | Status | Notes |
|---|--------------------------|--------|-------|
|39 | Feature Flags            |        |       |
|40 | Scheduler & Action Queue |        |       |
|41 | Config Push & OTA        |        |       |
|42 | Error Aggregator & Alerts|        |       |

TIER 9: CAMERAS & AI (43-44)
| # | Phase                    | Status | Notes |
|---|--------------------------|--------|-------|
|43 | Camera Pipeline          |        |       |
|44 | Face Detection & Counter |        |       |

TIER 10: OPS & COMPLIANCE (45-47)
| # | Phase                    | Status | Notes |
|---|--------------------------|--------|-------|
|45 | Log Health & Rotation    |        |       |
|46 | Comms-Link E2E           |        |       |
|47 | Standing Rules Compliance|        |       |

TIER 11: E2E JOURNEYS (48-50)
| # | Phase                    | Status | Notes |
|---|--------------------------|--------|-------|
|48 | Customer Journey E2E     |        |       |
|49 | Staff / POS Journey E2E  |        |       |
|50 | Security & Auth E2E      |        |       |

TIER 12: CODE QUALITY (51-53)
| # | Phase                    | Status | Notes |
|---|--------------------------|--------|-------|
|51 | Static Code Analysis     |        |       |
|52 | Frontend Deploy Integrity|        |       |
|53 | Binary Consistency       |        |       |

TIER 13: REGISTRY & RELAY (54)
| # | Phase                    | Status | Notes |
|---|--------------------------|--------|-------|
|54 | Command Registry & Shell |        |       |

TIER 14: DATA INTEGRITY DEEP (55-56)
| # | Phase                    | Status | Notes |
|---|--------------------------|--------|-------|
|55 | DB Migration Completeness|        |       |
|56 | LOGBOOK & OpenAPI Fresh  |        |       |

TIER 15: FULL TEST SUITES (57)
| # | Phase                    | Status | Notes |
|---|--------------------------|--------|-------|
|57 | Racecontrol E2E Suite    |        |       |

TIER 16: CLOUD & CROSS-BOUNDARY (58)
| # | Phase                    | Status | Notes |
|---|--------------------------|--------|-------|
|58 | Cloud Path E2E           |        |       |

TIER 17: CUSTOMER & STAFF FLOW (59)
| # | Phase                    | Status | Notes |
|---|--------------------------|--------|-------|
|59 | Customer Flow E2E        |        |       |

TIER 18: CROSS-SYSTEM CHAINS (60)
| # | Phase                    | Status | Notes |
|---|--------------------------|--------|-------|
|60 | Cross-System Chain E2E   |        |       |

OVERALL: PASS / FAIL / PARTIAL
TIERS PASSED: __ / 18
PHASES PASSED: __ / 60
ISSUES FOUND: ___
FIXED DURING AUDIT: ___
DEFERRED: ___
```

```
AUDIT DATE: _______________
AUDITOR: _______________
DURATION: _______________

TIER 1: INFRASTRUCTURE (1-10)
| # | Phase                    | Status | Notes |
|---|--------------------------|--------|-------|
| 1 | Fleet Inventory          |        |       |
| 2 | Config Integrity         |        |       |
| 3 | Network & Tailscale      |        |       |
| 4 | Firewall & Ports         |        |       |
| 5 | Pod Power & WoL          |        |       |
| 6 | Orphan Processes         |        |       |
| 7 | Process Guard & Allowlist|        |       |
| 8 | Sentinel Files           |        |       |
| 9 | Self-Monitor & Self-Heal |        |       |
|10 | AI Healer / Watchdog     |        |       |

TIER 2: CORE SERVICES (11-16)
| # | Phase                    | Status | Notes |
|---|--------------------------|--------|-------|
|11 | API Data Integrity       |        |       |
|12 | WebSocket Flows          |        |       |
|13 | rc-agent Exec            |        |       |
|14 | rc-sentry Health         |        |       |
|15 | Preflight Checks         |        |       |
|16 | Cascade Guard & Recovery |        |       |

TIER 3: DISPLAY & UX (17-20)
| # | Phase                    | Status | Notes |
|---|--------------------------|--------|-------|
|17 | Lock Screen & Blanking   |        |       |
|18 | Overlay Suppression      |        |       |
|19 | Display Resolution       |        |       |
|20 | Kiosk Browser Health     |        |       |

TIER 4: BILLING & COMMERCE (21-25)
| # | Phase                    | Status | Notes |
|---|--------------------------|--------|-------|
|21 | Pricing & Billing        |        |       |
|22 | Wallet & Payments        |        |       |
|23 | Reservations & Booking   |        |       |
|24 | Accounting               |        |       |
|25 | Cafe Menu & Inventory    |        |       |

TIER 5: GAMES & HARDWARE (26-29)
| # | Phase                    | Status | Notes |
|---|--------------------------|--------|-------|
|26 | Game Catalog & Launcher  |        |       |
|27 | AC Server & Telemetry    |        |       |
|28 | FFB & Hardware           |        |       |
|29 | Multiplayer & Friends    |        |       |

TIER 6: NOTIFICATIONS & MARKETING (30-34)
| # | Phase                    | Status | Notes |
|---|--------------------------|--------|-------|
|30 | WhatsApp Alerter         |        |       |
|31 | Email Alerts             |        |       |
|32 | Discord Integration      |        |       |
|33 | Cafe Marketing & PNG     |        |       |
|34 | Psychology & Gamification|        |       |

TIER 7: DATA & SYNC (35-38)
| # | Phase                    | Status | Notes |
|---|--------------------------|--------|-------|
|35 | Cloud Sync Bidirectional |        |       |
|36 | DB Schema & Migrations   |        |       |
|37 | Activity Log & Compliance|        |       |
|38 | Bono Relay & Failover    |        |       |

TIER 8: ADVANCED SYSTEMS (39-42)
| # | Phase                    | Status | Notes |
|---|--------------------------|--------|-------|
|39 | Feature Flags            |        |       |
|40 | Scheduler & Action Queue |        |       |
|41 | Config Push & OTA        |        |       |
|42 | Error Aggregator & Alerts|        |       |

TIER 9: CAMERAS & AI (43-44)
| # | Phase                    | Status | Notes |
|---|--------------------------|--------|-------|
|43 | Camera Pipeline          |        |       |
|44 | Face Detection & Counter |        |       |

TIER 10: OPS & COMPLIANCE (45-47)
| # | Phase                    | Status | Notes |
|---|--------------------------|--------|-------|
|45 | Log Health & Rotation    |        |       |
|46 | Comms-Link E2E           |        |       |
|47 | Standing Rules Compliance|        |       |

TIER 11: E2E JOURNEYS (48-50)
| # | Phase                    | Status | Notes |
|---|--------------------------|--------|-------|
|48 | Customer Journey E2E     |        |       |
|49 | Staff / POS Journey E2E  |        |       |
|50 | Security & Auth E2E      |        |       |

OVERALL: PASS / FAIL / PARTIAL
TIERS PASSED: __ / 11
PHASES PASSED: __ / 50
ISSUES FOUND: ___
FIXED DURING AUDIT: ___
DEFERRED: ___
```

---

# Quick Audit Mode (Tiers 1-2 only)

For daily health checks, run only **Phases 1-16** (Infrastructure + Core Services).
Takes ~15 minutes. Covers all critical operational risks.

# Standard Audit Mode (Tiers 1-11, Phases 1-50)

Runtime health + all user-facing paths. Run after deploys or weekly.
Takes ~45-60 minutes. Covers all runtime systems.

# Full Audit Mode (All 60 phases, all 18 tiers)

Complete protocol including static analysis, test suites, cloud paths, and cross-system chains.
Run before milestone ship, after major incidents, or bi-weekly.
Takes ~90-120 minutes. **100% coverage** of all standing rules, runtime modules, and E2E journeys.

# Pre-Ship Audit Mode (Critical subset)

Before marking ANY milestone shipped, run at minimum:
1. **Phase 1** (Fleet Inventory) — all binaries match HEAD
2. **Phase 51** (Static Code Analysis) — no new anti-patterns
3. **Phase 53** (Binary Consistency) — all pods identical
4. **Phase 57** (Racecontrol E2E Suite) — all tests pass
5. **Phase 46** (Comms-Link E2E) — Quality Gate pass
6. **Phase 48-50** (E2E Journeys) — customer/staff paths work
7. **Phase 58** (Cloud Path) — cloud sync verified

# Post-Incident Audit Mode

After a major incident, run:
1. **Phase 1** (Fleet Inventory) — confirm all binaries running
2. **Phase 8** (Sentinel Files) — clear any MAINTENANCE_MODE
3. **The tier related to the incident** — e.g., Tier 3 for display issues
4. **Phase 48-50** (E2E Journeys) — confirm customer/staff paths work
5. **Phase 60** (Cross-System Chains) — verify no cascade failures
