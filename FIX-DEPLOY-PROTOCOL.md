# Fix-Deploy Anti-Regression Protocol v1.0

**Date:** 2026-03-27 | **Author:** James Vowles (3-model audit synthesis)
**Models:** Claude Opus 4.6 (architectural), Claude Sonnet 4.6 (pattern), Claude Haiku 4.5 (checklist)
**Purpose:** Ensure every fix stays fixed and every deploy is complete. Born from 15 documented regressions across 27 days of production operation.

---

## Executive Summary

Three-model audit identified **10 recurring pain points**, **15 regression cycles**, and **14 standing rules with zero automation**. The root cause is structural: 65% of standing rules rely on human memory, health endpoints are proxies that pass while real behavior is broken, and deploys are partial by default (binary swapped, bat/config/POS left stale).

**The three highest-impact fixes:**
1. Extend rc-agent `/health` to expose Session context, MAINTENANCE_MODE, edge_process_count, bat hash, config fetch status
2. Add MAINTENANCE_MODE 30-minute auto-expiry with WhatsApp alert
3. Include POS PC in all fleet operations

---

## Part 1: The 10 Pain Points and Their Structural Fixes

### PP-01: Session 0 Kills All GUI
**Impact:** ALL pods blind — no blanking screen, no games, no overlays
**Recurrences:** 3+ (R-03, plus any schtasks restart)
**Root cause:** rc-agent started by SYSTEM (Session 0) cannot create windows

| Layer | Current State | Required State |
|-------|--------------|----------------|
| Restart mechanism | RCWatchdog service (WTSQueryUserToken) | Same — keep as-is |
| Deploy restart | `start` via rc-sentry exec (Session depends on rc-sentry) | Kill rc-agent, let RCWatchdog restart in Session 1 |
| Audit verification | Not checked | Phase 9b: `tasklist /V /FO CSV \| findstr rc-agent` → Session = Console |
| Health endpoint | No session info | Add `"session_id": 1` to `/health` response |
| Behavioral check | Not automated | Post-deploy: trigger RCAGENT_BLANK_SCREEN → verify `edge_process_count > 0` at `:18924/debug` within 12s |

**Deploy script fix (deploy-pod.sh):**
```bash
# AFTER killing old rc-agent, do NOT start new one directly.
# Let RCWatchdog detect the death and restart in Session 1.
# Then verify:
info "$POD_NAME: Verifying Session 1..."
SESSION=$(exec_on_pod "$POD_IP" "tasklist /V /FO CSV | findstr rc-agent" 2>/dev/null)
if echo "$SESSION" | grep -qi "services"; then
    fail "$POD_NAME: rc-agent in Session 0 — BLOCKING"
    exit 1
fi
```

---

### PP-02: Manual Fixes Regress Without Code Enforcement
**Impact:** ConspitLink flicker, power settings, USB suspend revert on every deploy/reboot
**Recurrences:** 5+ (R-01 x3, R-12, plus any bat drift)
**Root cause:** Fixes applied manually, not encoded in startup scripts

**The Rule:** Every manual fix MUST have a corresponding line in `start-rcagent.bat` or equivalent boot script. The bat file IS the source of truth for pod state enforcement.

**Bat file integrity check (new audit phase):**
```bash
# Phase 63: Bat File Integrity
# For each pod, hash the deployed bat and compare against canonical
CANONICAL_HASH=$(sha256sum scripts/deploy/start-rcagent.bat | cut -d' ' -f1)
for pod in $PODS; do
    DEPLOYED_HASH=$(exec_on_pod "$pod" "certutil -hashfile C:\\RacingPoint\\start-rcagent.bat SHA256" | sed -n '2p')
    if [ "$CANONICAL_HASH" != "$DEPLOYED_HASH" ]; then
        fail "Pod $pod: bat file drift detected"
    fi
done
```

**Required bat enforcement lines (must be present in canonical bat):**
```bat
:: Power enforcement
powercfg /setactive 8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c
powercfg /change standby-timeout-ac 0
:: Bloatware kill (singletons)
taskkill /F /IM ConspitLink2.0.exe 2>nul
taskkill /F /IM Variable_dump.exe 2>nul
:: Orphan cleanup
taskkill /F /IM powershell.exe 2>nul
:: Sentinel cleanup
del /Q C:\RacingPoint\MAINTENANCE_MODE 2>nul
del /Q C:\RacingPoint\GRACEFUL_RELAUNCH 2>nul
del /Q C:\RacingPoint\rcagent-restart-sentinel.txt 2>nul
```

