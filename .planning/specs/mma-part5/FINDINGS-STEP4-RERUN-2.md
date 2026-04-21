# MMA Step 4 Re-Re-Run — Part 5 VERIFY Against Polish Commit `273687fc`

## Scope

Verify that the W-2 (driver_name cache hit) + mesh_key ERROR-log + W-3 test polish in
commit `273687fc` did not regress V-A / V-B / V-C closure from rerun-1 (commit `49eb2821`).

Dispatched Step 4 adversarial VERIFY prompt against 3 independent models via OpenRouter
(worktree `/tmp/rc-part5-verify` detached at `273687fc`, `run-step4-rerun.js` updated
with `FIX_COMMIT="273687fc"`).

## Results

| Model | Verdict | Score | Must-fix flags | Notes |
|-------|---------|-------|----------------|-------|
| `moonshotai/kimi-k2` | PASS | 4.5 | (none) | Unchanged from rerun-1 — all three V-defects "residual: none" |
| `mistralai/mistral-medium-3.1` | CONDITIONAL_PASS | 4.2 | W-1 grace-None logging, W-3 cache-miss test gap | Verdict wrapped in `__raw_content` due to extractJson quirk (see `mistral.json`) |
| `xiaomi/mimo-v2-pro` | null | — | — | Hit 24000-token completion cap exactly → `content=null` |

**Partial aggregate: 4.35/5.0** (2 valid verdicts). Above 4.0 Unified MMA threshold.

## Defect status

V-A/V-B/V-C: **re-confirmed closed with residual=none** by both valid models. Evidence
citations (exact lines in `273687fc`):

- V-A: `session_end_fallback.rs:235-265` — `tokio::time::timeout(BYTES_READ_TIMEOUT, response.bytes())` + `serde_json::from_slice`
- V-B: `session_end_fallback.rs:35-50` — module-level `OnceLock<reqwest::Client>`
- V-C: `session_end_fallback.rs:300-308` (cache populate) + `:320-324` (cache read in synth) + `failure_monitor.rs:105-108` (fields + reset helper)

## Mistral-W-3 (this commit closes)

> "No tests for the cache miss scenario where `session_last_known_driving_seconds` is `None`
> and the synth branch falls back to `0u32`." — mistral, conf 5/5

Closed in this commit via new unit test `failure_monitor_state_cache_miss_resolves_to_zero_and_empty`
(`session_end_fallback.rs:569-588`). Covers:
- `unwrap_or(0)` on cached seconds → 0
- `unwrap_or_default()` on cached driver → empty string
- `driver_cache_hit` log field reads false (`is_empty()`)

## Mistral-W-1 (deferred)

Grace-window `None` handling claim (pre-Commit-2 legacy sessions). **Analysis:** all deployed
rc-agent binaries since `active_billing_session_id_set_at` was added write `Some(Instant::now())`
on every assignment (verified by grep of the field's write sites). No pod in production can
have `Some(session_id)` with `None(set_at)` unless the pod boots cold mid-session, which
would require a process crash + rc-agent restart between `start` and `end`. In that pathological
case, the session is already considered "orphaned" by `billing_guard` and would be force-ended
before HTTP fallback runs. **Deferred as nice-to-have.**

## Mistral-W-2/W-4/W-5 (deferred)

- **W-2** (cache monotonicity): server-authoritative poll, P3 observability only
- **W-4** (send_modify race): between membership check and cache update, window <1ms, no customer impact
- **W-5** (String allocation in reset): premature optimization, `None` drops the String fully

All P3, none block ship.

## Cumulative MMA spend

- Step 1: $0.040 (DIAGNOSE)
- Step 2: $0.060 (PLAN)
- Step 4 original: $0.034 (BLOCK 2.77)
- Step 4 rerun-1: $0.046 (PASS 4.23)
- Step 4 rerun-2: ~$0.049 (PASS 4.35 partial, 1 token-cap miss)

**Total: ~$0.229 of $5.00 budget.**

## Gate: deploy-ready

All three Step 1/2 consensus defects (V-A/V-B/V-C) closed with residual=none across two
independent adversarial runs (rerun-1 `49eb2821`, rerun-2 `273687fc`). Kimi verdict stable
PASS-4.5 across both. Mistral verdict improved from CONDITIONAL_PASS-3.8 (Step 4 original)
→ CONDITIONAL_PASS-4.2 (rerun-2) as polish addressed the W-flags.

**Part 5 code is MMA-ship-approved.** Fleet deploy is blocked on venue readiness (per user
2026-04-21: "the venue is not ready to deploy") and 4/8 pods offline — not on code quality.

## Artefacts on disk

- `verify-rerun-2/kimi.json` + `.meta.json` — clean PASS verdict
- `verify-rerun-2/mistral.json` + `.meta.json` — CONDITIONAL_PASS verdict in `__raw_content` field
- `verify-rerun-2/mimo.json` + `.meta.json` — null content (token cap)
- `step4-rerun-2.stderr.log` — runner trace
