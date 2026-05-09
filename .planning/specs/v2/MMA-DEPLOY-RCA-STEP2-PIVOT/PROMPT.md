# MMA Step 2 PIVOT — `/exec_atomic_deploy` server-side architecture — PLAN authoring

You are designing the second-iteration PR PLAN for the rc-agent fleet deploy mechanism RCA. The previous Step 2 PLAN (`single_exec_chain` client-side approach) was BLOCKED at Step 4 VERIFY (2.12/5 vs 4.0 PASS threshold) because of structural fragility. Captain has explicitly authorized the pivot to gemini's deferred `new_atomic_endpoint` architecture.

**Bundle scope**: CF-1 (atomic kill+swap) + CF-2 (OTA sentinel discipline) + CF-9 (watchdog deploy-aware health checks) — three Step 1 DIAGNOSE consensus findings, now bundled into a single coherent architectural change because Step 4 VERIFY established that CF-1+CF-2 alone are not safely separable from CF-9.

## Pipeline lineage

| Step | Date | Outcome | Artifact |
|---|---|---|---|
| Step 1 DIAGNOSE | 2026-05-09 ~14:08 UTC, $0.0409 | 9 CONSENSUS findings CF-1..CF-9 + 4 NOVEL | `MMA-DEPLOY-RCA-DIAGNOSE/CONSENSUS.md` |
| Step 2 PLAN (original) | 2026-05-09 ~14:18 UTC, $0.0474 | CF-1+CF-2 bundle PLAN — `single_exec_chain` + JSON sentinel; gemini's `new_atomic_endpoint` deferred | `MMA-DEPLOY-RCA-STEP2/CONSENSUS-PLAN.md` (BLOCKED) |
| Step 4 VERIFY | 2026-05-09 ~14:54 UTC, $0.0728 | overall 2.12/5 = BLOCK; 5 convergent flaws across sonnet+nano | `MMA-DEPLOY-RCA-STEP4/CONSENSUS-VERIFY.md` |
| Step 2 PIVOT (this run) | 2026-05-09 ~15:30 UTC | (your output) | `MMA-DEPLOY-RCA-STEP2-PIVOT/CONSENSUS-PLAN.md` |

## Architecture context

8 sim-racing pods on Windows. Each runs:
- `rc-agent.exe` (port 8090) — main pod agent (Rust, tokio reactor)
- `rc-sentry.exe` (port 8091) — separate process, exec endpoint, survives rc-agent kill
- `rc-watchdog.exe` (Windows service) — polls rc-agent health every 5-10s, calls `perform_rollback()` if rc-agent fails health check 2+ times after restart

Key files:
- `crates/rc-sentry/src/main.rs` — `/exec` endpoint + `BLOCKED_PATTERNS` guard at line 722
- `crates/rc-watchdog/src/service.rs:234-559` — `exe_dir`, `agent_binary_path`, rollback trigger logic
- `crates/rc-watchdog/src/rollback_manager.rs:104-216` — `perform_rollback()` (line 121-128 OTA_DEPLOYING check via bare `is_file()`; line 174-187 binary removal; line 190 prev→current restoration)
- `crates/rc-watchdog/src/rollback_manager.rs` `auto_clear_maintenance_mode_json()` — proven TTL+JSON+mtime-fallback pattern (BUG-71); models should mirror this for `auto_clear_ota_deploying_json`
- `scripts/deploy-pod.sh:138` — current SHA filter line containing `" | "` (CF-4)
- Pod 8 currently on PR #66 binary `8e378f4d` (5h+ stable); Pods 1-7 on `c5f94e31-dirty`; Pod 5 UNREACHABLE

## Step 1 CONSENSUS findings to bundle (CF-1, CF-2, CF-9)

**CF-1 (P0, 5/5)** — Non-atomic kill+swap race vs RCWatchdog. `deploy-pod.sh` uses 3 separate `/exec` HTTP roundtrips for kill/copy/move; watchdog wins the race; OLD binary respawns. Original Step 2 fix: single CMD `&` chain (client-side). Step 4 BLOCK: chain is "fragile by construction" — race not eliminated, only probabilistically mitigated.

**CF-2 (P0, 5/5)** — `OTA_DEPLOYING` sentinel discipline gap. No TTL, managed external to swap, prone to omission. Original Step 2 fix: TTL JSON sentinel mirroring `auto_clear_maintenance_mode_json`. Step 4 BLOCK: sentinel-before-chain ordering creates silent fleet death (FL-CONV-1).

