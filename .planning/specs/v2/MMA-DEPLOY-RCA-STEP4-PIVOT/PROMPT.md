# MMA Step 4 VERIFY (PIVOT round) — adversarial review of `/exec_atomic_deploy` PIVOT PLAN

You are an adversarial reviewer. Your job is to find what's WRONG with the PIVOT PLAN — not validate it. The prior Step 2 PLAN (`single_exec_chain` client-side approach) was BLOCKED at Step 4 VERIFY (2.12/5) for being "fragile by construction". Captain authorized a PIVOT to gemini's deferred `new_atomic_endpoint` architecture. The PIVOT PLAN claims to address all 5 convergent flaws (FL-CONV-1..5). Your task: challenge those claims and surface NEW flaws that the 5-model PIVOT consensus may have missed.

**Critical framing**: The PIVOT prompt was deliberately framed AROUND atomic-endpoint architecture. This means the 5-model PIVOT consensus may suffer from the OPPOSITE prompt-anchoring bias from the prior round (which was anchored toward `single_exec_chain` via CLAUDE.md "Remote deploy sequence"). Your adversarial role is to find what's wrong with `new_atomic_endpoint` — concurrency hazards, single-point-of-failure risks, rollout-ordering edge cases, mutex semantics under tokio cancellation, things the consensus glossed over.

## Pipeline lineage

| Step | Date | Verdict | Spend |
|---|---|---|---|
| Step 1 DIAGNOSE | 2026-05-09 14:08 UTC | 9 CONSENSUS findings CF-1..CF-9 + 4 NOVEL | $0.0409 |
| Step 2 PLAN (original `single_exec_chain`) | 2026-05-09 14:18 UTC | shipped (later BLOCKED) | $0.0474 |
| Step 4 VERIFY (prior round) | 2026-05-09 14:54 UTC | **2.12/5 = BLOCK** (5 convergent flaws FL-CONV-1..5; sonnet structural critique → "fragile by construction") | $0.0728 |
| Step 2 PIVOT (`/exec_atomic_deploy`) | 2026-05-09 15:34 UTC | shipped, 5/5 unanimous architectural consensus | $0.1548 |
| Step 4 VERIFY PIVOT (this run) | 2026-05-09 ~16:05 UTC | (your output) | TBD |

## PIVOT PLAN summary (what you are reviewing)

### Selected approach (5/5 unanimous in PIVOT round)

- **CF-1 atomicity**: `server_side_mutex_in_exec_atomic_deploy` — `tokio::sync::Mutex<Option<ActiveDeploy>>` in rc-sentry, process-wide, fail-fast on contention
- **CF-2 sentinel**: `internal_lifecycle_TTL_json_mtime_fallback` — endpoint manages OTA_DEPLOYING JSON sentinel internally; auto-clear on TTL=300s; 60s mtime fallback for parse failures
- **CF-9 watchdog-aware**: `deploy_state_query_extended_poll_interval` — watchdog queries `rc-sentry:8091/deploy_state` first, sentinel-file fallback, mtime-fallback; extends POLL_INTERVAL to 30s during deploy

### 8 actions (file + LOC + risk)

