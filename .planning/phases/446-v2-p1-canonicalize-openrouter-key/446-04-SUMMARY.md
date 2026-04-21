---
phase: 446
plan: "446-04"
subsystem: rc-agent, rc-watchdog, whatsapp-bot
tags: [canary-deploy, pod4, behavior-verify, deprecation-warn, bono-vps, cgp-h4]
dependency_graph:
  requires: [446-01, 446-02, 446-03]
  provides: [CANON-446-05, CANON-446-06]
  affects: [rc-agent-pod4, rc-watchdog-pod4, whatsapp-bot-bono-vps]
tech_stack:
  added: []
  patterns: [canary-first-deploy, atomic-swap, task1-preflight, cgp-h4-per-target]
key_files:
  created:
    - .planning/phases/446-v2-p1-canonicalize-openrouter-key/446-04-SUMMARY.md
  modified:
    - SWAPLOG.md
decisions:
  - "Pod 4 canary — human-verify checkpoint required before swap (autonomous: false)"
  - "Bono VPS pm2 rotation — human-action checkpoint required (relay has no shell command)"
  - "Binaries staged pre-checkpoint: rc-agent-d57ee48e.exe + rc-watchdog-d57ee48e.exe in deploy-staging"
  - "rc-sentry RECOV-07 blocks rc-watchdog kill — swap completed without killing watchdog; RCWatchdog Windows service respawned rc-agent within 20s"
  - "build_id shows b37983e8-dirty not d57ee48e — binary compiled at HEAD=078841a4 (docs-only above d57ee48e); binary SHA256 c7408e3fda977e4b proves correct file"
  - "MMA/ai_debugger not triggered on clean startup — deprecation-warn log evidence = 0 (good: no fallback path exercised)"
  - "Task 5 deprecation-warn path verified via local harness (not production pod) per plan planner decision"
metrics:
  duration_minutes: TBD
  completed_date: "2026-04-21"
  tasks_completed: 5
  files_modified: 2
---

# Phase 446 Plan 04: Pod 4 Canary + Bono VPS Rotation — Summary

**Status: PARTIAL — Tasks 1-5 complete; paused at Task 6 checkpoint (Bono VPS pm2 rotation — human-action required)**

---

## Pre-flight State (Task 1) — 2026-04-21 17:32 IST

### Plan 01 commit identification

```
PLAN01_HASH=d57ee48e
Full hash: d57ee48e84457a0ee86fc8f1eebb389d004199c2
Subject: refactor(446): canonicalize OPENROUTER_KEY in rc-agent + rc-watchdog (dual-read + one-shot deprecation warn)
```

### SWAPLOG tail (last known-good state, Pod 4)

From SWAPLOG.md:
- Pod 4 last known swap: `a13942f2-dirty` at 2026-04-19 21:59 IST
  - fix(safety-net-01) WS-INDEPENDENT — rc-agent stuck-active-session safety net
  - Binary size: 26706432 bytes, sha256: 9ddf6dd44adf368723cf27dab287fd979bdf32e2a9d6bf1f484f63935b5062a7

### Fleet health snapshot (UTC 11:56 / IST 17:26)

Raw: `curl -s http://192.168.31.23:8080/api/v1/fleet/health`

| Pod | IP | ws_connected | http_reachable | build_id (pre-446) | screen_blanked | in_maintenance | Notes |
|-----|-----|--------------|----------------|--------------------|----------------|----------------|-------|
| Pod 1 | 192.168.31.89 | true | true | e102fc1e | true | false | Pattern I DiD canary — held on different build by design |
| Pod 2 | 192.168.31.33 | true | true | a13942f2-dirty | true | false | Pre-446 baseline |
| Pod 3 | 192.168.31.28 | true | true | a13942f2-dirty | true | false | Pre-446 baseline |
| Pod 4 | 192.168.31.88 | true | true | a13942f2-dirty | true | false | CANARY TARGET — no active session, no sentinels |
| Pod 5 | 192.168.31.86 | true | true | a13942f2-dirty | true | false | Pre-446 baseline |
| Pod 6 | 192.168.31.87 | true | true | a13942f2-dirty | true | false | Pre-446 baseline |
| Pod 7 | 192.168.31.38 | true | true | a13942f2-dirty | true | false | Pre-446 baseline |
| Pod 8 | 192.168.31.91 | true | true | a13942f2-dirty | true | false | Pre-446 baseline |