**CF-9 (P0, 4/5 NOVEL)** — Watchdog deploy-aware health checks. When `OTA_DEPLOYING` exists, watchdog should extend POLL_INTERVAL to 30s and skip rollback entirely. Expose `startup_phase` + `graceful_shutdown_in_progress` via `/health` for bilateral protocol. Step 4 BLOCK confirms this is BLOCKING dependency, not optional (FL-CONV-2: Pod 8 OLD watchdog reads JSON via `is_file()` and suppresses indefinitely).

## Step 4 BLOCK convergent flaws to address (FL-CONV-1..5)

The new PLAN must explicitly demonstrate how each flaw is addressed:

**FL-CONV-1 (P0, both models)** — Sentinel-before-chain ordering creates silent fleet death (300s TTL window if script crashes between sentinel-write and atomic-chain). Server-side `/exec_atomic_deploy` endpoint should manage sentinel lifecycle internally — no client-side ordering hazard.

**FL-CONV-2 (P0, both models)** — Pod 8 OLD rc-watchdog reads JSON sentinel via bare `is_file()` → suppresses rollback indefinitely. CF-9 (watchdog deploy-aware mode) is a BLOCKING dependency. New rc-watchdog must reach all pods (including Pod 8) BEFORE deploy-pod.sh JSON sentinel format change rolls out. PLAN must specify: (a) `scripts/deploy-watchdog.sh` (currently does NOT exist), (b) watchdog rollout ordering relative to rc-agent rollout, (c) feature-flag gating if simultaneous rollout impossible.

**FL-CONV-3 (P1, both models)** — A2 JSON parse failure behavior unspecified. Server-side endpoint should handle parse failures internally with explicit policy (suggest: log WARNING + fall back to mtime; deny rollback if mtime within bounded window).

**FL-CONV-4 (P1, both models)** — Race window probabilistically mitigated, no timing analysis (taskkill latency, AV scanning, file ops). Server-side mutex eliminates this concern entirely — server holds lock across the swap; watchdog can query `is_deploy_in_progress()` synchronously.

**FL-CONV-5 (P1, both models)** — rc-watchdog deploy `sc start` failure unhandled. PLAN must include post-start health check (poll `sc query RCWatchdog` for RUNNING with 30s timeout) + Windows Service Recovery settings documentation.

## Sonnet's structural recommendation (verbatim from Step 4 VERIFY)

> "Gemini's `new_atomic_endpoint` has a fundamentally different correctness property: it moves the atomicity guarantee into the server (rc-sentry), where it can hold a mutex, check sentinel state, and perform the swap as a single operation with proper error handling and rollback."

## Your task — design the CF-1+CF-2+CF-9 bundle PLAN

Design a PLAN that:

1. **Adds `POST /exec_atomic_deploy` endpoint to rc-sentry** that:
   - Accepts: `{binary_url, expected_sha256, expected_build_id, deploy_id, timeout_secs}`
   - Holds a process-wide deploy mutex (must be cross-thread, fail-fast on contention)
   - Internally: writes OTA_DEPLOYING JSON sentinel → executes kill+rename swap → verifies binary present → returns `{success: true, swap_completed_at: ISO8601, deploy_id}` OR `{success: false, error: enum, deploy_id, sentinel_cleared: bool}`
   - On any failure: rolls back partial state + clears sentinel + returns explicit error
   - Idempotent on retry with same deploy_id (prevents double-swap on client timeout-retry)
2. **Updates rc-watchdog to be deploy-aware** (CF-9):
   - On poll cycle: BEFORE health check, query rc-sentry `/deploy_state` (or read sentinel + parse TTL JSON internally)
   - If deploy in progress: extend POLL_INTERVAL to 30s, skip rollback evaluation entirely
   - If sentinel parse fails: fall back to mtime check with bounded grace window (suggest: 60s)
   - Exposes `startup_phase` + `graceful_shutdown_in_progress` + `deploy_in_progress` via `/health`
