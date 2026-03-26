# Phase 211: Safe Scheduling Foundation - Research

**Researched:** 2026-03-26
**Domain:** Bash scheduling (Windows Task Scheduler + Linux cron), PID file locking, sentinel-aware execution, escalation cooldown, venue-state-aware mode selection
**Confidence:** HIGH

## User Constraints (from CONTEXT.md)

### Locked Decisions
None -- all implementation choices are at Claude discretion (pure infrastructure phase).

### Claude Discretion
- PID lock: file-based in /tmp or result directory
- Cooldown state: JSON file tracking last-alert timestamps per pod+issue
- Sentinel check: read OTA_DEPLOYING + MAINTENANCE_MODE via safe_remote_exec before each fix
- Venue state: reuse audit framework venue_state_detect() function
- Task Scheduler: register via schtasks or PowerShell, daily trigger at 02:30

### Deferred Ideas (OUT OF SCOPE)
None -- discussion stayed within phase scope.

## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| SCHED-01 | James auto-detect runs daily at 2:30 AM IST via Windows Task Scheduler | schtasks daily trigger; existing register-james-watchdog.bat is the template |
| SCHED-02 | Bono auto-detect runs daily at 2:35 AM IST via cron (5-min offset) | Current cron is 0 21 * * * (IST 02:30); target is 5 21 * * * (IST 02:35) |
| SCHED-03 | Run guard prevents overlapping auto-detect executions (PID file lock) | comms-link watchdog-runner.js has the canonical PID file guard pattern |
| SCHED-04 | Escalation cooldown prevents repeated WhatsApp alerts within 6 hours | notify.sh 5-min cooldown extended to 6-hour pod+issue-keyed JSON |
| SCHED-05 | Venue-state-aware timing -- full mode when closed, quick mode if open | venue_state_detect() already exists in audit/lib/core.sh |

---

## Summary

Phase 211 adds five safety layers to the auto-detect pipeline before any scheduled execution fires on live infrastructure: a Windows Task Scheduler daily trigger (SCHED-01), a corrected Bono cron schedule (SCHED-02), a PID file run guard (SCHED-03), a 6-hour per-pod+issue escalation cooldown (SCHED-04), and venue-state-aware mode selection (SCHED-05).

All five mechanisms build on verified existing code. The auto-detect.sh script (committed b54e4585) and bono-auto-detect.sh (deployed to VPS) are the foundation. The PID file guard pattern is directly lifted from comms-link/james/watchdog-runner.js (the Fix fork bomb: add PID-file guard commit). The cooldown JSON file extends audit/lib/notify.sh existing 5-minute per-app cooldown. The venue-state logic reuses audit/lib/core.sh venue_state_detect(). The Task Scheduler registration mirrors scripts/register-james-watchdog.bat.

Critical ordering constraint from STATE.md: Phase 211 safety gates must be complete before any schtasks /Create command fires. The scheduled task must not be created until the PID guard, sentinel check, and cooldown are all live in auto-detect.sh.

**Primary recommendation:** Add all five safety mechanisms to auto-detect.sh in a single wave, then register the Task Scheduler task last (not before). Correct Bono cron as a separate VPS command via relay.

---

## Standard Stack

### Core

| Component | Version/Location | Purpose | Why Standard |
|-----------|-----------------|---------|--------------|
| scripts/auto-detect.sh | commit b54e4585 | 6-step pipeline entry point | Already deployed; all modifications go here |
| audit/lib/core.sh | repo | venue_state_detect(), safe_remote_exec(), ist_now() | Canonical audit primitives -- do not reinvent |
| audit/lib/fixes.sh | repo | is_pod_idle(), check_pod_sentinels(), APPROVED_FIXES | Sentinel and billing gate already implemented here |
| audit/lib/notify.sh | repo | WhatsApp + Bono WS notification | Cooldown pattern to extend to 6-hour pod+issue keyed JSON |
| schtasks.exe | Windows built-in | Task Scheduler registration | Existing pattern in register-james-watchdog.bat |
| cron | Linux built-in on Bono VPS | Bono-side schedule | Already configured -- single crontab line edit |

### Supporting