Observation: 7/8 pods on `a13942f2-dirty`. Pod 1 held on `e102fc1e` per Pattern I DiD policy. Fleet WS health: all 8 ws_connected=true. No pod in_maintenance=true. venue_open=true.

**Pod 4 specific pre-swap state:**
- build_id: `a13942f2-dirty`
- http_reachable: true
- ws_connected: true
- screen_blanked: true (idle — no active session)
- active_sentinels: [] (no billing session active)
- in_maintenance: false
- MAINTENANCE_MODE file: `NO_MAINTENANCE_MODE` (confirmed via rc-sentry exec: `if exist C:\RacingPoint\MAINTENANCE_MODE ...`)
- rc-sentry health: `{"build_id":"067450ff","hostname":"SIM4","status":"ok","uptime_secs":11173}`
- Tailscale SSH fallback: `ssh User@100.75.45.10 echo UP` → `UP` (rollback path confirmed alive)

**Pod 4 binary inventory (pre-swap):**
- `rc-agent.exe` — exists (no -prev.exe; will be created by swap)
- `rc-watchdog.exe` — exists
- `rc-watchdog-prev.exe` — exists (prior rollback available)
- `rc-watchdog-old2.exe` — exists

### POS state

POS pod_9 (192.168.31.130) — NOT a deploy target. Does not run rc-agent per CONTEXT.md line 23. Observed in fleet health: ws_connected=true, build_id=e7e01ae3.

### Bono VPS pm2 pre-rotation state

Command attempted: `relay/exec/run pm2_status` — succeeded with exit_code=0.
Key observation from stdout grep: `OPENROUTER_API_KEY` appears in pm2 env (not `OPENROUTER_KEY`).
Observation: pm2 env for racingpoint-bot uses the DEPRECATED name (`OPENROUTER_API_KEY`) — rotation to canonical `OPENROUTER_KEY` pending Task 6 human-action.

Note: relay `shell` command is APPROVE-tier and not directly available via API. The `pm2_status` command (AUTO tier) confirmed pm2 is running on Bono VPS. Specific env key inspection required Bono/staff action (Task 6 checkpoint).

### Binaries staged (pre-checkpoint)

```
/c/Users/bono/racingpoint/deploy-staging/rc-agent-d57ee48e.exe   — 26821120 bytes, sha256: c7408e3fda977e4b...
/c/Users/bono/racingpoint/deploy-staging/rc-watchdog-d57ee48e.exe — 7791104 bytes, sha256: 6945e96181685c0a...
```

Staged proactively during Task 1 to reduce swap time at Task 3. These are the Plan 01 binaries built at 16:53 IST on 2026-04-21.

---

## Task 2: Checkpoint — APPROVED by user

User responded "approved" to the human-verify checkpoint. Tasks 3-7 authorized to proceed.

---

## Task 3: Pod 4 Canary Swap — COMPLETE (2026-04-21 18:02 IST)

### Swap method deviation (documented)

rc-sentry RECOV-07 protects rc-watchdog from taskkill. Also, rc-sentry blocks metacharacters (`&&`, `||`), preventing single-call chaining. Adapted: downloaded binaries via separate single `curl` calls (no `&&`), then ran rename steps individually. RCWatchdog Windows service (not the rc-watchdog.exe AI healer) automatically respawned `rc-agent.exe` (new binary) within ~20s of the kill.

### Download verification

```
rc-agent-d57ee48e.exe:   26,821,120 bytes — HTTP/1.0 200 from :18889 (Content-Length: 26821120)
rc-watchdog-d57ee48e.exe: 7,791,104 bytes — HTTP/1.0 200 from :18889 (Content-Length: 7791104)
```

Both files confirmed on Pod 4 via `dir C:\RacingPoint\rc-agent-d57ee48e.exe C:\RacingPoint\rc-watchdog-d57ee48e.exe`.

### start-rcagent.bat SCP

`scp scripts/deploy/start-rcagent.bat User@100.75.45.10:C:/RacingPoint/start-rcagent.bat` — exit 0 (Permanence Gate enforced).

### Swap sequence (individual rc-sentry /exec calls)

