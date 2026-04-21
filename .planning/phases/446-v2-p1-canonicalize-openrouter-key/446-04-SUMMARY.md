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
metrics:
  duration_minutes: TBD
  completed_date: "2026-04-21"
  tasks_completed: 1
  files_modified: 1
---

# Phase 446 Plan 04: Pod 4 Canary + Bono VPS Rotation — Summary

**Status: PARTIAL — paused at Task 2 checkpoint (human-verify required)**

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

## Task 2: Checkpoint — paused for user approval

See CHECKPOINT REACHED message below. Tasks 3-7 pending.

---

## Per-Target Enumeration Table — CGP H4 (PARTIAL — post-Task-1 only)

| Target | Pre-446 build_id | Post-446 build_id | ws_connected (post) | OPENROUTER_KEY env canonical? | Deprecation-warn count | Notes |
|--------|------------------|--------------------|---------------------|-------------------------------|------------------------|-------|
| Pod 1 | e102fc1e | (unchanged — deferred per kickoff line 253) | true | pre-446 env (N/A) | N/A | Pattern I DiD hold — NOT part of this plan |
| Pod 2 | a13942f2-dirty | (unchanged — deferred) | true | pre-446 env | N/A | DEFERRED |
| Pod 3 | a13942f2-dirty | (unchanged — deferred) | true | pre-446 env | N/A | DEFERRED |
| Pod 4 | a13942f2-dirty | PENDING (d57ee48e — awaiting user approval at Task 2 checkpoint) | true | pending (start-rcagent.bat already canonical per Plan 03) | PENDING | CANARY TARGET — awaiting swap authorization |
| Pod 5 | a13942f2-dirty | (unchanged — deferred) | true | pre-446 env | N/A | DEFERRED |
| Pod 6 | a13942f2-dirty | (unchanged — deferred) | true | pre-446 env | N/A | DEFERRED |
| Pod 7 | a13942f2-dirty | (unchanged — deferred) | true | pre-446 env | N/A | DEFERRED |
| Pod 8 | a13942f2-dirty | (unchanged — deferred) | true | pre-446 env | N/A | DEFERRED |
| Server (.23) | N/A (racecontrol, not rc-agent) | N/A | N/A | N/A | N/A | server not rebuilt (kickoff line 194) |
| POS (.130) | N/A | N/A | N/A | N/A | N/A | does not run rc-agent (CONTEXT line 23) |
| Bono VPS | N/A (whatsapp-bot) | pending Plan 02 commit 0981afb + pm2 restart | N/A | PENDING rotation (OPENROUTER_API_KEY currently in pm2 env) | PENDING | Task 6 human-action required |

---

## NOT TESTED (partial — will be expanded post-Tasks 3-7)

- Pod 4 post-swap behavior: pending Task 2 user approval + Task 3 swap
- Tasks 3-7: all pending
- Pods 1, 2, 3, 5, 6, 7, 8: fleet rollout deferred per kickoff line 253
- Concurrent MMA / load test: kickoff line 178
- Both env vars set to different values: deferred per CONTEXT lines 193-194
- Canonical set to empty string: deferred per CONTEXT line 194

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