| Component | Purpose | When to Use |
|-----------|---------|-------------|
| /tmp/auto-detect.pid | PID lock file | Written at startup, removed at EXIT trap |
| audit/results/auto-detect-cooldown.json | 6-hour cooldown state | Keyed by pod_ip:issue_type; checked before WhatsApp alert |
| scripts/register-auto-detect-task.bat | Task Scheduler registration (new file) | Run once as Administrator on James machine |

---

## Architecture Patterns

### Recommended File Layout After Phase 211

    scripts/
      auto-detect.sh               MODIFIED: +PID guard, +sentinel check, +cooldown, +venue-aware mode
      bono-auto-detect.sh          UNCHANGED: script unchanged; only cron schedule corrected on VPS
      register-auto-detect-task.bat  NEW: schtasks /Create for daily 02:30 IST trigger
    audit/
      results/
        auto-detect-cooldown.json  NEW: runtime cooldown state (gitignored, not committed)

### Pattern 1: PID File Run Guard (bash)

**What:** Write own PID to lock file at start; check at next invocation via kill -0; remove via EXIT trap.

**Canonical source:** comms-link/james/watchdog-runner.js (Fix fork bomb: add PID-file guard to watchdog-runner commit).

Bash translation for auto-detect.sh (insert at top of main(), before any pipeline steps):

    PID_FILE="/tmp/auto-detect.pid"

    _acquire_run_lock() {
      if [[ -f "$PID_FILE" ]]; then
        local existing_pid
        existing_pid=$(cat "$PID_FILE" 2>/dev/null | tr -d ")[:space:]")
        if [[ -n "$existing_pid" ]] && kill -0 "$existing_pid" 2>/dev/null; then
          log INFO "auto-detect already running (PID $existing_pid). Exiting."
          exit 0
        fi
        rm -f "$PID_FILE"
      fi
      echo $$ > "$PID_FILE"
      log INFO "PID lock acquired (PID $$)"
    }

    trap "rm -f $PID_FILE" EXIT
    _acquire_run_lock

kill -0 PID tests process existence without sending a signal. Works on bash/Linux (Bono) and Git Bash on Windows (James). If the process is dead, kill -0 returns non-zero -- guard removes stale file and proceeds.

### Pattern 2: Sentinel-Aware Fix Gate

**What:** Before applying any fix, check OTA_DEPLOYING and MAINTENANCE_MODE sentinels on the target pod via safe_remote_exec. Current check_pod_sentinels() in fixes.sh checks only OTA_DEPLOYING -- Phase 211 extends it.

Extended function (source into auto-detect.sh or update fixes.sh directly):

    check_pod_sentinels_extended() {
      local pod_ip="$1"
      local result
      result=$(safe_remote_exec "$pod_ip" 8090         "if exist C:\RacingPoint\OTA_DEPLOYING echo OTA_ACTIVE & if exist C:\RacingPoint\MAINTENANCE_MODE echo MM_ACTIVE" 10)
      if printf "%s" "$result" | grep -qE "OTA_ACTIVE|MM_ACTIVE"; then
        log WARN "Sentinel active on $pod_ip -- skipping fix"
        return 1
      fi
      return 0
    }

CRITICAL: Sentinel check must be called before EACH individual fix action, not once at pipeline start. OTA_DEPLOYING can be written during the pipeline run.

Only call with pod IPs (192.168.31.89/.33/.28/.88/.86/.87/.38/.91). Never with server IP (192.168.31.23) -- server uses port 8090 for server_ops.

### Pattern 3: 6-Hour Per-Pod+Issue Escalation Cooldown

**What:** JSON file tracks last_alert_ts keyed by "pod_ip:issue_type". Before any WhatsApp alert, check if same combination fired within 6 hours.

**Rationale:** notify.sh has a 5-minute per-app cooldown (ALERT_COOLDOWN_SECS=300). SCHED-04 requires a longer 6-hour cooldown per individual issue per pod -- complementary, not replacing.

