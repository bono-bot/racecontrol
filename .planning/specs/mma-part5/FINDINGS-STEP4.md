# MMA Part 5 Step 4 VERIFY — Findings

**Run:** 2026-04-21 (IST, session on `feat/f4-data-content-check-20260421` HEAD `2046305b`)
**Budget:** $0.0342 / $0.80 used (well under cap)
**Models:** 3 dispatched, 3 returned ok=true, 2/3 valid JSON, 1/3 truncated at 8000-token completion cap
**Raw responses:** `verify/{kimi,mistral,mimo}.json` + `{kimi,mistral,mimo}.meta.json`
**Runner:** `run-step4.js` / stderr: `step4.stderr.log`

## Per-model

| Model | Role | Elapsed | Tokens (p/c) | Cost | Verdict | Score | JSON |
|---|---|---|---|---|---|---|---|
| moonshotai/kimi-k2-0905 | reasoner-adversarial | 75.3s | 15369 / 1828 | $0.0138 | **BLOCK** | **2.0** | ok |
| mistralai/mistral-medium-3.1 | code-expert-adversarial | 27.8s | 18704 / 2855 | $0.0132 | **CONDITIONAL_PASS** | **3.8** | ok |
| xiaomi/mimo-v2-pro | sre-adversarial | 113.0s | 16047 / 8000 (cap) | $0.0072 | **BLOCK** | **2.5** (non-standard `score_out_of_3`) | malformed — truncated mid-response |

Vendor families: Moonshot, Mistral, Xiaomi — 3 families (MMA diversity rule met).
Diversity from Steps 1-3: zero overlap (r1 / v3 / qwen3-coder / nemotron / gemini-flash / grok-code / codex-mini all excluded).

## Aggregate score

**(2.0 + 3.8 + 2.5) / 3 = 2.77**

Per MMA Step 4 rubric: ≥4.0 = PASS unblocks deploy. **2.77 < 4.0 → BLOCK**. Deploy gate remains closed until must-fix defects are addressed.

## Step 1 finding coverage (C1-C9, D3, D6, D9, D11, D13)

All three models agree the implementation addressed the following **cleanly**: C1, C2, C5, C6, C7, C9, D6, D9, D11, D13. Residual-risk = none across models.

Remaining disagreements:

| Finding | kimi | mistral | mimo | Reconcile |
|---|---|---|---|---|
| C3 (T2 cancellation) | `addressed=false` residual=high | `addressed=true` cited event_loop.rs:412-425 | `addressed=false` residual=high (snippet not in prompt) | mistral had line-number evidence; kimi + mimo only saw the module. **Verify: check event_loop.rs for `CancellationToken select!` arm around the T2 tick.** |
| C4 (driver_name from server) | `addressed=true` | `addressed=true` residual=low | `addressed=false` (only server-name, no cache fallback) | mimo flagged that `unwrap_or_default()` ≠ `or_else(cached_name)`. Matches DESIGN-SPEC algorithm step 8 but not code. **Minor gap — P3.** |
| C8 (cached last-known stats) | `addressed=true` (but V-5 P2 defect) | `addressed=false` residual=medium (V-3 P2) | `addressed=false` residual=high | **3/3 consensus: C8 NOT fully addressed — code passes 0/None/0 to `apply_session_ended` instead of cached `session_last_known_stats`.** DESIGN-SPEC algorithm step 8 specifies `last_known.total_laps` etc — code hardcodes zeros. P2. |
| D3 (stuck_session_candidate rule) | `addressed=true` | `addressed=true` | partial response — truncated mid-answer | mistral flagged V-7 P2: rule fires on silent-reconnect + active session but does NOT check `fallback_version` marker freshness, so Part-5-patched pods in a silent-reconnect window still flag. |

## New defects (consensus view — ≥2/3 flag)

### **V-A: reqwest timeout race after status check, before/during `json().await`** — **P0/P1**
- **kimi V-1 P0** confidence 5/5
- **mistral V-1 P1** confidence 5/5
- **Description:** the 5s client-level timeout can fire after `response.status().is_success()` passed but while `response.json().await` is still reading the body. Result: `Err` path silently returns without synth; customer session remains stuck forever. This is the exact "silent drop" class Part 5 was designed to prevent.
- **Trigger:** server returns 200 OK + partial body; network stall at ≥4s. `reqwest::Client::builder().timeout(Duration::from_secs(5))` applies to the total request — partial responses plus stall exceed budget mid-parse.
- **Fix:** log at WARN on timeout-during-parse (currently debug) OR separate the operations: fetch body bytes with its own timeout, then parse in-memory with no timeout. Mistral's specific hint: "check elapsed time before json() parse."
- **Ships-as-is risk:** HIGH — recreates the symptom Part 5 targets.

### **V-B: `reqwest::Client` rebuilt on every `fetch_and_reconcile` call** — **P1/P2**
- **kimi V-4 P1** confidence 4/5 (OS fd exhaustion after weeks)
- **mistral V-5 P2** confidence 4/5 (connection-pool churn)
- **Description:** `session_end_fallback.rs:161-164` builds a new `reqwest::Client` inside the fn. Every T2 tick + every T1 post-reconnect → new client → new TCP connection pool → no reuse.
- **Fix:** hoist to `AppState::session_end_fallback_client: reqwest::Client` constructed at boot. Reuse across calls. ~15 LOC + AppState field.