---

### PP-03: Stale Binary Deployed (Cargo Cache)
**Impact:** Server serving 404 for endpoints that exist in code. Wrong build_id.
**Recurrences:** 3+ (R-04, R-13)
**Root cause:** `cargo build --release` reuses cached binary when only commits change (not source files)

**Structural fix — enforce in stage-release.sh:**
```bash
# MANDATORY before every cargo build:
for crate in crates/rc-agent crates/racecontrol crates/rc-common; do
    touch "$crate/build.rs"
done
# MANDATORY after build:
EXPECTED_ID=$(git rev-parse --short HEAD)
ACTUAL_ID=$(strings target/release/rc-agent.exe 2>/dev/null | grep -oP 'build_id=\K[a-f0-9]+' || echo "unknown")
if [ "$EXPECTED_ID" != "$ACTUAL_ID" ]; then
    fail "Build ID mismatch: expected $EXPECTED_ID, got $ACTUAL_ID — cargo cache stale"
    exit 1
fi
```

---

### PP-04: Bat File Drift (Binary Deployed, Bat Not Synced)
**Impact:** Stale startup procedures, missing process kills, ConspitLink multiplication
**Recurrences:** 4+ (R-01, R-07, R-12)
**Root cause:** Deploy pipeline swaps binary but bat has separate deploy path

**Structural fix:** Bat deployment MUST be atomic with binary deployment.

**In deploy-pod.sh — add mandatory bat sync:**
```bash
deploy_pod() {
    # ... existing binary deploy ...

    # MANDATORY: bat sync (never optional)
    info "$POD_NAME: Syncing bat files..."
    BAT_DL=$(exec_on_pod "$POD_IP" "curl.exe -s -o C:\\RacingPoint\\start-rcagent.bat http://${JAMES_IP}:${HTTP_PORT}/start-rcagent.bat")
    BAT_HASH=$(exec_on_pod "$POD_IP" "certutil -hashfile C:\\RacingPoint\\start-rcagent.bat SHA256" | sed -n '2p')
    LOCAL_HASH=$(sha256sum "${STAGING_DIR}/start-rcagent.bat" | cut -d' ' -f1)
    if [ "$BAT_HASH" != "$LOCAL_HASH" ]; then
        fail "$POD_NAME: Bat file hash mismatch after download"
    fi
}
```

---

### PP-05: Health Endpoints Pass While Behavior Is Broken
**Impact:** Blanking broken on all pods, config wrong, static files 404 — all undetected
**Recurrences:** 6+ (R-02, R-03, R-08, R-10, R-11, plus kiosk static)
**Root cause:** Health checks verify proxies (HTTP 200, build_id) not actual behavior

**Structural fix — extend rc-agent `/health` response:**
```json
{
    "build_id": "abc123",
    "session_id": 1,
    "maintenance_mode": false,
    "edge_process_count": 2,
    "conspitlink_count": 1,
    "bat_hash": "sha256:...",
    "config_fetch_ok": true,
    "config_fetch_age_secs": 120,
    "allowlist_entry_count": 167,
    "uptime_secs": 3600
}
```

**With these fields, automated verification catches PP-01, PP-02, PP-04, PP-06, PP-07, PP-09 — six of ten pain points become machine-detectable.**

**Domain-matched verification table:**

| Change Domain | Verification | Proxy Check (insufficient) |
|---------------|-------------|---------------------------|
| Blanking/display | `edge_process_count > 0` at `:18924/debug` | health OK |
| Game launch | Trigger test launch, verify acs.exe running | API returns 200 |
| Billing | Start/stop test session, verify DB record | build_id match |
| Serialization | Check generated INI on pod after test launch | compile success |
| Config | Read `/health` → `config_fetch_ok: true` + `allowlist_entry_count > 0` | server is running |
| Frontend | Curl `_next/static/` URL → 200 (not 404) | health page loads |

---

### PP-06: MAINTENANCE_MODE Silent Pod Killer
**Impact:** Pods dead for hours with no alert, no timeout, no auto-clear
**Recurrences:** 3+ (R-05, Pods 5/6/7 incident)
**Root cause:** Sentinel file created on 3 crashes in 10 min, never expires, no notification