| ID | File | Summary | LOC | Risk |
|---|---|---|---|---|
| A1 | `crates/rc-sentry/src/atomic_deploy.rs` (new) | `POST /exec_atomic_deploy` handler. Try_lock fail-fast → idempotency check by deploy_id → write OTA_DEPLOYING JSON → taskkill → copy temp → atomic rename(s) → verify SHA256 → clear sentinel → release mutex. On any failure: rollback partial state + clear sentinel + explicit error. **MutexGuard MUST be held across all file ops without `.await` cancellation hazards; `tokio::select!` with `timeout_secs` to bound operation.** | ~190 | high |
| A2 | (in A1) | `write_ota_deploying_sentinel()` + `clear_ota_deploying_sentinel()` + `auto_clear_ota_deploying_json()`. JSON: `{deploy_id, started_at, ttl_secs:300, build_id, pid}`. Atomic temp+rename. Parse-fail: log WARNING + 60s mtime fallback. Legacy bare-file: same mtime check. | (in A1) | low |
| A3 | `crates/rc-sentry/src/main.rs` (edit) | Register routes: `POST /exec_atomic_deploy` + `GET /deploy_state`. AppState extends with `Arc<Mutex<Option<ActiveDeploy>>>`. **AUDIT BLOCKED_PATTERNS line 722** — must NOT block new endpoint paths. Existing `/exec` route unchanged (backward compat). | ~120 | medium |
| A4 | `crates/rc-watchdog/src/deploy_state.rs` (new) | `DeployStateChecker`. Strategy chain: (1) HTTP `GET rc-sentry:8091/deploy_state` 2s timeout. (2) sentinel-file fallback on sentry unreach. (3) JSON parse + TTL check. (4) Parse-fail: 60s mtime fallback. (5) Sentinel absent: `DeployNotInProgress`. **Fail-open policy** (CheckFailed → allow rollback). | ~75 | medium |
| A5 | `crates/rc-watchdog/src/service.rs` (edit) | Modify poll loop. Before health check: `check_deploy_in_progress()`. If `DeployInProgress`: extend `POLL_INTERVAL` to 30s, skip rollback evaluation, log INFO. **Reset failure counter on deploy-end transition.** Expose `deploy_in_progress` + `startup_phase` + `graceful_shutdown_in_progress` via /health. | ~55 | medium |
| A6 | `crates/rc-watchdog/src/rollback_manager.rs` (edit) | Lines 121-128: replace bare `OTA_DEPLOYING.is_file()` with `DeployStateChecker::check_deploy_in_progress()`. Lines 174-187 + 190 (binary swap logic): no change. Add log line at rollback entry. | ~70 | high |
| A7 | `scripts/deploy-pod.sh` (edit) | Replace multi-step `/exec` chain with single `POST /exec_atomic_deploy` call. Compute local SHA256 → POST `{binary_url, expected_sha256, expected_build_id, deploy_id, timeout_secs:120}` → parse JSON → poll `rc-agent:8090/health` 30s for RUNNING. Client-side sentinel write REMOVED. | ~35 | low |
| A8 | `scripts/deploy-watchdog.sh` (new) | Mirror `deploy-sentry.sh`. SCP staged binary → `sc stop RCWatchdog` → poll STOPPED 15s → copy → `sc start` → poll RUNNING 30s. Failure: restore previous binary + retry sc start of previous + exit 1. Verify watchdog `/health` 200 within 10s. Document Windows Service Recovery via `sc failure`. | ~55 | medium |
| A9 | tests | T1 race scenario, T2 JSON parse fail, T3 idempotent deploy_id, T4 backward compat, T5 sc-start fail, T6 mutex contention, T7 TTL expiry, T8 live Pod 1 canary | ~150 | low |

**Total**: ~500 prod + ~150 tests = **~650 LOC, MEDIUM-LARGE PR**.

### FL-CONV-1..5 addressing claims (from PIVOT consensus, 5/5)

| Flaw | Claim |
|---|---|
| **FL-CONV-1** sentinel-before-chain silent fleet death | "`/exec_atomic_deploy` manages sentinel lifecycle internally (write before kill, clear after verify, clear on rollback). NO client-side ordering hazard. Mutex held across the entire sequence." |
| **FL-CONV-2** Pod 8 OLD watchdog suppresses indefinitely | "**Phase 1 rollout ordering** (rc-watchdog FIRST to ALL reachable pods incl Pod 8) eliminates the transition window. New rc-watchdog handles both legacy bare-file (mtime fallback) and new JSON-format sentinel. OLD watchdog during Phase 1 transition: reads JSON sentinel via `is_file()` and suppresses ROLLBACK (not indefinitely permanent — only until Phase 1 reaches the pod)." |
| **FL-CONV-3** JSON parse fail unspecified | "Explicit policy in A4: log WARNING + mtime fallback with 60s grace window (intentionally < 300s TTL to bound corrupted-sentinel blast radius). Test T2 verifies." |
| **FL-CONV-4** race window probabilistically mitigated | "Server-side `tokio::sync::Mutex` eliminates all client-side timing dependency. Watchdog queries `/deploy_state` synchronously (mutex try_lock); deploy state is authoritative server-side." |
| **FL-CONV-5** sc-start failure unhandled | "A8 `deploy-watchdog.sh` polls `sc query RCWatchdog` for RUNNING with 30s timeout. Failure → restore previous binary + attempt `sc start prev` + exit 1. Documents Windows Service Recovery via `sc failure` for self-healing. Test T5 verifies." |