### **V-C: C8 incomplete — synth uses hardcoded zeros instead of cached last-known stats** — **P2**
- **kimi V-5 P2** confidence 3/5
- **mistral V-3 P2** confidence 5/5
- **mimo C8 residual=high** (truncated, findings partial)
- **Description:** DESIGN-SPEC algorithm step 8 reads `monitor.session_last_known_stats.clone()` and passes `last_known.total_laps, last_known.best_lap_ms, last_known.driving_seconds` to `apply_session_ended`. Code at `session_end_fallback.rs:237-246` hardcodes `0u32, None, 0u32`.
- **Customer impact:** synth summary card shows "0 laps, no best time, 0s driving" for sessions where the driver actually raced. Analytics pipeline records zeros. Spec acknowledged this (C8 = P2) and proposed the cache; the cache was never added.
- **Fix:** either (a) add `session_last_known_stats: Option<SessionStats>` to `FailureMonitorState`, populate from WS lap events, read in fallback — ~40 LOC, or (b) ship Part 5 with zeros + `synth=true` flag in summary card so customers see "incomplete" instead of "0 laps" and close C8 in a follow-up.

### **V-D: Instant monotonicity under Windows suspend** — **P1/P3**
- **kimi V-3 P1** confidence 3/5
- **mistral V-6 P3** confidence 3/5
- **Description:** `saturating_duration_since` can yield 0 if `Instant::now()` jumps back after suspend/hibernate. On pod laptops this is rare (always-on posture) but not impossible.
- **Fix:** if addressed, do it in the same change as Part 4 dead-man's-switch (which must handle suspend regardless). Acceptable to defer this out of Part 5 scope.

## Dissenting / single-model flags (informational, not consensus)

- **mistral V-2 P1 concurrency on `try_claim_apply_session`** — mistral hypothesised a deadlock under simultaneous T1/T2 fire with RwLock write. Only mistral sees this, and `try_claim_apply_session` uses atomic swap on AppState which should not deadlock. Worth a 2-minute code re-read.
- **kimi V-6 P2 silent 401 on mesh_key_cache miss** — if the cached service key file is missing, `get_key_or_env().unwrap_or_default()` passes empty string → server returns 401 → fallback silently skips. Should log ERROR + escalate. ~5 LOC.
- **mistral V-7 P2 fallback_version freshness on stuck_session_candidate** — Part 5 patched pods briefly in silent-reconnect still flag as stuck because the rule doesn't check `fallback_version` age. Operator noise, not customer impact. Deferrable.
- **mistral V-10 P2 grace-check/snapshot ordering** — separate lock acquisition for `local_id` vs `set_at` could in theory see them from different monitor borrows. Code holds a single `failure_monitor_tx.borrow()` block so this does NOT apply. False positive — pattern-match level review, not trace-level.

## Proceed decision

**Deploy remains BLOCKED.** Step 4 VERIFY rubric requires ≥4.0 aggregate. Current 2.77.

### Must-fix before re-running Step 4

1. **V-A** (reqwest timeout race during parse) — fix timeout semantics OR wrap parse in explicit byte-read + in-memory decode. Add a test that mocks `reqwest` returning 200 OK + slow body.
2. **V-B** (Client reuse) — hoist `reqwest::Client` to AppState. Straight ~15 LOC change.
3. **V-C** (cached last-known stats OR explicit `synth=true` flag) — either path acceptable; zeros in customer-facing summary is not.

### Recommended to fix in same commit cycle (cheap)

4. **kimi V-6** (silent 401 → ERROR log). ~5 LOC.
5. **C4 + C8 cache-fallback** for `driver_name` (matches algorithm step 8). ~10 LOC.

### Deferrable to post-deploy follow-up

6. **V-D** (suspend/monotonicity) — handle in Part 4 dead-man's-switch which must handle it anyway.
7. **V-7** (stuck_session_candidate freshness) — operator noise, low customer impact.
8. **Test coverage V-8 / V-9** — add after fixing V-A and V-B.

### Verify before Step 4 re-run

- Confirm C3 (T2 cancellation) reality: does `event_loop.rs` actually `select!` on a `CancellationToken` for the T2 arm? mistral cited line numbers; kimi/mimo could not see it. 1-minute code read will settle it.

## Budget

- Step 1 (DIAGNOSE): $0.0417
- Step 2 (PLAN): $0.060
- Step 4 (VERIFY, this run): $0.0342
- **Running MMA total: $0.136 / $5.00 session budget.**
- Post-fix Step 4 re-run: budget headroom comfortable. Use 3 different adversaries for re-verify OR same 3 for delta comparison (latter cheaper).

## Reproducibility

- Runner: `.planning/specs/mma-part5/run-step4.js` (204 LOC)
- Context loader reads: DESIGN-SPEC + FINDINGS-STEP1 + `session_end_fallback.rs` full + `ws_handler.rs:2325-2570` + `fleet_health_api.rs:101-220`
- Prompt size: ~15.5K tokens average
- Temperature: 0.1 (deterministic-ish)
- mimo-v2-pro hit 8000 completion cap → truncated mid-D3. Re-run with `max_tokens: 16000` for mimo if its perspective is needed.