**Required changes (3 layers):**

1. **Auto-expiry in rc-sentry:** Check MAINTENANCE_MODE file age every 60s. If older than 30 minutes, delete it and restart rc-agent via proven `/exec` path. Log the auto-clear.

2. **WhatsApp alert on creation:** When rc-agent writes MAINTENANCE_MODE, also POST to server `/api/v1/alerts` with pod_id and crash count. Server sends WhatsApp to Uday: "Pod X entered MAINTENANCE_MODE after 3 crashes in 10 min."

3. **Pre-deploy gate:** Before deploying to any pod, check for MAINTENANCE_MODE and clear it:
```bash
# In deploy-pod.sh, before binary swap:
MAINT=$(exec_on_pod "$POD_IP" "if exist C:\\RacingPoint\\MAINTENANCE_MODE (echo YES) else (echo NO)")
if [ "$MAINT" = "YES" ]; then
    warn "$POD_NAME: MAINTENANCE_MODE active — clearing before deploy"
    exec_on_pod "$POD_IP" "del /Q C:\\RacingPoint\\MAINTENANCE_MODE C:\\RacingPoint\\GRACEFUL_RELAUNCH C:\\RacingPoint\\rcagent-restart-sentinel.txt"
fi
```

---

### PP-07: Config Fetched Once at Boot, Never Re-Fetched
**Impact:** Empty allowlist (28K false violations/day), stale feature flags
**Recurrences:** 2+ (R-09, feature flags)
**Root cause:** Boot-time fetch with no retry if server is down

**Status of periodic re-fetch:**

| Config | Re-fetch | Status |
|--------|----------|--------|
| Process allowlist | 5-min periodic | DONE (commit `821c3031`) |
| Feature flags | 5-min periodic | DONE (BOOT-02) |
| Billing rates | Unknown | CHECK — verify |
| Camera config | Unknown | CHECK — verify |
| Kiosk settings | WS push on change | OK |

**Post-deploy verification:**
```bash
# After deploying rc-agent, verify config was fetched:
HEALTH=$(curl -s "http://${POD_IP}:8090/health")
ALLOWLIST_COUNT=$(echo "$HEALTH" | jq -r '.allowlist_entry_count // 0')
CONFIG_OK=$(echo "$HEALTH" | jq -r '.config_fetch_ok // false')
if [ "$ALLOWLIST_COUNT" -eq 0 ]; then
    fail "$POD_NAME: Allowlist empty — config fetch failed"
fi
```

---

### PP-08: Cross-Boundary Serialization Mismatch
**Impact:** AI always Semi-Pro (user selected Easy), zero AI opponents
**Recurrences:** 2+ (ai_difficulty/ai_level, ai_count/ai_cars)
**Root cause:** Kiosk sends field names that Serde silently drops

**Structural fix — build-time contract test:**
```bash
# In stage-release.sh or CI:
echo "Checking kiosk → Rust field alignment..."
# Extract fields from kiosk buildLaunchArgs
KIOSK_FIELDS=$(grep -roP '(?<=")[a-z_]+(?=":)' kiosk/src/lib/api.ts | sort -u)
# Extract fields from AcLaunchParams
RUST_FIELDS=$(grep -oP 'pub \K[a-z_]+' crates/rc-agent/src/game_launcher/ac_launcher.rs | sort -u)
# Find mismatches
ORPHANED=$(comm -23 <(echo "$KIOSK_FIELDS") <(echo "$RUST_FIELDS"))
if [ -n "$ORPHANED" ]; then
    fail "Kiosk fields with no Rust match: $ORPHANED"
fi
```

**Runtime verification:** After any kiosk change, trigger test launch on Pod 8 canary, read back `race.ini` on pod, verify AI_LEVEL matches selection.

---

### PP-09: Orphan Process Multiplication
**Impact:** 11 ConspitLink instances, 16 watchdog PowerShells, ~960MB RAM waste
**Recurrences:** 4+ (R-01, R-07)
**Root cause:** Start without kill-first, no singleton enforcement

**Standing pattern:** Every `start` MUST be preceded by `taskkill /F /IM <process>`:
```bat
:: In start-rcagent.bat — ALWAYS kill-before-start:
taskkill /F /IM ConspitLink2.0.exe 2>nul
taskkill /F /IM Variable_dump.exe 2>nul
taskkill /F /IM powershell.exe 2>nul
:: ... then start
```

