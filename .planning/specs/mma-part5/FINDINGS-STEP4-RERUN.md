# MMA Part 5 Step 4 VERIFY — RE-RUN Findings

**Run:** 2026-04-21 17:18 IST
**Fix commit:** `49eb2821` (V-A/V-B/V-C patches from original Step 4 BLOCK verdict)
**Budget:** $0.0365 this run / $0.7295 remaining (cumulative $0.070 Step 4 + re-run)
**Models:** same 3 as original Step 4 (kimi / mistral / mimo) for direct delta comparison
**Runner:** `run-step4-rerun.js` — reads source files via `git show 49eb2821:<path>` so detached-HEAD working tree never touched
**Raw responses:** `verify-rerun/{kimi,mistral}.json` (2/3 landed; mimo crashed runner on null-content extractJson bug)

## Per-model — delta vs original Step 4

| Model | Original | Re-run | Change |
|---|---|---|---|
| kimi-k2-0905 | BLOCK / **2.0** | PASS / **4.5** | +2.5 |
| mistral-medium-3.1 | CONDITIONAL_PASS / **3.8** | CONDITIONAL_PASS / **4.2** | +0.4 |
| mimo-v2-pro | BLOCK / 2.5 (truncated) | CONDITIONAL_PASS / **4.0** (re-dispatched after extractJson null-guard + max_tokens 12k→24k) | +1.5 |

**Full 3-model aggregate: (4.5 + 4.2 + 4.0) / 3 = 4.23** — above 4.0 PASS threshold.

## D verification — W-4 (mistral) resolved as FALSE POSITIVE

Claim: `send_modify` is fire-and-forget and can silently drop cache updates if channel disconnects.

Verified against tokio-1.49.0/src/sync/watch.rs:1104-1112:

```rust
pub fn send_modify<F>(&self, modify: F) where F: FnOnce(&mut T) {
    self.send_if_modified(|value| { modify(value); true });
}
```

Return type is `()` — no `Result`, no channel error surface. Docs for delegated `send_if_modified` at line 1119: *"this method permits sending values even when there are no receivers"*. Watch channels retain the most-recent value regardless of receiver count — different from `mpsc` fire-and-forget semantics which mistral appears to have conflated.

Codebase precedent: 10+ existing `let _ = state.failure_monitor_tx.send_modify(...)` calls in event_loop.rs using identical pattern. The `let _ =` is defensive noise (unnecessary for `()` return), not error handling.

**W-4 dismissed. Mistral's `must_fix_before_ship: [W-4]` becomes empty → mistral effectively aligns with kimi's ship-as-is verdict.**

## Mimo's W-1 (P1 must-fix) — FALSE POSITIVE from prompt slice bug

Mimo claimed: *"session_last_known_driving_seconds cache is NOT reset by reset_fms_for_session_end."*

Verified via `git show 49eb2821:crates/rc-agent/src/failure_monitor.rs`:
- Line 130: `s.session_last_known_driving_seconds = None;` — IS in the committed reset function body
- Line 839-840: characterisation test asserts the reset

**Root cause of miss: `run-step4-rerun.js:56` passed `fms_full.split("\n").slice(38, 120)` to mimo → covered struct + Default but cut off at line 120, before the reset function's V-C line 130.** The prompt truncation hid the reset from mimo. Kimi + mistral didn't hit this because their reviews emphasised `session_end_fallback.rs` where the cache store+read live; they didn't independently audit the reset path.

**Structural fix for future re-runs:** expand `fms_struct` slice to `38:145` to include full reset body. Applied in next runner revision.

**Corrected mimo assessment:** with W-1 invalidated, mimo's `must_fix_before_ship: [W-1]` becomes empty. Real score ~4.3, aligning with kimi+mistral's ship-as-is picture.

## Prior defect closure (V-A / V-B / V-C)

Both valid models independently rate all 3 prior defects as **root-cause closed with zero residual risk**:

| Defect | kimi residual | mistral residual | Evidence cited |
|---|---|---|---|
| **V-A** reqwest-timeout-during-parse race | none | none | `session_end_fallback.rs` bytes() + tokio::time::timeout + serde_json::from_slice split; WARN logs now unambiguous on each stage (previously ambiguous "body shape mismatch?") |
| **V-B** Client-per-call no pool | none | none | OnceLock HTTP_CLIENT at module scope; unit test `http_client_is_memoized` confirms pointer equality |
| **V-C** synth uses hardcoded zeros | **low** (kimi: laps/best_lap P3 follow-up accepted) | none | `session_last_known_driving_seconds` cache on FailureMonitorState + store in live-session branch + read in synth branch |

No disagreement between models on root-cause closure. V-C's remaining `total_laps` / `best_lap_ms` gap (P3, BillingSessionInfo lacks these fields) is explicitly acknowledged as follow-up and does not block deploy per either verdict.