Functions to add to auto-detect.sh:

    COOLDOWN_FILE="$REPO_ROOT/audit/results/auto-detect-cooldown.json"
    ESCALATION_COOLDOWN_SECS=21600

    _is_cooldown_active() {
      local pod="$1" issue="$2"
      local key="${pod}:${issue}"
      if [[ ! -f "$COOLDOWN_FILE" ]]; then return 1; fi
      local last_ts now_ts elapsed
      last_ts=$(jq -r --arg key "$key" ".[\] // 0" "$COOLDOWN_FILE" 2>/dev/null || echo "0")
      now_ts=$(date +%s)
      elapsed=$(( now_ts - last_ts ))
      if [[ "$elapsed" -lt "$ESCALATION_COOLDOWN_SECS" ]]; then return 0; fi
      return 1
    }

    _record_alert() {
      local pod="$1" issue="$2"
      local key="${pod}:${issue}"
      local now_ts existing
      now_ts=$(date +%s)
      existing="{}"
      [[ -f "$COOLDOWN_FILE" ]] && existing=$(cat "$COOLDOWN_FILE" 2>/dev/null || echo "{}")
      printf "%s" "$existing" | jq --arg key "$key" --argjson ts "$now_ts"         ".[\] = " > "${COOLDOWN_FILE}.tmp" && mv "${COOLDOWN_FILE}.tmp" "$COOLDOWN_FILE"
    }

Usage (before any WhatsApp alert):

    if ! _is_cooldown_active "$pod_ip" "ws_disconnected"; then
      send_whatsapp_alert "Pod $pod_ip: ws_disconnected"
      _record_alert "$pod_ip" "ws_disconnected"
    fi

Since auto-detect.sh is singleton via PID guard (SCHED-03), concurrent cooldown.json writes are impossible after Phase 211.

### Pattern 4: Venue-State-Aware Mode Selection

**What:** After arg parsing, call venue_state_detect(). If "open", force MODE=quick.

Insert after existing arg parsing block in auto-detect.sh:

    source "$REPO_ROOT/audit/lib/core.sh"
    FLEET_HEALTH_ENDPOINT="${SERVER_URL}/api/v1/fleet/health"
    export FLEET_HEALTH_ENDPOINT

    DETECTED_VENUE_STATE=$(venue_state_detect 2>/dev/null || echo "closed")
    if [[ "$DETECTED_VENUE_STATE" == "open" ]] && [[ "$MODE" != "quick" ]]; then
      log WARN "Venue OPEN -- overriding mode to quick (SCHED-05)"
      MODE="quick"
    fi
    log INFO "Venue state: $DETECTED_VENUE_STATE | Effective mode: $MODE"

venue_state_detect() checks active billing sessions first, then falls back to IST time 09:00-22:00.

Note: audit/lib/core.sh functions are exported via export -f. Check [[ $(type -t venue_state_detect) == "function" ]] before sourcing to avoid double-sourcing.

### Pattern 5: Windows Task Scheduler Daily Trigger at 02:30 IST

James machine is IST-configured. schtasks uses machine local time. No UTC conversion needed.

Template for register-auto-detect-task.bat (following .bat standing rules: no parentheses in if/else, goto labels, CRLF):

    @echo off
    set BASH=C:\Program Files\Gitinash.exe
    set TASK_NAME=AutoDetect-Daily

    schtasks /Delete /TN "%TASK_NAME%" /F 2>/dev/null
    echo [1] Cleared old task

    schtasks /Create /TN "%TASK_NAME%" /TR "..." /SC DAILY /ST 02:30 /RU SYSTEM /RL HIGHEST /F
    if %ERRORLEVEL% neq 0 goto fail

    echo [2] Task registered at 02:30 daily
    schtasks /Query /TN "%TASK_NAME%" /FO LIST
    goto done

    :fail
    echo ERROR: Failed to register task
    exit /b 1

    :done
    echo [OK] Auto-detect daily task registered
    exit /b 0

Key details:
- /SC DAILY /ST 02:30: daily at 02:30 local time (IST on James machine)
- /RU SYSTEM /RL HIGHEST: matches existing CommsLink-DaemonWatchdog pattern
- /F: force overwrite, idempotent registration
- AUDIT_PIN baked into task command (env vars do not persist in SYSTEM context)
- Pre-registration: run "where bash" in cmd.exe on James to verify Git Bash path

### Pattern 6: Bono Cron Correction

Verified current state (live SSH during research):

    0 21 * * * AUDIT_PIN=261121 bash /root/racecontrol/scripts/bono-auto-detect.sh >> /root/auto-detect-logs/cron.log 2>&1