**Post-deploy verification (new verify.sh gate):**
```bash
# Gate 10: Process singleton check
for proc in rc-agent.exe ConspitLink2.0.exe; do
    COUNT=$(exec_on_pod "$POD_IP" "tasklist /FI \"IMAGENAME eq $proc\" /FO CSV" | grep -c "$proc")
    if [ "$COUNT" -gt 1 ]; then
        fail "$POD_NAME: $proc has $COUNT instances (expected 1)"
    fi
done
```

---

### PP-10: Deploy Chain Self-Destructs
**Impact:** Pod offline 2+ minutes, exec handler killed mid-command
**Recurrences:** 3+ (Pod 5 v17.0, plus any taskkill+start in same chain)
**Root cause:** `taskkill /F /IM rc-agent.exe` kills the process handling the exec request

**The Rule (ENFORCED):** Never combine `taskkill` and subsequent commands for the same process in one exec chain. Use `RCAGENT_SELF_RESTART` sentinel or let RCWatchdog handle restart.

**Deploy script enforcement:**
```bash
# In deploy-pod.sh — NEVER do this:
# BAD: exec_on_pod "taskkill /F /IM rc-agent.exe & start rc-agent"
#
# GOOD: Write self-restart sentinel, agent restarts itself:
exec_on_pod "$POD_IP" "echo RESTART > C:\\RacingPoint\\RCAGENT_SELF_RESTART"
# Then verify restart via health poll
```

---

## Part 2: The Anti-Regression Checklist

**Use this checklist BEFORE marking any fix as "done".** Every item maps to a pain point.

### Pre-Fix (before writing code)
- [ ] **PP-ALL:** Checked LOGBOOK.md and git history for identical past incident (Tier 2 debug)
- [ ] **PP-02:** Identified WHERE the fix will be enforced (bat file? Rust code? Config?)
- [ ] **PP-02:** If bat file change: verified canonical bat at `scripts/deploy/start-rcagent.bat` is the edit target

### Post-Fix (after code change, before deploy)
- [ ] **PP-03:** Ran `touch crates/<crate>/build.rs` before `cargo build --release`
- [ ] **PP-03:** Verified `build_id` in compiled binary matches `git rev-parse --short HEAD`
- [ ] **PP-08:** If kiosk/frontend field changed: grepped `buildLaunchArgs()` fields against Rust struct
- [ ] **PP-10:** Deploy plan uses `RCAGENT_SELF_RESTART` or RCWatchdog (not taskkill+start)
- [ ] **PP-ALL:** `cargo test -p rc-common -p rc-agent -p racecontrol` passes

### Deploy (during deployment)
- [ ] **PP-04:** Bat file deployed alongside binary (hash verified on pod)
- [ ] **PP-06:** MAINTENANCE_MODE cleared on all target pods before deploy
- [ ] **PP-01:** After restart, `tasklist /V /FO CSV | findstr rc-agent` shows Session = Console
- [ ] **PP-03:** Post-deploy `build_id` matches expected on every target
- [ ] **PP-09:** Singleton processes have exactly 1 instance (rc-agent, ConspitLink, racecontrol)

### Verification (after deployment)
- [ ] **PP-05:** Tested the EXACT broken behavior (not just health endpoint or build_id)
- [ ] **PP-05:** For display changes: visual confirmation from human or `edge_process_count > 0`
- [ ] **PP-07:** `config_fetch_ok: true` and `allowlist_entry_count > 0` in health response
- [ ] **PP-08:** If serialization change: triggered test launch, verified generated INI on pod
- [ ] **PP-ALL:** Fix deployed to ALL targets (8 pods + server + POS + VPS where applicable)

### Post-Ship
- [ ] **PP-02:** Fix encoded in boot script (bat file or rc-agent startup code)
- [ ] **PP-ALL:** LOGBOOK.md entry written with symptom, cause, fix, verification
- [ ] **PP-ALL:** `git push` + Bono notified + standing rules updated if new rule added

---

## Part 3: Structural Weak Points (Require Code Changes)