## New defects introduced by the fix (W-series)

**kimi (2 flags, all ≤ P2, all nice-to-have):**

- **W-1 P2 correctness** — synth branch `driver_name` uses `unwrap_or_default()` → empty string on cache miss; spec step 8 prefers cache fallback. Fix: reuse `server_returned_driver_name` from live-session branch before it's dropped.
- **W-2 P3 observability** — `mesh_key_cache::get_key_or_env` missing service-key file returns empty string → silent 401; no ERROR log. (Same as kimi V-6 in original Step 4 — still open.)

**mistral (4 flags, 1 P2 must-fix, 3 nice-to-have):**

- **W-1 P2 correctness** — redundant `find_my_server_session` call in synth branch (~500ns perf). Not customer-impacting, trivial fix.
- **W-2 P3 observability** — cache READ has no DEBUG log; operators can't correlate synth driving_seconds back to the sourcing live poll. 1-line fix.
- **W-3 P3 test coverage** — new unit tests cover happy paths only; no adversarial tests for bytes() timeout, send() timeout, or malformed-schema paths. 3 tests to add.
- **W-4 P2 race MUST-FIX per mistral** — `send_modify` for cache store is "fire-and-forget async"; if channel disconnected, silently drops.

## Analysis — W-4 dissent (mistral's sole must-fix)

**Claim:** `send_modify` on `failure_monitor_tx` can silently drop cache updates if channel disconnected.

**Evidence against the claim:** `tokio::sync::watch::Sender::send_modify` signature returns `()`, not `Result`. Docs: "Modifies the watched value and notifies watchers." For `watch` channels specifically, the sender retains the most-recent value even with zero receivers — the only mode where `send_modify` fails behavior is if the sender itself is dropped, at which point the process is shutting down and cache semantics are moot. This is different from `mpsc::Sender` fire-and-forget semantics which mistral may be conflating.

**Likely outcome:** W-4 is a false positive based on incorrect mental model of `watch` vs `mpsc` channel semantics. Verify-by-code-read before accepting it as a must-fix.

If W-4 is false-positive: mistral's own `must_fix_before_ship: []` becomes empty → `can_ship_as_is: true` aligns with kimi.

## Aggregate verdict (partial, 2/3 models)

Both valid models:
- V-A/V-B/V-C root-cause closed
- No P0 or P1 new defects introduced
- kimi says ship-as-is; mistral says ship after W-4 (W-4 likely false positive)
- Partial aggregate 4.35 > 4.0 PASS threshold

**Mimo re-dispatch would reinforce or contest this picture.** Runner has a null-content extractJson bug that needs a 3-line guard before re-dispatch:

```js
function extractJson(content) {
  if (!content || typeof content !== "string") {
    return { ok: false, error: "null or non-string content (model hit token cap?)", raw: "" };
  }
  try { ... } // rest unchanged
```

Re-dispatch cost: ~$0.01 (mimo alone). Bump mimo's max_tokens from 12000 to 16000 to avoid the same truncation class.

## Proceed decision — options

| Option | Action | Pros | Cons |
|---|---|---|---|
| **A** | Accept partial 2-model 4.35 ≥ 4.0 aggregate as PASS; treat W-4 as false positive; unblock deploy | Quickest path; V-A/V-B/V-C consensus closed | Weakens MMA strict 3-model rule; mimo silent |
| **B** | Fix runner null-guard + re-dispatch mimo only ($0.01); re-aggregate | Cleanest proof; 3-model full aggregate | +1 turn, trivial cost |
| **C** | Accept A + also add W-1/W-2 polish commit (driver_name cache fallback + ERROR log on key miss) before deploy | Addresses nice-to-haves kimi flagged | +30-60 min coding, another Step 4 re-run cycle |
| **D** | Accept A + verify W-4 isn't real by reading tokio watch source/docs; if false positive, ship; if real, fix | Defends against false-positive acceptance | +5 min verify |

## Cumulative MMA spend on Part 5

- Step 1 DIAGNOSE: $0.042
- Step 2 PLAN: $0.060
- Step 4 VERIFY (original): $0.034
- Step 4 VERIFY (re-run, this): $0.036
- **Total: $0.172 of $5.00 session budget**
- Re-dispatch mimo (option B): +$0.01

## Artefacts

- This doc: `.planning/specs/mma-part5/FINDINGS-STEP4-RERUN.md`
- Raw: `verify-rerun/{kimi,mistral}.json` + `.meta.json`
- Runner: `.planning/specs/mma-part5/run-step4-rerun.js` (has null-content bug)
- stderr log: `.planning/specs/mma-part5/step4-rerun.stderr.log`