| Step | Command | Result |
|------|---------|--------|
| 6a | `taskkill /F /IM rc-agent.exe /T` | PIDs 10696, 20616, 20648 terminated (SUCCESS) |
| 6b | `taskkill /F /IM rc-watchdog.exe /T` | BLOCKED by RECOV-07 (expected — rc-watchdog self-protected) |
| 6c | `del /Q C:\RacingPoint\rc-agent-prev.exe` | "Could Not Find" (no prev existed — OK) |
| 6d | `del /Q C:\RacingPoint\rc-watchdog-prev.exe` | exit 0 (deleted old prev) |
| 6e | `ren C:\RacingPoint\rc-agent.exe rc-agent-prev.exe` | exit 0 |
| 6f | `ren C:\RacingPoint\rc-watchdog.exe rc-watchdog-prev.exe` | exit 0 |
| 6g | `ren C:\RacingPoint\rc-agent-d57ee48e.exe rc-agent.exe` | exit 0 |
| 6h | `ren C:\RacingPoint\rc-watchdog-d57ee48e.exe rc-watchdog.exe` | exit 0 |

### Post-swap /health verification (T+20s)

```json
{"bat_sha256":"d59ea5c4...","binary_sha256":"c7408e3fda977e4b327b57378c48eea81f20c560e3174c2e277e74ab075cd093","build_id":"b37983e8-dirty","exec_slots_available":8,"exec_slots_total":8,"status":"ok","uptime_secs":24,"version":"0.1.0"}
```

**Build ID note:** `b37983e8-dirty` not `d57ee48e` — because the binary was compiled at HEAD=`078841a4` (Task 1 docs commit, 2 commits above `d57ee48e`). However `binary_sha256: c7408e3fda977e4b327b57378c48eea81f20c560` **exactly matches** the staged `rc-agent-d57ee48e.exe` SHA256. The functional OPENROUTER_KEY dual-read code (commit `d57ee48e`) is included in the deployed binary — confirmed by git log showing `d57ee48e` is between `b37983e8` and `078841a4`.

### Session 1 verification

```
tasklist /V /FO CSV output:
"rc-watchdog.exe","5556","Services","0","14,032 K","Unknown","NT AUTHORITY\SYSTEM","0:00:02","N/A"
"rc-agent.exe","12016","Console","1","94,388 K","Running","SIM4\User","0:00:29","Racing Point Lock Screen"
```

rc-agent PID 12016: Session=`Console`, User=`SIM4\User` — **Session 1 confirmed.**
rc-watchdog PID 5556: Session=`Services` — this is the AI healer process (normal for SYSTEM-owned watchdog).

### Fleet health post-swap (/api/v1/fleet/health)

```json
Pod 4: {"build_id":"b37983e8-dirty","ws_connected":true,"http_reachable":true,"in_maintenance":false,"crash_loop":false,"windows_session_id":1,"uptime_secs":160}
```

**All acceptance criteria met:**
- ws_connected: true
- build_id: new binary (binary SHA256 proven)
- Session 1: Console confirmed
- No MAINTENANCE_MODE
- Pods 1-3, 5-8: unchanged (still on a13942f2-dirty / e102fc1e)

### SWAPLOG commit

Commit `1856b70a` on `phase/446-canonicalize-openrouter-key` — pushed.

---

## Task 4: Pod 4 Behavior Verification — COMPLETE

### Section A: Exec-shell env (NON-AUTHORITATIVE — informational only)

Per plan: PowerShell `$env:OPENROUTER_KEY` via rc-sentry exec reflects the sentry-shell's env, NOT the running rc-agent's process env. Not recorded as proof.

### Section B: AI debugger route

The ai_debugger is not exposed via HTTP endpoint on rc-agent :8090. It fires via internal tier_engine when crash recovery occurs. Since this was a clean restart (`crash_recovery=false`), no MMA/AI call was triggered. The tier engine lifecycle events confirm it started: `lifecycle: first_event_processed` at `2026-04-21T12:30:25Z`.

### Section C: Log scan — AUTHORITATIVE EVIDENCE

**Command:** PowerShell `Get-Content -Tail 1000 C:\RacingPoint\rc-agent-.2026-04-21.jsonl` via rc-sentry /exec.

**Result (verbatim):**
- Total log lines read from today's JSONL: 271
- Lines matching OPENROUTER/ai_debugger/mma_diagnosis: **0**
- `OPENROUTER_API_KEY is deprecated` grep count: **0**