### SW-01: POS PC Orphaned from Fleet Operations [CRITICAL]
**Finding:** `deploy-pod.sh all` iterates pods 1-8 only. POS PC (192.168.31.20) is never included in fleet deploys, fleet health checks, or audits. It drifts indefinitely.
**Fix:** Add `pos` to pod map in `deploy-pod.sh`, `verify.sh`, and `audit.sh` PODS variable.

### SW-02: start-racecontrol.bat Uses `/dev/null` (Unix Syntax on Windows) [BUG]
**Finding:** Lines 3-9 redirect to `/dev/null` instead of `nul`. Creates junk files, output not suppressed.
**Fix:** Replace all `/dev/null` with `nul` in `scripts/deploy/start-racecontrol.bat`.

### SW-03: No Automated Periodic Audit [HIGH]
**Finding:** `audit.sh` runs only when manually invoked. Fleet drift (stale binaries, MAINTENANCE_MODE, Session 0, bat drift) goes undetected until customer reports.
**Fix:** Schedule daily `audit.sh --mode quick --notify` via Task Scheduler on James PC at 06:00 IST (before venue opens).

### SW-04: pods DB Desync on Server Restart [OPEN BUG]
**Finding:** Server restart with fresh DB → `pods` table empty → kiosk shows "Waiting for pods." Health endpoints pass because in-memory fleet state is correct.
**Fix:** Auto-upsert pod row in SQLite `pods` table on every WebSocket connect.

### SW-05: No Backup/Restore Verification [5/5 MODEL CONSENSUS]
**Finding:** No automated backup of SQLite database, no restore test, no RPO/RTO defined. Data loss risk on power failure or DB corruption.
**Fix:** Weekly cron: copy DB → restore to temp → `PRAGMA integrity_check` → verify row counts → WhatsApp alert on failure.

### SW-06: MAINTENANCE_MODE Has No Timeout [SILENT KILLER]
**Finding:** Once created (3 crashes in 10 min), MAINTENANCE_MODE blocks ALL restarts permanently. No expiry, no auto-clear, no notification.
**Fix:** rc-sentry checks file age every 60s, auto-deletes after 30 min, WhatsApp alert on creation.

### SW-07: Health Endpoint Structurally Insufficient
**Finding:** 6 of 10 pain points are invisible to the current `/health` response. Health passes while blanking is broken, config is stale, processes are multiplied, and Session is wrong.
**Fix:** Extend rc-agent `/health` to include: `session_id`, `maintenance_mode`, `edge_process_count`, `conspitlink_count`, `bat_hash`, `config_fetch_ok`, `config_fetch_age_secs`, `allowlist_entry_count`.

---

## Part 4: Deploy Pipeline — Complete Sequence

### 4.1 Full Fleet Deploy (all components)

```
Step 1: BUILD
  └─ touch build.rs (all crates)
  └─ cargo test (3 crates)
  └─ cargo build --release
  └─ Verify build_id matches HEAD
  └─ Cross-boundary field check (if kiosk changed)

Step 2: STAGE
  └─ stage-release.sh
  └─ Security gate (SEC-GATE-01)
  └─ Copy binaries + bat files + manifest to deploy-staging/
  └─ Start HTTP server on :18889

Step 3: CANARY (Pod 8)
  └─ Clear MAINTENANCE_MODE
  └─ Deploy binary + bat via RCAGENT_SELF_RESTART
  └─ Wait for RCWatchdog restart
  └─ Verify: build_id, Session=Console, edge_process_count>0, singleton, config_fetch_ok
  └─ Human visual check if display change

Step 4: FLEET (Pods 1-7 + POS)
  └─ Same as canary, all pods
  └─ Parallel deploy with 4-pod concurrency cap

Step 5: SERVER (v3.0 — MMA-hardened, 12-model audit)
  └─ `bash deploy-staging/deploy-server.sh [hash]` (preferred — automates all 8 steps)
  └─ OR manual: LAN SSH → download → confirmed kill (poll 15s + port check) → atomic swap (del prev + ren) → schtasks start → verify build_id (3 attempts) → smoke test (4 endpoints) → cleanup
  └─ Auto-rollback on: start failure, build_id mismatch, smoke test failure
  └─ Verify: build_id, watchdog singleton, health serving correct endpoints, debug/activity returns 200
  └─ NEVER: run new binary while old alive (port conflict), trust schtasks exit code alone

Step 6: FRONTEND (admin, kiosk, web)
  └─ deploy-nextjs.sh for each app
  └─ Verify: curl _next/static/ URL → 200
  └─ Verify from non-server machine (POS or James browser)

Step 7: CLOUD (Bono VPS)
  └─ comms-link relay: git_pull → pm2 restart
  └─ Verify: health endpoint on racingpoint.cloud

Step 8: VERIFY
  └─ verify.sh (all gates including new gates 8-10)
  └─ Quality gate (test/run-all.sh)
  └─ E2E round-trip (exec + chain + health)
  └─ LOGBOOK + git push + Bono notified
```