### 5 Captain Q-DECISIONs queued (PV-Q1..Q5)

- PV-Q1: `/deploy_state` authentication = NO (default; internal endpoint)
- PV-Q2: TTL value 300s vs 600s = 300s default (4/5; mistral 600s+dynamic dissent)
- PV-Q3: `CheckFailed` → fail-open (allow rollback) vs fail-closed (suppress) = fail-open default
- PV-Q4: rc-sentry crash mid-deploy = mtime+watchdog fallback (default); mistral wants watchdog-monitor-of-endpoint added
- PV-Q5: T4 transition window risk acceptance = YES (default; bounded by Phase 1)

### 4 minority dissents documented

- gemini: LOC higher than original ~350 estimate; pushes into LARGE PR
- deepseek-r1: mutex deadlock risk if endpoint crashes; mitigation = timeout
- sonnet: fail-open is conservative-correct vs fail-closed risks indefinite suppression
- mistral: rc-sentry becomes new SPOF; consider watchdog-monitor-of-endpoint with emergency rollback

## Your adversarial task

For each scoring dimension, return a numeric score 0-5 with rationale. Then list specific FLAWS or risks the consensus may have missed.

### Scoring dimensions

1. **Correctness** — Does `tokio::sync::Mutex<Option<ActiveDeploy>>` actually provide atomicity across `taskkill + copy + rename` on Windows? `tokio::sync::Mutex` is async-aware; if the handler `.await`s during the swap, can the runtime cancel the future and drop the MutexGuard mid-swap? Does `tokio::select!` with `timeout_secs` actually trigger cleanup of partial filesystem state on the timeout arm, or does it just race two futures?
2. **Risk coverage** — Failure modes the PLAN may have missed: (a) rc-sentry process crash mid-swap (mistral surfaced this; default disposition is mtime fallback — is 60s grace sufficient?); (b) `tokio::sync::Mutex` poisoning if the handler panics; (c) rc-watchdog deploy on Pod 8 itself — when rc-watchdog binary is being swapped, what manages rc-watchdog? (d) cross-pod deploy_id collision (UUIDs are local, no global registry); (e) deploy fleet-state divergence if Phase 1 partially succeeds.
3. **Backward compatibility** — Phase 1 ordering claim: "rc-watchdog deployed FIRST to all pods including Pod 8". But the ROLLOUT ITSELF requires deploy-pod.sh, which is being changed in this PR. How does Phase 1 work without circular dependency? If A8 (`deploy-watchdog.sh`) is new, has it ever been tested on Pod 8? Pod 8 is currently the canary on PR #66 — touching its rc-watchdog has high blast radius.
4. **Test plan adequacy** — T1 injects 500ms-2s sleep — is that sufficient to exercise the actual race? rc-watchdog polls every 5-10s; if the deploy chain completes in <500ms in normal conditions, the race may never fire in T1. T8 live Pod 1 canary tests happy path only — does it test the SCENARIO where rc-sentry crashes mid-deploy? T4 documents the transition-window risk but doesn't eliminate it — is documentation sufficient?
5. **Concreteness** — A1's claim "MutexGuard MUST be held across all file ops without `.await` cancellation hazards" is a constraint, not an implementation. Tokio's `Mutex` docs explicitly warn about holding guards across `.await`. The `tokio::select!` with `timeout_secs` pattern is mentioned but not specified — what happens to the MutexGuard if the timeout arm fires? Is the cleanup code in the cancellation arm or in a `Drop` impl? A4's "fail-open policy" — what if rc-sentry is permanently dead (binary corrupted, port hijacked)? The watchdog falls open and rollback fires inappropriately.
6. **Independence from anchoring** — The PIVOT prompt explicitly framed `new_atomic_endpoint` as "structurally correct" per sonnet's prior critique. This anchors models toward validating the architecture rather than challenging it. Would a fresh review without that framing reach the same 5/5 consensus, or would alternatives surface (e.g., a separate `rc-deploy-orchestrator` process, OR using systemd-style D-Bus, OR a file-lock-based approach using `LockFile`/`UnlockFile` Win32 APIs)?

### Specific challenge questions

