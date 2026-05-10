# PR #66 Deploy Runbook — silent-loop-death fix → Pods 1-8

**Status**: AUTHORED, AWAITING CAPTAIN AUTH TO EXECUTE
**Authored**: 2026-05-09 ~12:35 IST · james
**Target PR**: [#66](https://github.com/bono-bot/racecontrol/pull/66) · MERGED `d6c623d7` 11:10 IST
**Source main**: `02d2cfb2` (HEAD)
**Current pod state**: all 8 pods uniform on `c5f94e31-dirty` (per live `/api/v1/fleet/health` probe at 12:18 IST)
**Boundary class**: foundational pod-state-channel (per Captain V1↔V2 RCA rule §S-146)
**Auth required**: Captain explicit per-PR deploy auth — standing-autonomy verbs do NOT clear

---

## What this runbook does

Deploys the 40-line `crates/rc-agent/src/main.rs:693` patch (panic-hook + heartbeat OS thread) to Pod 8 first as canary, then to Pods 1-7 fleet-wide, with explicit behavioral verification of new artifacts (`rc-agent-heartbeat.txt` + `rc-agent-panic.log`) before fleet roll-forward.

**Smallest reversible step at every gate**. Rollback path = `ren rc-agent-prev.exe rc-agent.exe` + watchdog respawn (≤10s).

---

## Patch isolation verification

```bash
$ git log --oneline c5f94e31..d6c623d7 -- crates/rc-agent/src/
d6c623d7 fix(rc-agent): pre-tracing-init panic hook + heartbeat thread (silent-loop-death Part 4) (#66)
```

**ONLY commit affecting rc-agent in the gap = PR #66**. No other rc-agent changes. Clean isolation; no surprise side-effects from intermediate commits.

---

## Pre-flight gate (BLOCKS deploy if any fails)

| # | Check | Command (run from James .27) | Expected | Action if fail |
|---|---|---|---|---|
| 1 | Cargo build clean | `cd ~/racingpoint/racecontrol && export PATH="$PATH:/c/Users/bono/.cargo/bin" && cargo build --release --bin rc-agent 2>&1 \| tail -5` | `Compiling ... Finished release` + zero errors | Diagnose; do NOT proceed |
| 2 | Cargo test clean (regression) | `cargo test -p rc-agent-crate --bin rc-agent 2>&1 \| grep "test result:"` | `test result: ok. 858 passed; 0 failed` | Diagnose; do NOT proceed |
| 3 | Binary size sane | `ls -la target/release/rc-agent.exe` | size in 25-50MB range (similar to current `c5f94e31-dirty` deployed binary) | Diagnose; do NOT proceed |
| 4 | Hash matches PR #66 merge commit | `git rev-parse --short HEAD` | `02d2cfb2` (LOGBOOK row commit; PR #66 squash is `d6c623d7` one before) — verify on `main` branch | Switch to main; pull; retry |
| 5 | Stage binary with hash name | `cp target/release/rc-agent.exe ../deploy-staging/rc-agent-d6c623d7.exe` | file exists at deploy-staging | Create dir if needed |
| 6 | SHA256 of staged binary | `sha256sum ../deploy-staging/rc-agent-d6c623d7.exe` | record hash for post-deploy verification | n/a (informational) |
| 7 | Start staging HTTP server on James .27:18889 | `cd ../deploy-staging && python -m http.server 18889 --directory . &` | server listening; `curl -s -I http://192.168.31.27:18889/rc-agent-d6c623d7.exe` returns 200 | Kill any conflicting process on :18889 |
| 8 | Verify staged binary is downloadable | `curl -s -I http://192.168.31.27:18889/rc-agent-d6c623d7.exe \| head -3` | `HTTP/1.0 200 OK` + Content-Length matches local file size | Restart HTTP server; verify path |
| 9 | Pod 8 reachability + Pod 8 active billing check | `curl -s http://192.168.31.91:8091/health` (rc-sentry :8091 — survives rc-agent death) AND `curl -s http://192.168.31.23:8080/api/v1/fleet/health \| python3 -c "import json,sys; d=json.load(sys.stdin); pod=[p for p in d if p.get('pod_number')==8][0]; print('pod_8 active_session_id=', pod.get('active_session_id'))"` | rc-sentry health 200 OK + Pod 8 active_session_id=null OR known-test-only | If Pod 8 has live customer session → DEFER deploy; pick different canary or wait |
| 10 | rc-sentry service key parity (X-Service-Key) | (verify with prior pod-deploy logs OR ping with key from `/c/RacingPoint/racecontrol.toml` on Server .23) | key matches Pod 8's `rc-sentry.toml` X-Service-Key | If mismatch → fix key parity FIRST per memory `feedback_canonical_deploy_path_vs_symmetry_projected_user_candidate_n1.md` |
| 11 | Captain Cognitive Load: Pod 8 venue impact | n/a — informational | Pod 8 swap window ~30-60s during which Pod 8 is briefly unavailable to customer | acceptable per OTA pipeline standing rule (rolling deploy with rollback) |

**IF ANY FAIL: STOP. Do NOT proceed past this gate. Report to Captain.**

---

## Pod 8 canary — atomic-swap sequence

**Goal**: Deploy PR #66 binary to Pod 8 only. Verify behavioral artifacts emerge. Hold for ≥5 min observation before fleet roll-forward.

**Canonical path**: rc-sentry `/exec` on **Pod 8 :8091** (NOT rc-agent :8090; rc-sentry is separate process that survives rc-agent kill).

### Step C1: Stage binary on Pod 8

```bash
# From James .27, via rc-sentry on Pod 8 :8091
POD8_IP="192.168.31.91"
JAMES_IP="192.168.31.27"
SERVICE_KEY="<read from server racecontrol.toml [rcagent] service_key>"

curl -s --max-time 60 -X POST "http://$POD8_IP:8091/exec" \
  -H "Content-Type: application/json" \
  -H "X-Service-Key: $SERVICE_KEY" \
  -d '{"cmd":"curl.exe -s -o C:\\RacingPoint\\rc-agent-d6c623d7.exe http://192.168.31.27:18889/rc-agent-d6c623d7.exe","timeout":60}'

# Verify staged
curl -s --max-time 10 -X POST "http://$POD8_IP:8091/exec" \
  -H "Content-Type: application/json" \
  -H "X-Service-Key: $SERVICE_KEY" \
  -d '{"cmd":"powershell -Command \"(Get-Item C:\\RacingPoint\\rc-agent-d6c623d7.exe).Length\"","timeout":10}'
# Expected: byte count matches James .27 local file size from pre-flight #6
```

### Step C2: Sync updated start-rcagent.bat (for next-boot only, not active swap)

```bash
# SCP requires SSH trust to Pod 8 ADMIN account; if not available, skip — bat-only takes effect at next reboot
scp scripts/deploy/start-rcagent.bat ADMIN@$POD8_IP:C:/RacingPoint/start-rcagent.bat
# If SSH fails: SKIP (rc-agent restart via watchdog uses CreateProcessAsUser, NOT the bat per CLAUDE.md "Remote deploy sequence")
```

### Step C3: Atomic swap — single /exec chain (rc-sentry :8091)

Per CLAUDE.md "Remote deploy sequence (rc-agent)" canonical sequence:

```bash
# CRITICAL: chain MUST complete ren steps within RCWatchdog's ~5-10s polling window.
# RCWatchdog spawns rc-agent.exe DIRECTLY (per rc-watchdog/src/session.rs:126).
# If we taskkill but don't ren in time → watchdog restarts OLD binary.
curl -s --max-time 30 -X POST "http://$POD8_IP:8091/exec" \
  -H "Content-Type: application/json" \
  -H "X-Service-Key: $SERVICE_KEY" \
  -d '{"cmd":"taskkill /F /IM rc-agent.exe & del /Q C:\\RacingPoint\\rc-agent-prev.exe & ren C:\\RacingPoint\\rc-agent.exe rc-agent-prev.exe & ren C:\\RacingPoint\\rc-agent-d6c623d7.exe rc-agent.exe","timeout":30}'
# rc-sentry returns once chain spawned. Wait ≤15s for RCWatchdog to spawn new rc-agent.exe.
```

**Race-condition note**: this works because rc-sentry on :8091 is a SEPARATE process from rc-agent on :8090. Killing rc-agent does NOT kill rc-sentry, so the /exec chain completes.

### Step C4: Wait for RCWatchdog to spawn new rc-agent (15s)

```bash
sleep 15
```

### Step C5: build_id verification

```bash
curl -s --max-time 5 "http://$POD8_IP:8090/health" | python3 -c "import json,sys; d=json.load(sys.stdin); print('Pod 8 build_id:', d.get('build_id'))"
# Expected: d6c623d7 (NOT c5f94e31-dirty)
# If still c5f94e31-dirty → swap failed; investigate (likely watchdog respawned old binary or ren failed)
```

### Step C6: Behavioral verification — heartbeat artifact (PRIMARY)

This is the H3 EVIDENCE check for the actual fix behavior, not just the binary running.

```bash
# T+0s: probe heartbeat file existence
curl -s --max-time 10 -X POST "http://$POD8_IP:8091/exec" \
  -H "Content-Type: application/json" \
  -H "X-Service-Key: $SERVICE_KEY" \
  -d '{"cmd":"powershell -Command \"if (Test-Path C:\\RacingPoint\\rc-agent-heartbeat.txt) { (Get-Item C:\\RacingPoint\\rc-agent-heartbeat.txt).LastWriteTimeUtc.ToString(\\\"o\\\") } else { \\\"MISSING\\\" }\"","timeout":10}'
# Expected output: ISO-8601 UTC timestamp from now-30s onwards (heartbeat thread writes every 30s after rc-agent boot)
# If MISSING after T+45s → fix did not activate; rollback

# T+45s: probe again to verify mtime advanced
sleep 45
curl -s --max-time 10 -X POST "http://$POD8_IP:8091/exec" \
  -H "Content-Type: application/json" \
  -H "X-Service-Key: $SERVICE_KEY" \
  -d '{"cmd":"powershell -Command \"(Get-Item C:\\RacingPoint\\rc-agent-heartbeat.txt).LastWriteTimeUtc.ToString(\\\"o\\\")\"","timeout":10}'
# Expected: timestamp ADVANCED from previous read (proves OS thread is alive + writing)
```

### Step C7: Behavioral verification — panic.log artifact (NEGATIVE)

```bash
curl -s --max-time 10 -X POST "http://$POD8_IP:8091/exec" \
  -H "Content-Type: application/json" \
  -H "X-Service-Key: $SERVICE_KEY" \
  -d '{"cmd":"powershell -Command \"if (Test-Path C:\\RacingPoint\\rc-agent-panic.log) { (Get-Item C:\\RacingPoint\\rc-agent-panic.log).Length } else { 0 }\"","timeout":10}'
# Expected: 0 (file missing or empty — no panics on healthy boot)
# Non-zero size → rc-agent panicked during boot; READ the file content via /exec, diagnose, rollback
```

### Step C8: Cross-check via Server .23 fleet view

```bash
# Verify Pod 8 still ws_connected from Server .23's perspective
curl -s --max-time 5 "http://192.168.31.23:8080/api/v1/fleet/health" | python3 -c "
import json, sys
d = json.load(sys.stdin)
pods = d if isinstance(d, list) else d.get('pods', [])
for p in pods:
    if p.get('pod_number') == 8:
        print(f'pod_8: ws={p.get(\"ws_connected\")} http={p.get(\"http_reachable\")} build={p.get(\"build_id\")} uptime={p.get(\"uptime_secs\")}s')
        break
"
# Expected: ws=True http=True build=d6c623d7 uptime <60s (fresh boot)
```

### Step C9: Hold for ≥5 min observation

```bash
# During hold window: monitor Pod 8 for crash-loop, panic, or WS-flap
# Re-probe heartbeat mtime every 60s — should always be <60s old
# Re-probe Server .23 fleet view — Pod 8 should stay ws=True
```

**Pod 8 canary PASS criteria** (ALL must hold for ≥5 min):
- ✅ Pod 8 build_id = `d6c623d7` (per `/health` and Server .23 fleet view)
- ✅ Pod 8 `rc-agent-heartbeat.txt` exists + mtime advances every 30s + always <60s old
- ✅ Pod 8 `rc-agent-panic.log` empty (size 0 or file missing)
- ✅ Pod 8 ws_connected = True sustained (no flap)
- ✅ Pod 8 no crash-loop (no PID rotation; uptime monotonic)
- ✅ No new INFO/WARN log entries indicating regression in Pod 8's `rc-agent-.*.jsonl` log

**If ANY FAIL**: rollback Pod 8 (Step R1 below); STOP; report to Captain.

---

## Rollback (R1) — Pod 8 only

```bash
# Single rc-sentry /exec to swap prev binary back
curl -s --max-time 30 -X POST "http://$POD8_IP:8091/exec" \
  -H "Content-Type: application/json" \
  -H "X-Service-Key: $SERVICE_KEY" \
  -d '{"cmd":"taskkill /F /IM rc-agent.exe & del /Q C:\\RacingPoint\\rc-agent-d6c623d7.exe & ren C:\\RacingPoint\\rc-agent.exe rc-agent-failed-d6c623d7.exe & ren C:\\RacingPoint\\rc-agent-prev.exe rc-agent.exe","timeout":30}'

sleep 15

# Verify rollback
curl -s --max-time 5 "http://$POD8_IP:8090/health" | python3 -c "import json,sys; d=json.load(sys.stdin); print('Pod 8 build_id post-rollback:', d.get('build_id'))"
# Expected: c5f94e31-dirty (back to pre-deploy state)
```

**Rollback PASS**: Pod 8 build_id back to `c5f94e31-dirty` AND ws_connected=True from Server .23 fleet view AND uptime fresh (<60s).

---

## Fleet roll-forward — Pods 1-7

**Gate**: Pod 8 canary PASS for ≥5 min sustained AND Captain explicit auth to roll forward.

**Strategy**: Sequential, NOT parallel — one pod at a time, ≥30s gap between pods, build_id verify after each.

### Pod IPs (per CLAUDE.md Network Map)

| Pod | LAN IP |
|---|---|
| pod_1 | 192.168.31.89 |
| pod_2 | 192.168.31.33 |
| pod_3 | 192.168.31.28 |
| pod_4 | 192.168.31.88 |
| pod_5 | 192.168.31.86 |
| pod_6 | 192.168.31.87 |
| pod_7 | 192.168.31.38 |
| pod_8 | 192.168.31.91 (already canary'd) |

### Per-pod sequence (repeat for pod_1 → pod_7)

For each `POD_IP` in the order above:

1. **Pre-pod-flight**: probe `/api/v1/fleet/health` for that pod's `active_session_id` — DEFER if non-null
2. **Stage**: same as Step C1 above
3. **SCP bat**: same as Step C2 (best-effort; SKIP if SSH unavailable)
4. **Atomic swap**: same as Step C3
5. **Wait 15s**: same as Step C4
6. **build_id verify**: same as Step C5 — must show `d6c623d7`
7. **Heartbeat verify (T+45s)**: same as Step C6 — mtime must advance
8. **Panic.log empty verify**: same as Step C7
9. **30s gap before next pod**: protects against simultaneous fleet-wide failure if anything is wrong with the binary

### Stop conditions during fleet roll-forward

- ANY pod fails build_id verify → STOP fleet roll-forward; rollback that pod (R1); investigate
- ANY pod's heartbeat fails to advance → STOP; rollback (R1); investigate
- ANY pod's `panic.log` non-empty → STOP; READ panic.log content; rollback (R1); diagnose
- Server .23 `silent_reconnect_suspected` flag fires for ANY pod → STOP; investigate (the very thing this fix is supposed to prevent should NOT fire post-deploy)
- 2+ pod simultaneous WS flap → STOP (could indicate a fleet-wide issue introduced by the new binary)

---

## Post-fleet-deploy (gates 6-10 closure)

### Step P1: Final fleet build_id sweep

```bash
curl -s --max-time 5 "http://192.168.31.23:8080/api/v1/fleet/health" | python3 -c "
import json, sys
d = json.load(sys.stdin)
pods = d if isinstance(d, list) else d.get('pods', [])
expected = 'd6c623d7'
for p in pods[:8]:
    n = p.get('pod_number')
    bid = (p.get('build_id') or '')[:8]
    ok = '✓' if bid == expected[:8] else '✗ STILL OLD'
    print(f'pod_{n}: build={bid} {ok}')
"
# Expected: all 8 pods show d6c623d7 ✓
```

### Step P2: Heartbeat artifact fleet sweep

```bash
# For each pod, confirm heartbeat file exists + mtime <60s old
# Failure of any pod → mark for individual investigation; do NOT roll back fleet
```

### Step P3: LOGBOOK row append

```bash
cd ~/racingpoint/racecontrol
# Append row of form:
# | YYYY-MM-DD HH:MM IST | James | deploy-rc-agent-d6c623d7 | **deploy(rc-agent): PR #66 silent-loop-death fix DEPLOYED to Pods 1-8 — silent-loop-death class observability resilience now active fleet-wide. Pod 8 canary first; ≥5min hold; sequential roll-forward 1-7. Behavioral verification: heartbeat.txt + panic.log artifacts confirmed on all 8 pods.** ... |
git add LOGBOOK.md && git commit -m "logbook: deploy PR #66 rc-agent silent-loop-death fix → Pods 1-8" && git push
```

### Step P4: Memory state advance — IN-FLIGHT/MERGED-PENDING-DEPLOY → SHIPPED

Update both:
- `~/.claude/projects/C--Users-bono/memory/project_silent_loop_death_v1v2_rca_20260509.md` — gate status table 7+8+9+10 → ✅
- `~/.claude/projects/C--Users-bono/memory/project_pod1_silent_loop_death_rca_20260509.md` — improvement #1 status: MERGED-PENDING-DEPLOY → SHIPPED `<deploy-timestamp-IST>`

### Step P5: §S-N ledger entry to V2-MASTER-STATE.md

Add §S-153 (or next available; reconcile with bono's §S-N before writing) capturing the deploy event + behavioral verification evidence. This advances PR #66 from MERGED-PENDING-DEPLOY (recorded in §S-150) to true SHIPPED.

### Step P6: bilateral msg ship to bono

```bash
RESPONSIVE_TO_INBOUND=1 COMMS_SENDER=james COMMS_API_URL=http://srv1422716.hstgr.cloud:3100 COMMS_API_KEY=rp-gateway-2026-secure-key node send-message.js --to bono --type update --subject "PR #66 silent-loop-death fix DEPLOYED + VERIFIED → Pods 1-8 fleet-wide" --body "..."
```

### Step P7: Now §S-153 server-side reader phase becomes actionable

The follow-up phase (server-side `fleet_health_api.rs` heartbeat-mtime reader) was kaizen-pinned to wait for PR #66 deploy lands. Post-P1 success, §S-153 can be authored. Needs own 5-section RCA + MMA Step 1 + per-PR Captain auth per §S-146 (foundational pod-state-channel).

---

## Stop conditions (ANY fires → halt + report)

- Pre-flight gate fails (cargo, hash, staging, key parity, Pod 8 active billing)
- Pod 8 canary build_id mismatch
- Pod 8 heartbeat artifact missing or mtime not advancing
- Pod 8 panic.log non-empty
- Pod 8 ws_connected drops post-deploy
- Pod 8 crash-loop (PID rotation)
- 2+ pod simultaneous failure during roll-forward
- Server .23 `silent_reconnect_suspected` flag fires for ANY deployed pod (regression of the very class this fix prevents)
- Captain "stop" verb at any point

---

## NOT covered by this runbook (separate scope)

- racecontrol.exe rebuild on Server .23 — NOT NEEDED for PR #66 (silent-loop-death is rc-agent-only; Server .23 at `c43459c8` is fine)
- §S-153 server-side fleet_health reader — separate phase; requires its own runbook + RCA + auth
- DWM hang Failure 3 root cause — separate workstream
- Pod 7 WS flap actual cause — diagnostic-only follow-up; not deploy-related
- Cloud parity for rc-agent — N/A; rc-agent is venue-only per CLAUDE.md service map

---

## Auth required to execute

Per Captain V1↔V2 RCA rule §S-146: foundational pod-state-channel boundary requires per-PR Captain merge auth AND deploy auth. Standing-autonomy verbs do NOT clear.

Captain has already cleared:
- ✅ PR-open auth ("Open PR for Captain review (Captain authorize)" 10:34 IST)
- ✅ PR-merge auth ("merge" 11:09 IST)

Captain has NOT yet cleared:
- ⏳ DEPLOY auth (this runbook step). Recommended granularity: separate auth per phase:
  - Phase 1: "deploy to Pod 8 canary" → executes Pre-flight + Steps C1-C9 + STOPS at hold
  - Phase 2: "deploy fleet-wide" (after Pod 8 canary clean ≥5 min) → executes per-pod sequence for Pods 1-7
  - Phase 3: "post-deploy closure" → executes Steps P1-P7

---

## Cross-references

- §S-150 ledger row at `comms-link/V2-MASTER-STATE.md` (silent-loop-death PR #66 MERGED — FIRST end-to-end §S-146 application)
- §S-152 bono-side absorption (sibling-anchor + 8/8 doctrine criteria recorded)
- `~/.claude/projects/C--Users-bono/memory/project_silent_loop_death_v1v2_rca_20260509.md` — formal 5-section RCA + MMA receipts
- `~/.claude/projects/C--Users-bono/memory/session_handoff_20260509_silent_loop_death_e2e_pipeline_NEXT_SESSION_PICKUP.md` — full session pickup doc
- `feedback_v1_dependent_v2_root_cause_before_proceeding.md` — Captain V1↔V2 RCA rule (BILATERAL)
- CLAUDE.md "Remote deploy sequence (rc-agent)" — canonical 7-step swap sequence
- CLAUDE.md "Crash loop = reboot first, investigate second" — recovery doctrine
- CLAUDE.md "Test before upload = ... deploy to Pod 8 first" — canary doctrine
- CLAUDE.md "OTA pipeline ... rollback window: previous binary preserved for 72 hours minimum" — rollback doctrine

— james / 2026-05-09 ~12:35 IST · Authored as DEPLOY-EXECUTE proposal · Awaits Captain "deploy to Pod 8 canary" verb to execute Phase 1