UTC 21:00 = IST 02:30 -- same time as James. Target: UTC 21:05 = IST 02:35. Change: "0 21" to "5 21".

Fix via relay (standing rule: prefer relay over direct SSH). Write JSON payload to file, then relay exec:

    (crontab -l | grep -v bono-auto-detect; echo "5 21 * * * AUDIT_PIN=261121 bash /root/racecontrol/scripts/bono-auto-detect.sh >> /root/auto-detect-logs/cron.log 2>&1") | crontab -

Verification: relay exec crontab -l, check output contains "5 21" not "0 21".

### Anti-Patterns to Avoid

- Registering Task Scheduler task before safety gates are in auto-detect.sh: task fires next night without protection.
- Global sentinel check once at pipeline start: sentinels can be written mid-run. Check before each fix.
- Calling sentinel check with server IP (.23): server uses port 8090 for server_ops.
- Double-sourcing core.sh: venue_state_detect is exported via export -f.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Venue state detection | Custom time-check logic | venue_state_detect() in audit/lib/core.sh | Handles billing-state + time-window; tested in 60-phase audit |
| Sentinel checking | New remote exec logic | Extend check_pod_sentinels() in fixes.sh | safe_remote_exec has cmd.exe quoting rules baked in |
| WhatsApp notifications | Direct Evolution API call | notify.sh functions | Three-channel pattern with failure isolation |
| Task Scheduler registration | PowerShell New-ScheduledTask | schtasks /Create in .bat file | Matches existing pattern; .bat rules: CRLF + goto |

---

## Common Pitfalls

### Pitfall 1: schtasks SYSTEM context cannot find Git Bash
**What goes wrong:** /RU SYSTEM runs in non-interactive session where bash.exe is not on PATH.
**Why it happens:** SYSTEM PATH differs from user PATH; Git Bash is user-installed.
**How to avoid:** Use full absolute path. Verify with 'where bash' in cmd.exe on James.
**Warning signs:** Task shows Last Run Result 0x1 or 0x2 in Task Scheduler.

### Pitfall 2: PID file stale on crash
**What goes wrong:** SIGKILL bypasses EXIT trap, leaving stale PID file.
**How to avoid:** Guard handles this: if kill -0 fails (process dead), stale file is removed. Same logic as watchdog-runner.js.

### Pitfall 3: Cooldown JSON not gitignored
**What goes wrong:** Runtime cooldown timestamps committed to git.
**How to avoid:** Verify .gitignore includes audit/results/ before writing cooldown file.

### Pitfall 4: Venue OPEN at 02:30 due to unclosed session
**What goes wrong:** Test session left open causes venue_state_detect() to return 'open', forcing quick mode.
**How to avoid:** Log the override reason. Do not change venue_state_detect() -- behavior is correct per SCHED-05.

### Pitfall 5: Bono cron race if James fires late
**What goes wrong:** James fires 3+ minutes late; Bono fires at 02:35; both run concurrently.
**How to avoid:** James machine should not sleep at 02:30. Cross-machine race is Phase 214 concern. Each side PID guard independently prevents its own overlaps.

### Pitfall 6: Registering task before script safety is complete
**What goes wrong:** Task fires next night without PID guard, cooldown, or sentinel check.
**How to avoid:** Commit all 5 mechanisms to auto-detect.sh and verify with --dry-run BEFORE running register-auto-detect-task.bat.

---

## State of the Art

| Old Approach | Current Approach | Impact |
|--------------|-----------------|--------|
| Manual trigger before leaving venue | Scheduled execution at 02:30 IST | Zero human trigger required |
| No run guard -- double-trigger possible | PID file lock (comms-link pattern) | Second trigger exits immediately |
| No WhatsApp throttle | 6-hour per-pod+issue cooldown JSON | Uday not woken repeatedly for same issue |
| Mode from --mode arg only | venue_state_detect() gates mode | Accidental open-hours run uses quick mode |
| Bono at 02:30 (same as James) | Bono at 02:35 (5-min offset) | No simultaneous audit load on venue server |

---

## Open Questions

1. **Git Bash path on James machine**
   - What we know: Standard Git for Windows installs to C:\Program Files\Git\bin\bash.exe
   - What is unclear: James machine may have non-standard install
   - Recommendation: Planner task must include 'where bash' verification before writing the .bat

