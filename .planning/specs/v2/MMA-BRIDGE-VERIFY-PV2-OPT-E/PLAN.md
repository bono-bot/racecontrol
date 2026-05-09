# Bridge PLAN — PV2-OPT-E manual atomic-chain-with-sentinel deploy of PR #66 binary

- **Author:** james
- **Date:** 2026-05-09 22:40 IST
- **Captain ratification:** hybrid `PV2-OPT-E + PV2-OPT-B` 2026-05-09 (bridge now; LockFileEx defer)
- **Validates against:** 2 prior MMA Step 4 BLOCKs (original CONSENSUS-PLAN 2.12/5, PIVOT 1.75/5)
- **Status:** AWAITING Step 4 VERIFY adversarial gate (this PLAN itself)

## 1. Scope

| Pod | IP | Current binary | Action |
|-----|----|----------------|--------|
| Pod 1 | 192.168.31.89 | c5f94e31-dirty (degraded — prior failed deploy) | Bridge LAST in ramp |
| Pod 2 | 192.168.31.33 | c5f94e31-dirty (stable 13h+) | PILOT first |
| Pod 3 | 192.168.31.28 | c5f94e31-dirty (stable) | Ramp 1 |
| Pod 4 | 192.168.31.88 | c5f94e31-dirty (stable) | Ramp 2 |
| Pod 5 | 192.168.31.86 | UNREACHABLE | SKIP (physical) |
| Pod 6 | 192.168.31.87 | c5f94e31-dirty (stable) | Ramp 3 |
| Pod 7 | 192.168.31.38 | c5f94e31-dirty (stable) | Ramp 4 |
| Pod 8 | 192.168.31.91 | 8e378f4d (PR #66 canary 6.5h+ stable) | UNTOUCHED |

- **MECHANISM:** Manual operational bridge — NO code changes to rc-agent / rc-sentry / rc-watchdog
- **DEFERRED to next session:** PV2-OPT-B Win32 LockFileEx structural rewrite

## 2. Rationale (why bridge over LockFileEx now)

- 6 pods exposed to silent-loop-death (PR #66 RCA scope). Customer fix urgency.
- 2 prior MMA Step 4 BLOCKs on code-level PRs (2.12/5 then 1.75/5) demonstrate architectural complexity exceeds single-PR-with-MMA-iteration capacity.
- Bridge bypasses code-path entirely; uses existing OLD-watchdog sentinel mechanism (`OTA_DEPLOYING` + `auto_clear_maintenance_mode_json` from BUG-71).
- Single-pod blast radius (~30s downtime), reversible via preserved `rc-agent-prev.exe`.
- PV2-OPT-D (defer all) accepts unbounded silent-loop-death exposure — rejected by Captain.

## 3. Sentinel mechanism (existing in c5f94e31 OLD watchdog — verified preflight required)

- **File:** `C:\RacingPoint\OTA_DEPLOYING` (text or JSON)
- **JSON form:** `{"timestamp_epoch":N,"action_id":"<uuid>","reason":"manual-bridge-pv2-opt-e"}`
- **TTL:** 300s via `auto_clear_maintenance_mode_json(300)` in `crates/rc-watchdog/src/service.rs:261`
- **mtime fallback (BUG-71):** if JSON corrupt, uses file mtime + 300s
- **Watchdog behavior when sentinel present:**
  - `perform_rollback()` returns early at `crates/rc-watchdog/src/rollback_manager.rs:121-128`
  - service.rs main loop logs `OTA_DEPLOYING active — skipping restart cycle (SF-05)` at line 288
- **Preflight gate:** `git show c5f94e31:crates/rc-watchdog/src/rollback_manager.rs` MUST contain OTA_DEPLOYING handling (else mechanism not present in OLD binary; abort bridge)

## 4. Per-pod procedure (sequential, one pod at a time)

### 4.1 Pre-flight (read-only, ~30s/pod)
1. Probe build_id via `GET :8091/status` (rc-sentry) — confirm `c5f94e31-dirty`
2. Probe ws + http via Server `:8080/api/v1/fleet/health` filtered by pod_number — confirm ws=True
3. Probe rc-watchdog liveness via `:8091/exec` `Get-Service RCWatchdog` — confirm Running
4. Probe panic.log size via `:8091/exec dir C:\RacingPoint\panic.log` — confirm absent or <1KB
5. md5 of in-place rc-agent.exe via `:8091/exec certutil -hashfile C:\RacingPoint\rc-agent.exe MD5` — record
6. Verify PR #66 binary already at C:\RacingPoint\rc-agent-8e378f4d.exe on pod (or stage via James-served HTTP)

### 4.2 Sentinel set (SEPARATE /exec call, completes before chain begins)
- POST :8091/exec cmd: write JSON to `C:\RacingPoint\OTA_DEPLOYING` (Write-to-file pattern per CLAUDE.md Git-Bash-JSON rule)
- Wait 1s flush
- Verify: POST :8091/exec cmd `dir C:\RacingPoint\OTA_DEPLOYING` — confirm size > 0

### 4.3 Atomic chain (SINGLE /exec call, ~10s wall-clock)
POST :8091/exec cmd (single Windows-CMD `&` chain):
```
taskkill /F /IM rc-agent.exe & ping -n 3 127.0.0.1 > nul &
copy /Y C:\RacingPoint\rc-agent.exe C:\RacingPoint\rc-agent-prev.exe &
copy /Y C:\RacingPoint\rc-agent-8e378f4d.exe C:\RacingPoint\rc-agent.exe &
ping -n 2 127.0.0.1 > nul
```
Watchdog detects rc-agent gone, sees `OTA_DEPLOYING` present, suppresses rollback, respawns rc-agent.exe via existing `RCWatchdog` service (Session 1 via `WTSQueryUserToken` + `CreateProcessAsUser` per CLAUDE.md).

### 4.4 Healthcheck (poll, max 60s)
Every 3s for up to 60s, poll `:8091/exec curl :8090/healthz`:
- **PASS condition:** status 200 AND build_id == 8e378f4d AND ws_connected == True
- **TIMEOUT condition:** 60s without PASS → invoke rollback (4.6)

### 4.5 Sentinel clear (SEPARATE /exec call after PASS)
- POST :8091/exec cmd `del C:\RacingPoint\OTA_DEPLOYING`
- Confirm file gone

### 4.6 Rollback (if 4.4 TIMEOUT or 4.7 fails)
- Re-set sentinel (extends TTL +300s)
- Atomic chain reverse: `taskkill /F /IM rc-agent.exe & ping -n 3 127.0.0.1 > nul & copy /Y C:\RacingPoint\rc-agent-prev.exe C:\RacingPoint\rc-agent.exe & ping -n 2 127.0.0.1 > nul`
- Healthcheck: build_id == c5f94e31-dirty
- Clear sentinel
- HALT entire ramp; surface to Captain

### 4.7 Post-flight verification (5min observation window)
1. build_id == 8e378f4d (from /healthz)
2. ws_connected == True (from server fleet-health)
3. http_reachable (from server fleet-health)
4. heartbeat.txt mtime advancing every ~30s — 3 samples 60s apart, all incrementing
5. panic.log empty / unchanged size
6. rc-watchdog logs show no rollback attempt within window (via :8091/exec tail of watchdog log)

## 5. Pod ordering (pilot → ramp)

`Pod 2 (PILOT) → Pod 3 → Pod 4 → Pod 6 → Pod 7 → Pod 1 (LAST)` · skip Pod 5 · untouched Pod 8

## 6. Captain wait points
- After pilot pre-flight (4.1), before sentinel set (4.2)
- After pilot post-flight verify (4.7) PASS, before ramp Pod 3
- After Pod 1 (last in ramp), before declaring done

## 7. Acceptance per pod (all 6 must hold)
- All 6 post-flight checks pass within 5min observation
- No rc-watchdog rollback observed
- No new panic.log entries
- Heartbeat advancing continuously

## 8. Failure modes addressed (FROM prior Step 4 BLOCKs)

### From original CONSENSUS-PLAN BLOCK 2.12/5
- **FL-CONV-1 sentinel-before-chain:** Addressed — sentinel SET via separate /exec call (4.2) and verified BEFORE atomic chain (4.3) begins.
- **FL-CONV-2 watchdog suppression indefinite:** Addressed — TTL=300s + `auto_clear_maintenance_mode_json` enforces hard ceiling. Worst case = 5min suppression.
- **FL-CONV-3 JSON parse fail:** Addressed — mtime fallback (BUG-71 pattern) handles corrupt JSON; file existence + mtime alone suffices.
- **FL-CONV-4 race timing missing:** Addressed — HEALTHCHECK polls at 3s intervals for 60s; chain phases non-overlapping; sentinel cleared only post-PASS.
- **FL-CONV-5 sc-start unhandled:** Addressed — taskkill+respawn-by-RCWatchdog model has built-in retry; if respawn fails, sentinel remains set + healthcheck times out → rollback (4.6).

### From PIVOT BLOCK 1.75/5
- **PV-FL-1 Tokio Mutex cancellation hazard:** NOT APPLICABLE — bridge uses no Tokio, only Windows-CMD chain via single /exec.
- **PV-FL-2 rc-sentry SPOF:** PARTIALLY ACCEPTED — rc-sentry IS the bridge transport. If rc-sentry dies mid-chain, sentinel remains set + watchdog suppresses rollback for 300s + manual recovery via Tailscale SSH (CLAUDE.md fallback).
- **PV-FL-3 Phase 1 circular dep:** NOT APPLICABLE — bridge is single-phase per pod, no cross-pod dependency.
- **PV-FL-4 Pod 8 OLD-sentry 404:** NOT APPLICABLE — Pod 8 untouched, already on PR #66.
- **PV-FL-5 chaos tests missing:** ACCEPTED RISK — manual operation under real-time observation; single-pod blast radius limits damage.
- **PV-FL-6 mutex poisoning:** NOT APPLICABLE — no Rust Mutex involved.

## 9. Failure modes NOT addressed (deferred to PV2-OPT-B next session)
- Win32 LockFileEx kernel-level mutual exclusion
- /exec_atomic_deploy server-side mutex endpoint
- Bilateral coordination protocol watchdog↔sentinel
- Long-term cooperative deploy-aware watchdog redesign

## 10. Risk profile

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| Atomic chain race lost vs watchdog | LOW (sentinel) | 1 pod stuck OLD | Re-attempt with verified sentinel timing |
| Watchdog respawns OLD binary | LOW (sentinel suppresses) | 1 pod stuck OLD | TTL=300s ceiling; manual rollback prepared |
| rc-sentry dies mid-chain | LOW | sentinel remains, manual recovery | Tailscale SSH fallback per CLAUDE.md |
| Pod 1 prev binary corrupted | MEDIUM | 1 pod stuck broken | Pod 1 LAST; canonical OLD binary archived offline at deploy-staging |
| Healthcheck false-pass | LOW | bad deploy goes ramp-wide | 6-check post-flight with 5min observation gate |
| New panic in PR #66 binary | LOW (Pod 8 stable 6.5h+) | 1 pod stuck broken, ramp halt | Rollback to OLD c5f94e31 binary |
| Chain typo / cmd.exe quoting bug | LOW | unpredictable | Pilot pod gates ramp; Write-to-file pattern per CLAUDE.md |
| sequential ramp under-observation | MEDIUM | undetected gradual degradation | 5min/pod × 6 pods + Captain go-no-go between pilot+ramp |
| concurrent parallel-james touches Pods | MEDIUM | duplicate deploy / race | Bono INBOX preflight notify before bridge starts |

- Per-pod blast radius: 1 pod, ~30s downtime
- Total wall-clock: ~30min sequential (pre-flight + atomic + 5min observation × 6 pods)
- Reversibility: rc-agent-prev.exe preserved per pod for at least 72h per CLAUDE.md OTA pipeline rule

## 11. Composes-with
- §S-146 V1↔V2 RCA doctrine (foundational pod-state-channel boundary)
- §S-150 PR #66 silent-loop-death merged
- §S-159 pre-MMA-duplicate-check hook (this gate is its first beneficiary on bridge-class)
- Sentinel discipline rule PROMOTE-NOW-ACTIVE 2026-05-09
- Dry-test rule PROMOTE-NOW-ACTIVE 2026-05-09 (pilot Pod 2 = single-target dry-test)
- §S-166 model-role-fit code enforcement
- per-PR Captain auth rule (hybrid verb satisfies)