- **Tokio Mutex semantics**: `tokio::sync::Mutex` is meant for async-aware locking and IS held across `.await`. If the handler awaits on `tokio::fs::write()` while holding the guard, that's the EXPECTED pattern. But: if the request times out and the runtime drops the future, the MutexGuard's Drop releases the lock — but the partial filesystem state (sentinel written, taskkill issued, no binary swap) leaks. Is there a `Drop` guard for cleanup, or does cleanup only happen in the success path?
- **Fleet-wide deploy_id collision**: Each pod generates its own UUID. If a deploy script targets multiple pods with the same `deploy_id` (intentional for batch correlation), idempotency caching across pods is impossible (each rc-sentry has its own state). If `deploy_id` is per-pod (regenerated), correlation across pods for fleet-rollout debugging is broken. Which model wins?
- **Phase 1 deploy of rc-watchdog**: A8 says "SCP staged binary → sc stop RCWatchdog → ... → sc start". But during the sc stop window, RCWatchdog is not running. If rc-agent crashes during this window (5-30s), nothing restarts it. Acceptable? On 7 pods sequentially, that's 7 windows of vulnerability.
- **rc-sentry restart**: `rc-sentry` itself never restarts (it's stateful via DEPLOY_MUTEX). If rc-sentry crashes mid-deploy, the new rc-sentry instance starts with empty mutex state — the old deploy is orphaned. The 60s mtime fallback assumes rc-sentry-up; if rc-sentry stays down for >60s, watchdog clears sentinel and starts rolling back the in-progress new binary. This is the mistral SPOF concern unresolved.
- **What about the in-flight deploy when CONSENSUS says "Pod 8 deployed LAST" (Phase 3)?** Pod 8 currently has: PR #66 binary `8e378f4d` running rc-agent + OLD rc-watchdog (no deploy_state endpoint). If Phase 1 ships new rc-watchdog to Pod 8, Pod 8 briefly has new rc-watchdog + OLD rc-agent. New rc-watchdog will poll rc-sentry:8091/deploy_state — but rc-sentry on Pod 8 hasn't been upgraded (this PR doesn't deploy rc-sentry). Does the new rc-watchdog gracefully handle "rc-sentry doesn't have /deploy_state endpoint = 404 = fail-open = allow rollback"? Or does it loop?
- **CF-4 BLOCKED_PATTERNS**: The original deploy-pod.sh:138 SHA filter contains `" | "` which trips BLOCKED_PATTERNS. The PIVOT explicitly tags this as "TODO out of scope". But A7 still calls rc-sentry/exec for the SHA computation step (or does it skip that entirely?). Is the SHA verify step server-side (inside `/exec_atomic_deploy`) or still client-side via `/exec`?
- **Rollout halt criteria**: Phase 1 says "HALT on first failure" — but Phase 3 says "If Phase 3 fails: halt rollout". What about a single pod failure in Phase 3 — does the halt apply mid-pod (e.g., Pod 4 fails after Pods 2,3 succeed)? Or per-phase-completion?

## Output format (JSON only)

```json
{
  "scores": {
    "correctness": 0.0,
    "risk_coverage": 0.0,
    "backward_compatibility": 0.0,
    "test_plan_adequacy": 0.0,
    "concreteness": 0.0,
    "independence_from_anchoring": 0.0,
    "overall": 0.0
  },
  "rationale_per_dimension": {
    "correctness": "...",
    "risk_coverage": "...",
    "backward_compatibility": "...",
    "test_plan_adequacy": "...",
    "concreteness": "...",
    "independence_from_anchoring": "..."
  },
  "flaws_identified": [
    {"id": "PV-FL-1", "severity": "P0|P1|P2", "title": "...", "description": "...", "fix_recommendation": "..."}
  ],
  "missing_from_plan": ["..."],
  "verdict": "PASS|FLAG|BLOCK",
  "verdict_rationale": "<= 250 chars",
  "would_recommend_amendments": ["..."],
  "alternatives_consensus_anchoring_may_have_dismissed": ["..."]
}
```

**Scoring scale**: 0 = catastrophically wrong / 1 = major flaws / 2 = significant gaps / 3 = workable with caveats / 4 = solid, ship-ready / 5 = excellent. Overall = arithmetic mean. Verdict gate: overall ≥4.0 = PASS, 3.0-3.99 = FLAG (proceed with amendments), <3.0 = BLOCK.

Be adversarial. Default to skepticism. The PIVOT consensus was 5/5 unanimous — too unanimous? If the architecture is sound, the score reflects it; if there are gaps, surface them.