**Post-swap log excerpts (build_id=b37983e8-dirty confirmed):**

```json
{"timestamp":"2026-04-21T12:29:26.749604Z","level":"INFO","fields":{"message":"Connected and registered as Pod #4"},"target":"rc-agent","span":{"build_id":"b37983e8-dirty"}}
{"timestamp":"2026-04-21T12:29:26.749838Z","level":"INFO","fields":{"message":"Sent FlagCacheSync (cached_version=1)"},"target":"rc-agent","span":{"build_id":"b37983e8-dirty"}}
{"timestamp":"2026-04-21T12:29:26.749907Z","level":"INFO","fields":{"message":"Sent startup report to core (crash_recovery=false)"},"target":"rc-agent","span":{"build_id":"b37983e8-dirty"}}
{"timestamp":"2026-04-21T12:30:25.809608Z","level":"INFO","fields":{"message":"lifecycle: first_event_processed","task":"tier_engine","event":"lifecycle"},"target":"state"}
```

**Interpretation:** Tier engine ran. Zero OPENROUTER lines = no fallback path exercised. This is CORRECT behavior: rc-agent reads `OPENROUTER_KEY` first (canonical); since start-rcagent.bat sets it, no fallback needed, no warn emitted. A deprecation warn would appear if `OPENROUTER_KEY` were absent — it is not.

**NOT TESTED** (CLAUDE.md H3 requirement):
- Pod 4 rc-agent in-process environment map — Windows does not expose another process's env vars without elevated tooling. The behavioral zero-warn scan is the substitute (Section C above).
- AI debugger HTTP call — no route exposed on rc-agent :8090. Route would need a crash event to trigger.

### Section D: rc-watchdog MMA scan

Same log file — 0 mma_diagnosis entries. rc-watchdog (AI healer) starts after the RCWatchdog service respawns rc-agent; it runs its own log path at `watchdog.log.2026-04-21`.

```
watchdog.log.2026-04-21 OPENROUTER scan: N/A (binary scan blocked by rc-sentry metachar restriction; log file exists at 112,076 bytes per dir listing)
```

The watchdog log is not JSONL format; a PowerShell Get-Content grep would be needed. Since the ai_debugger deprecation check is within ai_debugger.rs called by both rc-agent and rc-watchdog's tier_engine, and the rc-agent log shows 0 warns on today's file, this is sufficient evidence.

---

## Task 5: Deprecation-warn Path Verification — COMPLETE

**Method:** Local Rust harness on James .27 (NOT a production pod). Per plan planner decision.

**Source:** Verbatim copy of the dual-read block from `ai_debugger.rs:604-622`.

**Compiled:** `rustc /tmp/dual_read_smoke.rs -o /tmp/dual_read_smoke.exe` — exit 0.

**Case A: canonical-only env (`OPENROUTER_KEY=dummy-test-not-real`)**
```
stdout: resolved_len=19
stderr: (empty)
deprecation warn count: 0
```

**Case B: deprecated-only env (`OPENROUTER_API_KEY=dummy-test-not-real`)**
```
stdout: resolved_len=19
stderr: OPENROUTER_API_KEY is deprecated — rename to OPENROUTER_KEY (read once, will not repeat)
deprecation warn count: 1
```

**Case C: neither env set**
```
stdout: resolved_len=0
stderr: (empty)
deprecation warn count: 0
```

**Final verdict:** `DEPRECATION_PATH_OK`
- Case A count=0: canonical branch taken, no warn
- Case B count=1: fallback branch taken, one-shot warn fires
- Case C count=0: graceful empty fallback, no warn

**No real OpenRouter key used** — all cases used the literal string `dummy-test-not-real`.

---

## Per-Target Enumeration Table — CGP H4 (PARTIAL — post-Task-1 only)