### 4.2 Quick Fix Deploy (single bug fix)

```
1. Fix code or bat file
2. touch build.rs → cargo test → cargo build --release
3. Verify build_id matches HEAD
4. Deploy to Pod 8 canary
5. Test EXACT broken behavior (not just health)
6. If pass: deploy to all affected targets (pods, server, POS, VPS)
7. Anti-regression checklist (Part 2)
8. LOGBOOK entry
```

---

## Part 5: Regression Timeline Reference

15 documented regressions categorized by root cause pattern:

| ID | Date | Issue | Category | Fix Permanent? |
|----|------|-------|----------|----------------|
| R-01 | Mar 25 | ConspitLink flicker (3x same day) | Env drift + Toolchain bypass | Yes (bat enforcement) |
| R-02 | Mar 24 | Pod healer flicker (curl quotes) | Verification gap | Partial (standing rule only) |
| R-03 | Mar 26 | Session 0 blanking broken (all pods) | Architecture gap | Partial (RCWatchdog, no audit phase script) |
| R-04 | Mar 24 | Stale binary deployed (server 6 commits behind) | Toolchain bypass | No (standing rule only) |
| R-05 | Mar 23 | rc-sentry restart silent failure | Architecture gap | Yes (run_cmd_sync fix) |
| R-06 | Mar 23 | Log filename mismatch (3-day stale API) | Toolchain bypass | No (no contract test) |
| R-07 | Mar 24 | 16 orphan watchdog PowerShells | Env drift | Yes (singleton mutex) |
| R-08 | Mar 24 | TOML corruption via SSH pipe | Verification gap | Partial (standing rule, load_or_default still masks) |
| R-09 | Mar 23 | Process guard .exe suffix mismatch | Toolchain bypass | Yes (code fix) |
| R-10 | Mar 23 | Wrong monitoring targets (go2rtc, comms-link) | Knowledge gap | No (hardcoded, no service discovery) |
| R-11 | Ongoing | pods DB empty after restart | Architecture gap | No (open bug) |
| R-12 | Mar 22+ | Variable_dump.exe crashes on pedal | Env drift | UNCONFIRMED (fix in staging, deploy status unknown) |
| R-13 | Mar 23 | build.rs GIT_HASH caching | Toolchain bypass | Partial (rerun-if-changed, but touch still needed) |
| R-14 | Mar 23 | Blanking not spanning NVIDIA Surround | Architecture gap | Yes (SetWindowPos fix, but SCP-only deploy constraint is manual) |
| R-15 | Mar 23 | UTC/IST timestamp misread | Knowledge gap | No (no IST in log format) |

**Category distribution:**
- Environment drift: 4 incidents → Fix: fleet-wide bat hash verification
- Toolchain bypass: 5 incidents → Fix: CI/build pipeline integration tests
- Verification gap: 3 incidents → Fix: behavioral checks in health endpoint + audit
- Architecture gap: 4 incidents → Fix: structural code changes (pods DB, MAINTENANCE_MODE, health extension)
- Knowledge gap: 2 incidents → Fix: service discovery for monitoring, IST in logs

---

## Part 6: Automation Priority List

Ranked by (frequency x severity) / effort:

| Priority | Automation | Pain Points | Effort | Impact |
|----------|-----------|-------------|--------|--------|
| P1 | Extend `/health` with session_id, maintenance_mode, edge_count, bat_hash, config_ok | PP-01,02,04,05,06,07,09 | Medium | Eliminates 7 of 10 pain points from manual checking |
| P2 | MAINTENANCE_MODE auto-expiry (30 min) + WhatsApp alert | PP-06 | Small | Prevents silent pod death |
| P3 | POS PC in pod map (deploy + audit + verify) | PP-02,04 | Trivial | Ends POS drift |
| P4 | Daily `audit.sh --mode quick --notify` on schedule | PP-ALL | Trivial | Catches fleet drift before customers |
| P5 | `touch build.rs` enforced in stage-release.sh | PP-03 | Trivial | Prevents stale binary deploy |
| P6 | Bat hash verification in deploy-pod.sh | PP-02,04 | Small | Detects bat drift at deploy time |
| P7 | Session 1 check in deploy-pod.sh and verify.sh | PP-01 | Small | Catches Session 0 at deploy |
| P8 | Cross-boundary field check in CI | PP-08 | Small | Prevents silent serialization drops |
| P9 | pods auto-register on WS connect | PP-11 | Medium | Fixes open DB desync bug |
| P10 | Weekly DB backup + restore test | PP-ALL | Small | Prevents data loss (5/5 model consensus) |

---

## Quick Wins (< 20 lines bash each)

### QW-1: Pre-deploy MAINTENANCE_MODE check
```bash
# Add to deploy-pod.sh before binary swap
for pod in $PODS; do
    MAINT=$(curl -s "http://$pod:8091/exec" -d '{"cmd":"if exist C:\\RacingPoint\\MAINTENANCE_MODE (echo YES) else (echo NO)"}' | jq -r '.output // "UNKNOWN"')
    [ "$MAINT" = "YES" ] && { warn "Pod $pod: clearing MAINTENANCE_MODE"; curl -s "http://$pod:8091/exec" -d '{"cmd":"del /Q C:\\RacingPoint\\MAINTENANCE_MODE"}' > /dev/null; }
done
```

### QW-2: Session 1 verification post-deploy
```bash
# Add to verify.sh as Gate 8
for pod in $PODS; do
    SESS=$(curl -s "http://$pod:8091/exec" -d '{"cmd":"tasklist /V /FO CSV | findstr rc-agent"}' | jq -r '.output // ""')
    echo "$SESS" | grep -qi "services" && fail "Pod $pod: rc-agent in Session 0"
done
```

### QW-3: Enforce touch build.rs in stage-release.sh
```bash
# Add before cargo build line
for crate in crates/*/build.rs; do touch "$crate"; done
echo "Touched all build.rs files — cargo cache busted"
```

### QW-4: Post-deploy blanking behavioral check
```bash
# Add to verify.sh as Gate 9
for pod in $PODS; do
    DEBUG=$(curl -s "http://$pod:18924/debug" 2>/dev/null)
    EDGE=$(echo "$DEBUG" | jq -r '.edge_process_count // 0')
    STATE=$(echo "$DEBUG" | jq -r '.lock_screen_state // "unknown"')
    [ "$STATE" = "screen_blanked" ] && [ "$EDGE" -eq 0 ] && fail "Pod $pod: blanking broken (edge=0)"
done
```

### QW-5: Fix start-racecontrol.bat /dev/null → nul
```bash
# One-time fix
sed -i 's|/dev/null|nul|g' scripts/deploy/start-racecontrol.bat
```

---

## Appendix A: Model Contribution Summary

| Finding | Opus | Sonnet | Haiku |
|---------|:---:|:---:|:---:|
| POS PC structurally orphaned | x | | |
| start-racecontrol.bat /dev/null bug | x | | |
| Health endpoint structurally insufficient (7 missing fields) | x | | x |
| Deploy should let RCWatchdog restart (not direct start) | x | | |
| 15 regression timeline with 5 categories | | x | |
| R-12 (Variable_dump) unconfirmed deployment | | x | |
| R-08 load_or_default() still masks corruption | | x | |
| 43 standing rules: 35% automated, 32% manual | | | x |
| 5 quick wins with bash snippets | | | x |
| Deploy script quality scores (6-8/10) | | | x |
| MAINTENANCE_MODE 30-min expiry | x | x | x |
| Session 1 verification in deploy scripts | x | x | x |
| Bat file hash verification | x | x | x |

**Cross-model consensus (all 3):** MAINTENANCE_MODE timeout, Session verification, bat hash checking.
**Opus-unique:** POS orphaning, /dev/null bug, health extension architecture.
**Sonnet-unique:** Full 15-regression timeline, 5-category classification, unconfirmed deploys.
**Haiku-unique:** Standing rule automation percentages, deploy script scoring, quick wins.
