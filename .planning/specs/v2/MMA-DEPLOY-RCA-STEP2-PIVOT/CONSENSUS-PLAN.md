> **⛔ STATUS — BLOCKED-AT-STEP-4-VERIFY-PIVOT-ROUND (2026-05-09 ~21:36 IST)**
>
> MMA Step 4 VERIFY PIVOT-round adversarial gate scored this PIVOT PLAN at **1.75/5 overall** (PASS threshold = 4.0); 3/3 models returned BLOCK. **SECOND CONSECUTIVE BLOCK** on this RCA pipeline.
>
> **DO NOT author PR commits from this PIVOT PLAN until Captain dispositions amendments.** See `../MMA-DEPLOY-RCA-STEP4-PIVOT/CONSENSUS-VERIFY.md` for:
> - 2 P0 convergent flaws (PV-FL-1 Tokio Mutex cancellation hazard leaks partial state · PV-FL-2 rc-sentry crash orphans deploy state — mistral SPOF concern from PIVOT round PROVEN insufficient at 60s mtime fallback)
> - 4 P1 convergent flaws (PV-FL-3 Phase 1 circular dependency · PV-FL-4 Pod 8 OLD-sentry 404 path · PV-FL-5 missing chaos tests · PV-FL-6 mutex poisoning unaddressed)
> - 3/3 models surfaced **Win32 `LockFileEx`** as strongest alternative architecture (OS-level mutex persistence across crashes; eliminates PV-FL-1 + PV-FL-2 + PV-FL-6)
> - 5 revised disposition options (PV2-OPT-A respin / **PV2-OPT-B LockFileEx pivot** / PV2-OPT-C single-pilot amend / PV2-OPT-D halt / **PV2-OPT-E manual bridge for customer impact**)
>
> Captain Q-DECISION required before any code is written from this PIVOT PLAN. My hybrid recommendation: **PV2-OPT-E (manual bridge per G9 #4 sentinel discipline) + PV2-OPT-B (LockFileEx) queued for next session**.

---

# MMA Step 2 PIVOT — CF-1+CF-2+CF-9 bundle (`/exec_atomic_deploy` server-side architecture) — CONSENSUS

**Captain-authorized**: 2026-05-09 ~21:04 IST verbatim "Pivot to gemini's new_atomic_endpoint (~$0.05-0.10 MMA cost; sustainable structural fix; no pod touch yet)" → Option B explicit ratification post-Step-4-BLOCK
**Consumes**:
- `MMA-DEPLOY-RCA-DIAGNOSE/CONSENSUS.md` (CF-1, CF-2, CF-9 bundle)
- `MMA-DEPLOY-RCA-STEP4/CONSENSUS-VERIFY.md` (FL-CONV-1..5 to address)
**Supersedes**: `MMA-DEPLOY-RCA-STEP2/CONSENSUS-PLAN.md` (BLOCKED-AT-STEP-4-VERIFY 2.12/5)
**Models**: 5 vendor-distinct (deepseek/anthropic/xiaomi/google/mistral)
**Vendor families**: 5 (≥3 required ✓)
**Roles**: ≥1 reasoner (deepseek-r1) + ≥1 code expert (sonnet-4.6) + ≥1 SRE (mimo-v2-pro) ✓
**Wall time**: 361.4s (deepseek-r1 longest at 6.0min; gemini 31.9s; mistral 16.6s)
**Cost**: $0.1548 / $5 budget (sonnet-4.6 alone = $0.10, dominant)
**Cumulative MMA-day spend**: ~$0.72 / $5
**Hooks**: pre-mma-duplicate-check passed (Step 2 PIVOT ≠ Step 2 PLAN — different session_purpose; new dir avoids file-mtime trigger)
**Truncations**: 2/5 hit max_tokens=6000 (sonnet-4.6 + mimo-v2-pro). Both produced complete `selected_approach` + `actions` sections. sonnet truncated mid-rollout-plan; mimo truncated mid-rollout-plan. Neither truncation lost the architectural consensus signal.

---

## §1 — Selected approach (5/5 unanimous)

| Dimension | Approach | Models |
|---|---|---|
| **CF-1 atomicity** | `server_side_mutex_in_exec_atomic_deploy` | 5/5 |
| **CF-2 sentinel** | `internal_lifecycle_in_endpoint_with_ttl_json_and_mtime_fallback` | 5/5 |
| **CF-9 watchdog-aware** | `deploy_state_query_with_extended_poll_interval` (HTTP poll OR direct file read fallback) | 5/5 |

**Rationale**: This is the architecture sonnet's Step 4 critique recommended pivoting to. It eliminates client-side race window (FL-CONV-4), eliminates client-side sentinel ordering hazard (FL-CONV-1), and provides synchronous deploy-state visibility for rc-watchdog (FL-CONV-2). The original `single_exec_chain` approach was BLOCKED at 2.12/5 by both Step 4 VERIFY models for being "fragile by construction" — this PIVOT moves atomicity into rc-sentry where it can be enforced via mutex.

---

## §2 — Files touched (5-model consensus)

| File | Models | Action class | Median LOC |
|---|---|---|---|
| `crates/rc-sentry/src/main.rs` | **5/5** | edit — register `/exec_atomic_deploy` + `/deploy_state` routes; integrate atomic_deploy module; AppState extension; BLOCKED_PATTERNS audit | ~120 |
| `crates/rc-sentry/src/atomic_deploy.rs` | **4/5** (gemini/sonnet/mimo/mistral; deepseek-r1 inlines into main.rs) | new — core handler + mutex + sentinel lifecycle + idempotency by deploy_id | ~190 |
| `crates/rc-watchdog/src/service.rs` | **5/5** | edit — poll loop deploy-state query; extend POLL_INTERVAL to 30s during deploy; expose `deploy_in_progress` via /health; reset failure counter on deploy-end | ~55 |
| `crates/rc-watchdog/src/rollback_manager.rs` | **5/5** | edit — replace bare `is_file()` (line 121) with TTL-aware DeployStateChecker; add `auto_clear_ota_deploying_json()` mirroring BUG-71 pattern | ~70 |
| `crates/rc-watchdog/src/deploy_state.rs` | **3/5** (gemini/sonnet/mistral; deepseek-r1+mimo inline into rollback_manager) | new — DeployStateChecker module: HTTP query rc-sentry first, sentinel-file fallback second, mtime-fallback third | ~75 |
| `scripts/deploy-pod.sh` | **5/5** | edit — replace 3 separate `/exec` calls with single `POST /exec_atomic_deploy`; add deploy_id generation; parse JSON response | ~35 |
| `scripts/deploy-watchdog.sh` | **5/5** | new — mirror `deploy-sentry.sh`; sc stop → copy → sc start → sc query verify (30s timeout); rollback to previous binary on failure | ~55 |
| Test suite (1-2 files) | **5/5** | new — race scenarios + JSON parse fail + idempotency + backward compat + sc-start fail + canary | ~150 |

**Optional / single-model**:
- `crates/rc-agent/src/http_handlers/health.rs` (deepseek-r1 only) — expose `startup_phase`/`graceful_shutdown_in_progress` via rc-agent /health
- `crates/rc-watchdog/src/health.rs` (mistral only) — separate watchdog health module

**Median LOC summary**: ~500 prod + ~150 tests = **~650 total**. **PR shape: MEDIUM-LARGE** (3 of 5 models classified as MEDIUM; gemini and sonnet classified as LARGE due to higher individual estimates).

---

## §3 — Action plan (consensus-merged, 8 actions)

| ID | File | Kind | Summary | LOC | Risk | Addresses |
|---|---|---|---|---|---|---|
| **A1** | `crates/rc-sentry/src/atomic_deploy.rs` | new | Module: `POST /exec_atomic_deploy` handler. `tokio::sync::Mutex<Option<ActiveDeploy>>` (process-wide). Request: `{binary_url, expected_sha256, expected_build_id, deploy_id, timeout_secs}`. Response: `{success, deploy_id, swap_completed_at, error: enum, sentinel_cleared}`. Flow: try_lock fail-fast → idempotency check by deploy_id → write OTA_DEPLOYING JSON sentinel → taskkill → copy temp → atomic rename(s) → verify SHA256 → clear sentinel → release mutex. On any failure: rollback partial state + clear sentinel + return explicit error. **Critical**: MutexGuard MUST be held across all file ops without `.await` cancellation hazards; use `tokio::select!` with `timeout_secs` to bound operation. | ~190 | high | FL-CONV-1, FL-CONV-3, FL-CONV-4 |
| **A2** | `crates/rc-sentry/src/atomic_deploy.rs` | (in A1) | Helpers: `write_ota_deploying_sentinel()` + `clear_ota_deploying_sentinel()` + `auto_clear_ota_deploying_json()`. JSON schema: `{deploy_id, started_at: ISO8601, ttl_secs: u32 default 300, build_id, pid}`. Atomic write via temp-file + rename (BUG-71 pattern). Parse-fail policy: log WARNING + mtime fallback (60s grace, intentionally < TTL to bound corrupted-sentinel blast radius). Legacy bare-file fallback: same mtime check. | (in A1 LOC) | low | FL-CONV-1, FL-CONV-2, FL-CONV-3 |
| **A3** | `crates/rc-sentry/src/main.rs` | edit | Register routes: `POST /exec_atomic_deploy` + `GET /deploy_state` (returns `{in_progress, deploy_id, started_at, ttl_remaining_secs}` via `try_lock` non-blocking). AppState extends with `Arc<Mutex<Option<ActiveDeploy>>>`. **AUDIT BLOCKED_PATTERNS line 722** — must NOT block new endpoint paths. Existing `/exec` route unchanged (backward compat for non-migrated tooling). | ~120 | medium | FL-CONV-1, FL-CONV-4 |
| **A4** | `crates/rc-watchdog/src/deploy_state.rs` | new | Module `DeployStateChecker`. Method `check_deploy_in_progress() -> DeployCheckResult`. Strategy chain: (1) HTTP `GET rc-sentry:8091/deploy_state` 2s timeout → if `in_progress=true`, return `DeployInProgress`. (2) If sentry unreachable: read `OTA_DEPLOYING` sentinel file → JSON parse → TTL check. (3) Parse-fail: mtime fallback 60s grace. (4) Sentinel absent: return `DeployNotInProgress`. Result enum: `DeployInProgress{deploy_id, grace_until} | DeployNotInProgress | CheckFailed{reason}`. **Fail-open policy** (CheckFailed → allow rollback) — surfaced as PV-Q3. | ~75 | medium | FL-CONV-2, FL-CONV-3, FL-CONV-4 |
| **A5** | `crates/rc-watchdog/src/service.rs` | edit | Modify poll loop (lines 234-559). At top of each cycle, BEFORE health check: `DeployStateChecker::check_deploy_in_progress()`. If `DeployInProgress`: set `next_poll_interval = 30s`, skip rollback evaluation entirely, log INFO. If `DeployNotInProgress`/`CheckFailed`: proceed with existing logic. **Reset failure counter** on deploy-end transition (avoid stale count triggering immediate rollback post-deploy). Expose `deploy_in_progress` + `startup_phase` + `graceful_shutdown_in_progress` via /health. | ~55 | medium | FL-CONV-2, FL-CONV-4, FL-CONV-5 |
| **A6** | `crates/rc-watchdog/src/rollback_manager.rs` | edit | Lines 121-128: replace bare `OTA_DEPLOYING.is_file()` check with `DeployStateChecker::check_deploy_in_progress()` (reuse A4 module). Same TTL+JSON+mtime semantics in poll loop and rollback path. Lines 174-187 + 190 (binary swap logic): no change. Add log line at rollback entry: `'perform_rollback() called; deploy_in_progress={}'`. | ~70 | high | FL-CONV-2, FL-CONV-3 |
| **A7** | `scripts/deploy-pod.sh` | edit | Replace multi-step `/exec` chain with single `POST /exec_atomic_deploy` call. New flow: compute local SHA256 → POST `{binary_url, expected_sha256, expected_build_id, deploy_id=$(uuidgen), timeout_secs=120}` → parse JSON `{success}` → poll `rc-agent:8090/health` 30s for RUNNING. Client-side sentinel write **REMOVED**. CF-4 BLOCKED_PATTERNS issue (line 138 `" \| "`) tagged as TODO (out of scope — separate PR). | ~35 | low | FL-CONV-1, FL-CONV-4 |
| **A8** | `scripts/deploy-watchdog.sh` | new | Mirror `scripts/deploy-sentry.sh`. Args: POD_IP, WATCHDOG_BINARY_PATH. Steps: SCP staged binary → `sc stop RCWatchdog` → poll STOPPED 15s → copy over rc-watchdog.exe → `sc start RCWatchdog` → poll RUNNING 30s (FL-CONV-5 fix). Failure path: `sc stop` + restore previous binary from `.prev` backup + `sc start` + exit 1. Verify rc-watchdog `/health` 200 within 10s. Document Windows Service Recovery: `sc failure RCWatchdog reset=86400 actions=restart/5000/restart/10000/restart/30000`. Caller MUST HALT fleet rollout on first failure. | ~55 | medium | FL-CONV-2, FL-CONV-5 |
| **A9** | tests (1-2 files) | new | Unit + integration. T1 race scenario (5/5). T2 JSON parse fail (5/5). T3 idempotent deploy_id (4/5). T4 backward compat OLD watchdog + new sentinel (5/5). T5 sc-start failure (5/5). T6 mutex contention fail-fast (3/5). T7 TTL expiry during long deploy (2/5). T8 live Pod 1 canary post-watchdog-deploy (5/5). | ~150 | low | all FL-CONV |

**Total**: ~500 LOC code + ~150 LOC tests = **~650 LOC**. **PR shape: MEDIUM-LARGE**.

---

## §4 — Test plan (consensus-merged, 8 tests)

| ID | Kind | What | Expected | Exercises |
|---|---|---|---|---|
| **T1** | integration | Inject 500ms-2s sleep between taskkill and final rename in `/exec_atomic_deploy`. Concurrent watchdog poll attempts rollback. | rollback returns early ("deploy in progress"). swap completes. sentinel cleared. | FL-CONV-1, FL-CONV-4 |
| **T2** | unit | Write corrupted JSON sentinel ('{invalid'). Within 60s mtime, expect `DeployInProgress`. After 60s, expect `DeployNotInProgress` + sentinel deleted + WARNING. | parse-fail policy enforced. | FL-CONV-3 |
| **T3** | integration / unit | Idempotent retry: 2 calls with same deploy_id; first succeeds, second returns `AlreadyCompleted` with original `swap_completed_at`. No double-swap. | deploy_id idempotency works. | FL-CONV-4 |
| **T4** | live-pod | OLD rc-watchdog (pre-A6, bare `is_file()`) on Pod 8 + new `/exec_atomic_deploy` runs against another pod. OLD watchdog reads bare sentinel via `is_file()` → suppresses indefinitely (KNOWN behavior, not regression). After deploy completes + sentinel cleared, OLD watchdog resumes normal poll. **Documents transition window risk**; elimination requires A8 rollout ordering (Phase 1 first). | OLD watchdog suppresses during deploy + resumes after sentinel cleared. No permanent suppression. | FL-CONV-2 |
| **T5** | integration | Mock `sc start` returns STOPPED after start attempt. Assert `deploy-watchdog.sh` exits non-zero, restores previous binary, attempts `sc start` of previous binary. | sc-start failure recovery works. | FL-CONV-5 |
| **T6** | unit | Mutex contention: spawn 2 concurrent deploy requests with **different** deploy_ids. Second returns `MutexContention` immediately (fail-fast). | mutex fail-fast verified. | FL-CONV-4 |
| **T7** | integration | TTL expiry: set TTL to 5s in test config. Inject 10s sleep in swap step. Assert `auto_clear_ota_deploying_json()` fires at ~5s. Watchdog (extended 30s poll) detects sentinel gone next poll. Deploy handler returns `Timeout` + clears partial state. | TTL expiry path works during long deploy. | FL-CONV-1, FL-CONV-3 |
| **T8** | live-pod | Pod 1 canary post-watchdog-deploy (Phase 2 of rollout). Deploy new rc-watchdog → verify `sc query` RUNNING + GET /health includes `deploy_in_progress` field. Then run new `deploy-pod.sh` → monitor watchdog logs for "deploy in progress, skipping health check". Verify rc-agent on new binary, no spurious rollback. **5min stability soak**. | Pod 1 stable on new binary, watchdog deploy-aware skip observed, no rollback, /health = `startup_phase: 'running'`. | FL-CONV-1, FL-CONV-2, FL-CONV-4 |

---

## §5 — Rollout plan (5/5 consensus on phases)

### Phase 1 — rc-watchdog rollout (BLOCKING PRECURSOR)
- Deploy new rc-watchdog to **Pods 1, 2, 3, 4, 6, 7, 8** sequentially via `scripts/deploy-watchdog.sh` (one-at-a-time per Standing Rule "test before upload")
- **Pod 5 OUT-OF-SCOPE** — UNREACHABLE per `project_pod5_offline_during_deploy_rca_20260509.md`; physical recovery is separate work item
- Per-pod verify: `sc query RCWatchdog` returns RUNNING within 30s + `GET /health` 200 within 10s
- HALT fleet rollout on FIRST failure
- Transient unreachability: retry after 15min (max 3 retries) before escalating
- **Critical**: Pod 8 must receive new rc-watchdog in this phase — otherwise FL-CONV-2 transition window risk persists indefinitely

### Phase 2 — `/exec_atomic_deploy` canary on Pod 1
- Update `scripts/deploy-pod.sh` in repo (commit + merge to main per Captain auth)
- Run new `deploy-pod.sh` targeting Pod 1 only (Pod 1 already on new rc-watchdog from Phase 1)
- Monitor 5min: rc-agent /health build_id + watchdog logs ("deploy in progress" entries) + zero rollback events + heartbeat advancing
- Behavior verification (per H3): rc-agent serves expected build_id endpoint AND demonstrates the silent-loop-death fix (panic-hook + heartbeat thread observable)

### Phase 3 — fleet rollout
- Sequential deploy to **Pods 2, 3, 4, 6, 7** using new `deploy-pod.sh` + new rc-watchdog from Phase 1
- Per-pod 2min stability soak
- **Pod 8 deployed LAST** — Pod 8 is currently the canonical canary on PR #66 binary `8e378f4d` (6.5h+ stable); only swap when fleet is otherwise green

### Rollback plan
- **Phase 1 failure**: revert rc-watchdog via `deploy-watchdog.sh` with previous binary (`.prev` backup preserved). New rc-watchdog binary preserved 72hr per Standing Rule.
- **Phase 2 failure**: revert `scripts/deploy-pod.sh` commit on `feat/...` branch; manually revert rc-agent on Pod 1 via OLD `deploy-pod.sh` chain (with sentinel discipline per G9 #4). NO new code runs.
- **Phase 3 failure**: HALT rollout. Investigate. Pod 8 stays on PR #66 binary as fleet fallback reference. Revert affected pods individually.
- **In-flight `/exec_atomic_deploy` failure**: endpoint's internal rollback restores partial state + clears sentinel. Watchdog mtime-fallback (60s) clears stale locks if sentry crashes mid-deploy.

---

## §6 — FL-CONV-1..5 addressing (5/5 unanimous claims)

| Flaw | Resolution |
|---|---|
| **FL-CONV-1** sentinel-before-chain silent fleet death | `/exec_atomic_deploy` manages sentinel lifecycle internally (write before kill, clear after verify, clear on rollback). NO client-side ordering hazard. Mutex held across the entire sequence. |
| **FL-CONV-2** Pod 8 OLD watchdog suppresses indefinitely | **Phase 1 rollout ordering** (rc-watchdog FIRST to ALL reachable pods incl Pod 8) eliminates the transition window. New rc-watchdog handles both legacy bare-file (mtime fallback) and new JSON-format sentinel. OLD watchdog during Phase 1 transition: reads JSON sentinel via `is_file()` and suppresses ROLLBACK (not indefinitely permanent — only until Phase 1 reaches the pod). |
| **FL-CONV-3** JSON parse fail unspecified | Explicit policy in A4: log WARNING + mtime fallback with 60s grace window (intentionally < 300s TTL to bound corrupted-sentinel blast radius). Test T2 verifies. |
| **FL-CONV-4** race window probabilistically mitigated | Server-side `tokio::sync::Mutex` eliminates all client-side timing dependency. Watchdog queries `/deploy_state` synchronously (mutex try_lock); deploy state is authoritative server-side. |
| **FL-CONV-5** sc-start failure unhandled | A8 `deploy-watchdog.sh` polls `sc query RCWatchdog` for RUNNING with 30s timeout. Failure → restore previous binary + attempt `sc start prev` + exit 1. Documents Windows Service Recovery via `sc failure` for self-healing. Test T5 verifies. |

---

## §7 — Captain Q-DECISIONs (5 PV-Q items)

| ID | Question | Default recommendation | Sources |
|---|---|---|---|
| **PV-Q1** | Should `/deploy_state` endpoint be authenticated? | **NO** — internal pod-network endpoint; only exposes deploy status (no destructive action); auth adds rollout complexity. `/exec_atomic_deploy` itself uses existing X-Service-Key per `/exec` pattern. | gemini default; aligns with existing rc-sentry policy |
| **PV-Q2** | OTA_DEPLOYING TTL value: 300s vs 600s? | **300s** (matches MAINTENANCE_MODE pattern). Mistral dissented for 600s with dynamic adjustment. Consensus 4/5 is 300s. | 4/5 default (gemini, deepseek-r1, sonnet, mimo); mistral dissent (600s + dynamic) |
| **PV-Q3** | Watchdog `CheckFailed` policy: fail-open (allow rollback) vs fail-closed (suppress rollback)? | **Fail-open** — if watchdog cannot determine deploy state, do NOT suppress rollback indefinitely (that risks masking real crash). | sonnet default; safer-conservative |
| **PV-Q4** | rc-sentry crash mid-deploy: who detects + remediates? | **Mtime fallback + watchdog HTTP timeout** — if sentry crashes mid-deploy, sentinel stays present until TTL expires (300s) OR watchdog mtime check (60s grace) fires whichever first. No additional component needed. **Mistral dissent**: add watchdog-monitor of `/exec_atomic_deploy` endpoint state. | gemini/deepseek-r1/sonnet/mimo default; mistral surfaces additional safety layer |
| **PV-Q5** | Test T4 backward compat — explicit acceptance of OLD-watchdog transition window risk? | **YES — accept** — Phase 1 rollout ordering bounds the window to <30min total fleet rollout time. T4 documents the behavior; doesn't eliminate it (only rollout ordering does). | 5/5 consensus on rollout ordering; T4 = documentation test |

---

## §8 — Minority dissents

- **gemini**: "loc_estimate (~960 total) significantly higher than gemini-original ~350 estimate. Pushes into LARGE PR category. Higher review burden + integration risk than initially anticipated. Necessary complexity given scope, but acknowledge as deviation from 'smallest sustainable change' ideal." — **Disposition**: SURFACE in PR description; estimate tracks median ~650 LOC = MEDIUM PR (not LARGE); 8 actions → 8 commits or grouped commits enable incremental review.
- **deepseek-r1**: "Mutex deadlock if endpoint crashes mid-deploy. Mitigation: mutex uses timeout (suggest 300s)." — **Disposition**: ABSORB into A1 — `tokio::select!` with `timeout_secs` MANDATORY; PV-Q4 documents.
- **sonnet**: "Fail-open policy is conservative but correct. Alternative (fail-closed → suppress rollback) risks indefinite suppression if sentry is down." — **Disposition**: ABSORB as PV-Q3 default; surfaced for Captain ratification.
- **mistral**: "Atomic endpoint introduces new SPOF (rc-sentry crash mid-deploy). Comprehensive error handling but crash during critical section could leave pod in partially deployed state. Consider watchdog-specific health check that monitors `/exec_atomic_deploy` state and triggers emergency rollback if endpoint becomes unresponsive during a deploy." — **Disposition**: PARTIAL ABSORB — mtime fallback in A4 + watchdog 30s extended poll covers most scenarios; PV-Q4 surfaces for Captain. Watchdog-monitor-of-endpoint deferred to V2.1 if mtime fallback proves insufficient.

---

## §9 — What I observed (CGP H3 evidence)

**BEHAVIOR**: Authored Step 2 PIVOT PROMPT.md (10791 chars), invoked 5 vendor-distinct models in parallel, parsed JSON responses (3 clean / 2 truncated at max_tokens=6000), synthesized 8-action consensus PLAN with 5/5 architectural agreement.

**RAW OUTPUT**:
- `resp-deepseek-r1.md` (6781 chars; finish=stop; clean JSON; 6.0min)
- `resp-sonnet-4.6.md` (19701 chars; finish=length; truncated mid-rollout-plan; selected_approach + 10 actions + 7 tests + FL-CONV addressing complete; 2.0min; $0.10 dominant)
- `resp-mimo-v2-pro.md` (7822 chars; finish=length; truncated mid-rollout-plan; 8 actions + FL-CONV addressing complete; 1.6min)
- `resp-gemini-flash.md` (17102 chars; finish=stop; clean JSON; 31.9s; original new_atomic_endpoint proposer)
- `resp-mistral-sm.md` (9795 chars; finish=stop; clean JSON; 16.6s)
- `meta-*.json` per model
- `results.json` summary
- Spend ledger appended at 2026-05-09T15:34:19Z

**WHERE**: Models invoked from James .27 via OpenRouter HTTPS. Outputs to `racecontrol/.planning/specs/v2/MMA-DEPLOY-RCA-STEP2-PIVOT/`. NO live-pod activity.

**NOT TESTED**:
- Step 4 VERIFY adversarial gate on this PIVOT PLAN (recommended next step; ~$0.05-0.08 for 3 fresh-vendor adversarial models; would address sonnet's prompt-anchoring concern from prior Step 4 since this PROMPT explicitly framed atomic-endpoint architecture)
- Whether the proposed code actually compiles or runs
- Whether the mutex implementation correctly bounds operation across `tokio::select!` (concurrency review needed at Step 3 EXECUTE)
- Whether Pod 8 OLD watchdog actually behaves as predicted under test (T4 is a live-pod test deferred to Phase 1 of rollout)
- Bono cross-pilot AMPLIFIER vote on this PIVOT (single-pilot synthesis; no INBOX notify sent this turn)
- Whether sonnet/mimo response truncation lost critical signal — both responses were truncated AFTER selected_approach + actions + FL-CONV addressing; rollout_plan + later sections only partially captured but consensus is preserved by 3 clean responses (deepseek-r1/gemini/mistral)

---

## §10 — Spend log entry (appended)

```json
{
  "timestamp": "2026-05-09T15:34:19Z",
  "ts_ist": "2026-05-09 21:04 IST",
  "pilot": "james",
  "session_purpose": "MMA Step 2 PIVOT — CF-1+CF-2+CF-9 bundle (new_atomic_endpoint server-side architecture) — deploy-mechanism RCA — Captain Option B post-Step-4-BLOCK",
  "mma_step": "PLAN-PIVOT",
  "models": [
    {"model": "deepseek/deepseek-r1-0528", "status": "OK", "cost": 0.0168, "finish": "stop", "elapsed_s": 361.4},
    {"model": "anthropic/claude-sonnet-4.6", "status": "OK", "cost": 0.1006, "finish": "length", "elapsed_s": 120.5},
    {"model": "xiaomi/mimo-v2-pro", "status": "OK", "cost": 0.0213, "finish": "length", "elapsed_s": 94.1},
    {"model": "google/gemini-2.5-flash", "status": "OK", "cost": 0.0139, "finish": "stop", "elapsed_s": 31.9},
    {"model": "mistralai/mistral-small-2603", "status": "OK", "cost": 0.0022, "finish": "stop", "elapsed_s": 16.6}
  ],
  "vendor_families": ["deepseek", "anthropic", "xiaomi", "google", "mistral"],
  "valid_responses": 5,
  "total_responses": 5,
  "total_cost_usd": 0.1548,
  "anchor": ".planning/specs/v2/MMA-DEPLOY-RCA-STEP2-PIVOT/CONSENSUS-PLAN.md",
  "consumes_step1": ".planning/specs/v2/MMA-DEPLOY-RCA-DIAGNOSE/CONSENSUS.md (CF-1+CF-2+CF-9)",
  "supersedes_step2": ".planning/specs/v2/MMA-DEPLOY-RCA-STEP2/CONSENSUS-PLAN.md (BLOCKED 2.12/5)",
  "addresses_flaws": ".planning/specs/v2/MMA-DEPLOY-RCA-STEP4/CONSENSUS-VERIFY.md FL-CONV-1..5",
  "verdict": "AWAITING-STEP-4-VERIFY-ADVERSARIAL-GATE",
  "consensus_actions": 8,
  "consensus_tests": 8,
  "captain_q_decisions": 5,
  "minority_dissent_count": 4,
  "loc_summary_median": "~500 prod + ~150 tests = ~650 total, MEDIUM-LARGE PR shape",
  "authorization": "Captain Option B explicit ratification 2026-05-09 ~21:04 IST after Step 4 BLOCK"
}
```

---

## §11 — Open items for Captain (next-step decision)

| Option | Action | Cost | Time |
|---|---|---|---|
| **PV-OPT-1** | Approve PIVOT PLAN as-is + proceed to Step 4 VERIFY adversarial gate (3 fresh-vendor models, ≥4.0 score = PASS, BLOCK halts again if <4.0) | ~$0.05-0.10 | ~5min wall |
| **PV-OPT-2** | Approve PIVOT PLAN + skip Step 4 VERIFY + proceed directly to Step 3 EXECUTE (PR authoring) | $0 | ~30-60min PR authoring |
| **PV-OPT-3** | Amend defaults for any of PV-Q1..Q5 before Step 4 VERIFY | $0 | <5min |
| **PV-OPT-4** | Defer to bono AMPLIFIER (cross-pilot 24h CHALLENGE-AMEND) before Step 4 VERIFY | $0 | 24h | 
| **PV-OPT-5** | Reject PIVOT — different architecture (e.g., PowerShell-based atomic deploy or feature-flag dual-write) | varies | varies |

**Recommended sequencing**: PV-OPT-1 (Step 4 VERIFY first). Cost is bounded; gate caught real flaws on prior PLAN; PROMPT this round was framed around atomic-endpoint architecture so prompt-anchoring bias is in the OPPOSITE direction now (would surface "what's wrong with atomic endpoint?" — useful adversarial signal).

---

— james / 2026-05-09 ~21:08 IST · MMA Step 2 PIVOT complete · 5/5 valid · 8-action consensus · 5 Captain Q-DECISIONs queued · awaits Step 4 VERIFY OR direct Step 3 EXECUTE per Captain disposition · cumulative MMA-day spend ~$0.72/$5