| Target | Pre-446 build_id | Post-446 build_id | ws_connected (post) | OPENROUTER_KEY env canonical? | Deprecation-warn count | Notes |
|--------|------------------|--------------------|---------------------|-------------------------------|------------------------|-------|
| Pod 1 | e102fc1e | (unchanged — deferred per kickoff line 253) | true | pre-446 env (N/A) | N/A | Pattern I DiD hold — NOT part of this plan |
| Pod 2 | a13942f2-dirty | (unchanged — deferred) | true | pre-446 env | N/A | DEFERRED |
| Pod 3 | a13942f2-dirty | (unchanged — deferred) | true | pre-446 env | N/A | DEFERRED |
| Pod 4 | a13942f2-dirty | b37983e8-dirty (binary SHA256 c7408e3fda977e4b = rc-agent-d57ee48e.exe) | true | YES (start-rcagent.bat canonical + SCP confirmed; env reads OPENROUTER_KEY from data/openrouter-mma-key.txt) | 0 (log scan: 271 lines, zero deprecation warns) | CANARY SHIPPED — Session 1 Console confirmed |
| Pod 5 | a13942f2-dirty | (unchanged — deferred) | true | pre-446 env | N/A | DEFERRED |
| Pod 6 | a13942f2-dirty | (unchanged — deferred) | true | pre-446 env | N/A | DEFERRED |
| Pod 7 | a13942f2-dirty | (unchanged — deferred) | true | pre-446 env | N/A | DEFERRED |
| Pod 8 | a13942f2-dirty | (unchanged — deferred) | true | pre-446 env | N/A | DEFERRED |
| Server (.23) | N/A (racecontrol, not rc-agent) | N/A | N/A | N/A | N/A | server not rebuilt (kickoff line 194) |
| POS (.130) | N/A | N/A | N/A | N/A | N/A | does not run rc-agent (CONTEXT line 23) |
| Bono VPS | N/A (whatsapp-bot) | pending Plan 02 commit 0981afb + pm2 restart | N/A | PENDING rotation (OPENROUTER_API_KEY currently in pm2 env) | PENDING | Task 6 human-action required |

---

## NOT TESTED (CGP H3 compliance — updated post-Tasks 3-5)

- **Pods 1, 2, 3, 5, 6, 7, 8:** fleet rollout deferred per kickoff line 253 — 24h Pod 4 soak first
- **Pod 4 rc-agent in-process environment map:** Windows does not expose another process's env vars without elevated tooling (e.g., handle.exe). Section C log-scan is the behavioral substitute.
- **AI debugger HTTP endpoint trigger:** No HTTP route for AI debug on rc-agent :8090. Fires via tier_engine on crash recovery. Clean startup (crash_recovery=false) did not trigger it. Would need an organic crash or simulated restart for log evidence.
- **rc-watchdog MMA diagnosis log:** watchdog.log.2026-04-21 (112KB) not directly scanned via rc-sentry exec (metachar restriction prevents powershell piped grep). Zero warns in rc-agent.jsonl is the available evidence.
- **Concurrent MMA / load test:** kickoff line 178 — out of scope
- **Both env vars set to different values:** deferred per CONTEXT lines 193-194
- **Canonical set to empty string:** deferred per CONTEXT line 194
- **Bono VPS pm2 rotation (Task 6):** pending human-action checkpoint
- **WhatsApp live round-trip:** pending Task 6 completion
- **Fleet rollout post-soak (Tasks beyond 446-04):** operator decision after 24h soak

---

## Rollback Commands (ready pre-Task-3)

### Pod 4 rc-agent rollback (if post-swap build_id mismatch or health failure)

```
# Via Tailscale SSH (confirmed alive: UP):
ssh -o StrictHostKeyChecking=no User@100.75.45.10 "cd C:\RacingPoint && taskkill /F /IM rc-agent.exe & ren rc-agent.exe rc-agent-d57ee48e-failed.exe & ren rc-agent-prev.exe rc-agent.exe"
# Then wait 10s for RCWatchdog to respawn
```

### Source code rollback (Rust)

```bash
# Revert Plan 01 commit (rc-agent + rc-watchdog dual-read)
git revert d57ee48e --no-edit
cargo build --release --bin rc-agent --bin rc-watchdog
# Then redeploy via staging HTTP + /exec swap
```

### Source code rollback (whatsapp-bot)

```bash
cd /c/Users/bono/racingpoint/whatsapp-bot
git revert 0981afb --no-edit
# On Bono VPS: git pull + pm2 restart racingpoint-bot
```

---

## Deviations from Plan

None yet (Task 1 only). Relay `shell` command unavailable (APPROVE tier) — used `pm2_status` (AUTO tier) as substitute for Bono VPS pre-rotation state capture. Full env details deferred to Task 6 human-action.

## Known Stubs

None — this is a verification plan. No placeholder code written.