2. **SYSTEM account network access at 02:30**
   - What we know: CommsLink-DaemonWatchdog runs as SYSTEM and accesses the network (WS relay)
   - What is unclear: Whether SYSTEM on James can reach 192.168.31.23:8080 at 02:30 AM specifically
   - Recommendation: Established pattern; if network fails, audit records WARN and exits 1 -- acceptable

3. **Cooldown file gitignore status**
   - What we know: audit/results/ is the established results directory
   - What is unclear: Whether audit/results/ is currently in .gitignore
   - Recommendation: Planner task should verify .gitignore before writing cooldown file

---

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | bash manual assertions (no formal test runner; comms-link test/run-all.sh covers quality gate) |
| Config file | none for phase 211 (shell scripts only) |
| Quick run command | bash scripts/auto-detect.sh --dry-run --no-notify |
| Full suite command | COMMS_PSK=... bash comms-link/test/run-all.sh |

### Phase Requirements to Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| SCHED-01 | Task registered at 02:30 daily | smoke | schtasks /Query /TN AutoDetect-Daily /FO LIST | No -- Wave 0: register-auto-detect-task.bat |
| SCHED-02 | Bono cron at 02:35 IST | smoke | relay exec crontab -l, check for '5 21' | Yes -- verify post-edit |
| SCHED-03 | Double-trigger exits with already running | unit | Two concurrent auto-detect.sh --dry-run; second exits 0 | No -- Wave 0: test-sched-03-pid-guard.sh |
| SCHED-04 | 6-hour cooldown suppresses repeated alerts | unit | Seed cooldown.json now-3600 verify suppressed; now-25200 verify fires | No -- Wave 0: test-sched-04-cooldown.sh |
| SCHED-05 | Quick mode when venue open | unit | Mock fleet with active billing; --dry-run; verify MODE=quick | No -- Wave 0: verify in dry-run output |

### Sampling Rate
- **Per task commit:** bash scripts/auto-detect.sh --dry-run --no-notify
- **Per wave merge:** COMMS_PSK=... bash comms-link/test/run-all.sh
- **Phase gate:** All 5 requirements verified before /gsd:verify-work

### Wave 0 Gaps
- [ ] scripts/register-auto-detect-task.bat -- new file, covers SCHED-01
- [ ] test/test-sched-03-pid-guard.sh -- SCHED-03 double-trigger test
- [ ] test/test-sched-04-cooldown.sh -- SCHED-04 cooldown unit test
- [ ] Verify audit/results/ in .gitignore -- covers cooldown file hygiene

---

## Sources

### Primary (HIGH confidence)
- comms-link/james/watchdog-runner.js -- PID file guard pattern (production code, Fix fork bomb commit)
- audit/lib/core.sh -- venue_state_detect(), safe_remote_exec(), ist_now() (production, 60-phase audit)
- audit/lib/fixes.sh -- check_pod_sentinels(), is_pod_idle() (production, v23.0)
- audit/lib/notify.sh -- cooldown pattern, three-channel notification (production, v23.0)
- scripts/auto-detect.sh -- 6-step pipeline (commit b54e4585, read in full during research)
- scripts/bono-auto-detect.sh -- Bono VPS side (deployed, read in full during research)
- scripts/register-james-watchdog.bat -- schtasks registration pattern (production, in use)
- Live SSH check: crontab -l on Bono VPS confirms current cron is '0 21 * * *' (IST 02:30)

### Secondary (MEDIUM confidence)
- Windows Task Scheduler /SC DAILY /ST HH:MM uses machine local time -- consistent with IST-configured James machine
- Git Bash kill -0 PID works for process existence on Windows

### Tertiary (LOW confidence)
- SYSTEM account network access at 02:30 -- assumed based on CommsLink-DaemonWatchdog precedent, not tested at that hour

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- all components are existing production code read during research
- Architecture: HIGH -- PID guard and cooldown patterns directly derived from production comms-link code
- Pitfalls: HIGH -- all documented from real incidents (MAINTENANCE_MODE, schtasks SYSTEM context, stale PID)
- Bono cron current state: HIGH -- live SSH verification performed during research

**Research date:** 2026-03-26
**Valid until:** 2026-04-25 (stable infrastructure; 30-day validity)