3. **Addresses CF12-Q4 deploy-watchdog distribution**:
   - Specifies `scripts/deploy-watchdog.sh` (mirror of `scripts/deploy-sentry.sh` pattern)
   - Specifies rollout ordering: rc-watchdog must land on ALL 7 deployable pods (Pods 1-4, 6-8) BEFORE deploy-pod.sh format change. Pod 5 OUT-OF-SCOPE (UNREACHABLE).
   - If pod cannot receive new rc-watchdog (e.g., transient unreachability): explicit hold-and-retry vs deploy-with-feature-flag-gate
4. **Updates `scripts/deploy-pod.sh`** to use `/exec_atomic_deploy` instead of multi-step `/exec` chain
5. **Specifies the `auto_clear_ota_deploying_json` policy**: TTL value (default 300s mirroring MAINTENANCE_MODE), JSON parse failure fallback (mtime), legacy bare-file fallback (mtime)
6. **Test plan** that explicitly exercises:
   - The race scenario (artificial delay injection + watchdog poll during swap)
   - JSON parse failure (corrupted/partial sentinel content)
   - sc-start failure on rc-watchdog deploy
   - Backward compat (NEW deploy-pod.sh + OLD rc-watchdog on Pod 8 during transition)
   - Live Pod 1 canary post-rc-watchdog-deploy

7. **Risk + rollback plan** with explicit failure modes (network drops mid-deploy, mutex deadlock, sentinel TTL expiry during long deploy)

## Output format (JSON only)

```json
{
  "pr_title": "fix(rc-sentry/rc-watchdog/deploy-pod): atomic deploy endpoint + watchdog deploy-aware mode (CF-1+CF-2+CF-9 bundle)",
  "selected_approach": {
    "cf1_atomicity": "server_side_mutex_in_exec_atomic_deploy",
    "cf2_sentinel": "internal_lifecycle_in_endpoint_with_ttl_json_and_mtime_fallback",
    "cf9_watchdog_aware": "deploy_state_query_with_extended_poll_interval"
  },
  "files_touched": [
    {"file": "...", "kind": "edit|new|delete", "loc_estimate": 0}
  ],
  "actions": [
    {"id": "A1", "file": "...", "kind": "...", "summary": "...", "loc_estimate": 0, "risk": "low|medium|high", "risk_reason": "...", "addresses_flaw": ["FL-CONV-N", "..."]}
  ],
  "test_plan": [
    {"id": "T1", "kind": "unit|integration|live-pod", "what": "...", "expected": "...", "exercises_flaw": ["FL-CONV-N", "..."]}
  ],
  "rollout_plan": {
    "phase_1": "rc-watchdog deploy via deploy-watchdog.sh to Pods 1-4, 6-8 (Pod 5 OUT-OF-SCOPE); per-pod sc query verify; HALT on any failure",
    "phase_2": "deploy-pod.sh updated to use /exec_atomic_deploy; canary on Pod 1 with new rc-watchdog; 5min stability soak",
    "phase_3": "fleet rollout Pods 2,3,4,6,7 sequential; Pod 8 last (canary stays on existing PR #66 binary until phase 3)",
    "rollback_plan": "..."
  },
  "captain_q_decisions": [
    {"id": "PV-Q1", "question": "...", "default_recommendation": "...", "rationale": "..."}
  ],
  "verify_post_deploy": [
    {"step": 1, "command": "...", "pass_criterion": "..."}
  ],
  "fl_conv_addressing": {
    "FL-CONV-1_sentinel_before_chain": "<how this PLAN eliminates the silent-fleet-death window>",
    "FL-CONV-2_pod_8_old_watchdog": "<rollout ordering + backward compat strategy>",
    "FL-CONV-3_json_parse_fail": "<explicit policy>",
    "FL-CONV-4_race_timing": "<server-side mutex eliminates timing dependency>",
    "FL-CONV-5_sc_start_fail": "<post-start health check + recovery>"
  },
  "loc_summary": {"prod": 0, "tests": 0, "total": 0, "pr_shape": "small|medium|large"},
  "minority_dissent": "..."
}
```

**Constraints**:
- Production-safe: no flag day; backward compat preserved during rollout
- Smallest sustainable change: ~200 LOC code + ~150 LOC tests is the gemini-original estimate; if your plan exceeds 500 LOC, surface why
- Each action MUST list which FL-CONV flaw(s) it addresses
- Be specific: file paths + line numbers + function names where known

Be skeptical of your own design. If `new_atomic_endpoint` has a flaw the Step 4 VERIFY didn't catch, surface it in `minority_dissent`.
