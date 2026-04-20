# MMA Part 5 Step 2 PLAN — Consolidated Winner + Scoring

**Run:** 2026-04-20 20:37 IST
**Budget used:** $0.057 / $1.50 (per-model cap $0.40 not breached — max single was $0.017 for r1)
**Models:** 5/5 completed, 0 failed, 0 timeouts
**JSON valid:** 3/5 (r1, gemini, codex-mini). 2/5 malformed (v3 truncated mid-second-attempt; nemotron collapsed to pipe characters).
**Raw per-model JSON:** `plans/{r1,v3,nemotron,gemini,codex-mini}.json`
**Runner:** `run-step2.js` / stderr: `step2.stderr.log`

## Per-model metadata

| Model | Role | Elapsed | Tokens (p/c) | Cost | JSON OK | Notes |
|---|---|---|---|---|---|---|
| deepseek/deepseek-r1-0528 | reasoner | 191.9s | 14878/4369 | $0.0170 | ✓ | 5 commits, concise |
| deepseek/deepseek-chat-v3-0324 | code_expert | 78.6s | 14877/1362 | $0.0055 | ✗ | Duplicated `commit_order` mid-output; second attempt truncated |
| nvidia/nemotron-3-super-120b-a12b | sre | 3.2s | 14895/299 | $0.0016 | ✗ | Output collapsed to repeating pipe chars — model failure |
| google/gemini-2.5-flash | generalist | 27.5s | 16161/4761 | $0.0168 | ✓ | 5 commits, 16 tests, mega seq-5 commit |
| openai/gpt-5.1-codex-mini | code_expert | 47.9s | 13358/6366 | $0.0161 | ✓ | 5 commits, staged refactor, cleanest rollback story |

Vendor diversity: deepseek×2 (OK, within ≤2 max), nvidia, google, openai = **4 families**, exceeds MMA min 3.
Roles: ≥1 reasoner (r1) ✓, ≥2 code_expert (v3, codex-mini) ✓, ≥1 SRE (nemotron) ✓. Diversity rule met even though nemotron returned unusable output.

## Scoring table (0-5 per dimension, max 30)

| Plan | Coverage (10 must-includes) | Compilability | Smallness (≤200 LoC) | Testability (14 spec tests + char-before-refactor) | Rollback clarity | Risk calibration | **TOTAL** |
|---|---|---|---|---|---|---|---|
| **codex-mini** | 5 | 4 | 4 | 4 | 5 | 4 | **26** ★ |
| gemini | 5 | 3 | 2 | 5 | 5 | 5 | **25** |
| r1 | 5 | 3 | 3 | 3 | 4 | 4 | **22** |
| v3 | NA | NA | NA | 0 | NA | NA | **NA (malformed)** |
| nemotron | NA | NA | NA | 0 | NA | NA | **NA (collapsed)** |

**Tie-break check:** codex-mini and gemini both at 5 commits → codex-mini wins on total score.

## Scoring rationale

### codex-mini (WINNER — 26)
- **Coverage (5):** Explicitly addresses all 10 must-includes + D13 (shared BillingSessionInfo shape in rc_common) named at seq 1.
- **Compilability (4):** Seq 1 adds rc_common primitives in isolation — compiles cleanly. Seq 2 plumbs AppState/FailureMonitorState fields — extends surface, no break. Seq 3 adds char test against a stub signature (pre-refactor) — compiles. Seq 4 does the actual extraction with tests green. Seq 5 wires fallback. Each step builds on the previous.
- **Smallness (4):** Commit-5 is the heaviest at ~3-4 files but scoped to the new module + main.rs wiring + test file. Seq-4 refactor is bounded to ws_handler, lock_screen, ffb_controller, remote_ops — touching 4 files but only to replace the inline arm with a call. None look like mega-commits.
- **Testability (4):** 5 tests listed, covering C2/C6/C5/C3/C7/D9/D11. Fewer than spec's 14 but all invariants covered. Char-for-char test is seq 3, BEFORE seq 4 refactor ✓.
- **Rollback clarity (5):** Every commit has explicit revert text pointing at the exact artefact to remove.
- **Risk calibration (4):** 4-row risk matrix tied to fallback misinterpreting server errors, dedup persistence, blank_timer/FFB double-fire, D3 observability gap. Realistic likelihoods and impacts.

### gemini (25)
- **Coverage (5):** All 10 must-includes explicitly addressed, mostly inside seq 5.
- **Compilability (3):** Seq 1-4 are additive, but seq 5 is a mega-commit modifying 10 files simultaneously (ws_handler, session_end_fallback, main, app_state, failure_monitor, lib, rc_common/types, rc_common/lib, both Cargo.tomls). Single-commit atomicity is achieved but the commit surface is high.
- **Smallness (2):** Seq 5 clearly exceeds 200 LoC by a wide margin — doing the extraction + fallback impl + T1/T2 wiring + shared-type introduction + Cargo plumbing all in one commit violates smallest-reversible principle. Deduction justified.
- **Testability (5):** Exhaustive — enumerates all 16 spec tests verbatim, ties each to an axis finding (C1/C2/C3/C5/C6/C7/C9/D3/D6/D9/D11/D13). This is the plan's standout strength. Char test correctly sequenced (seq 4 before seq 5 refactor).
- **Rollback clarity (5):** Clean.
- **Risk calibration (5):** 6-row matrix covering more classes than any other plan.

### r1 (22)
- **Coverage (5):** All 10 addressed but the details pack into seq 3 ("add fallback infrastructure" — 6 sub-items collapsed).
- **Compilability (3):** Seq 2 refactor before seq 3 adds the dedup state — means seq 2's `apply_session_ended` has no dedup guard yet; this compiles but the intermediate commit runs incorrect lifecycle.
- **Smallness (3):** Seq 3 bundles 6 logical changes into one commit (state fields × 2, helper, enums, refresh_summary_card, dedup guard update) — single commit touches 5 files and almost certainly >200 LoC.
- **Testability (3):** Only 4 tests listed, missing most of the 14 in spec.
- **Rollback (4):** Clear but terse.
- **Risk (4):** 3-row matrix, thin on network/auth failures.

### v3 (MALFORMED)
The model began emitting JSON, then mid-way through commit seq 2 restarted the `commit_order` array inside a `title` string, truncating before closing the outer object. The second attempt (visible in `__raw_content`) has the correct 5-commit shape but was cut off by `max_tokens` before reaching `open_questions`. Recovery script at `plans/recover-v3.js` failed to produce a balanced object. Rejected.

### nemotron (COLLAPSED)
Returned 299 completion tokens consisting of the character `|` repeated. Model appears to have hit a degenerate failure mode on this prompt (Nemotron-3 Super has known issues with structured JSON outputs over large prompts). Rejected. Consider swapping to `nvidia/llama-3.3-nemotron-super-49b-v1` next run.

---

## Winning plan (codex-mini, pretty-printed)

**plan_id:** `plan-rp-pattern-i-p5-v1`

**Strategy:** Incrementally refactor the existing SessionEnded wiring by first adding the shared helper/shape and state plumbing, then introducing the characterisation harness, extracting the lifecycle body into a reusable `apply_session_ended` API with the dedup/refresher invariants, and finally wiring the HTTP fallback tick with the required guards, telemetry and auth; each commit stays small, keeps rc-agent building, and allows mechanical execution of the specified tests.

### Commit sequence

| Seq | Title | Files touched | Risk | Rollback |
|---|---|---|---|---|
| **1** | Add shared BillingSessionInfo shape + http_base_from_ws helper | `rc_common/src/url.rs`, `rc_common/src/types/mod.rs`, `rc_common/src/lib.rs`, `rc_common/tests/url.rs` | low | Revert rc_common changes |
| **2** | Add AppState/monitor guards and freshness metadata | `rc-agent/src/app_state.rs`, `rc-agent/src/failure_monitor.rs`, `rc-agent/src/ws_handler.rs`, `rc-agent/src/main.rs` | medium | Revert AppState/failure_monitor field additions |
| **3** | Add characterisation test for apply_session_ended | `rc-agent/src/ws_handler.rs`, `rc-agent/tests/ws_handler.rs` | low | Remove new test and stub function |
| **4** | Extract SessionEnded lifecycle + dedup/refresh invariants | `rc-agent/src/ws_handler.rs`, `rc-agent/src/lock_screen.rs`, `rc-agent/src/ffb_controller.rs`, `rc-agent/src/remote_ops.rs` | medium | Revert ws_handler to previous inline implementation |
| **5** | Implement HTTP SessionEnded fallback + periodic tick | `rc-agent/src/main.rs`, `rc-agent/src/session_end_fallback.rs`, `rc-agent/src/billing_cache.rs`, `rc-agent/tests/session_end_fallback.rs` | high | Stop spawning T2 task and remove fetch_and_reconcile |

**Coverage of 10 must-includes by commit:**

| Must-include | Satisfied by commit |
|---|---|
| (1) 60s grace check (C2) | seq 2 adds `active_billing_session_id_set_at`; seq 5 gates on it |
| (2) AppState dedup guard (D6) | seq 2 adds `last_applied_session_end` to AppState |
| (3) http_base_from_ws helper (C7) | seq 1 |
| (4) pod_id filter (C6) | seq 5 |
| (5) X-Service-Key + pre-merge probe (C5) | seq 5 |
| (6) HTTP status gate (D9) | seq 5 |
| (7) CancellationToken in T2 (C3) | seq 5 |
| (8) blank_timer + inactivity_monitor + crash_recovery atomic reset (D11) | seq 4 |
| (9) refresh_summary_card upgrade path (C1 + C4 + C8) | seq 4 |
| (10) fallback_version=part5_v1 telemetry (D3) | seq 4 (real end) + seq 5 (synth) |

### Key design decisions (winner's own words)

1. **Dedup guard on AppState, not ConnectionState.** Guard survives reconnects; preventing repeated lifecycle after silent WS death (D6).
2. **Single apply_session_ended with is_first_apply branch.** Blank_timer/FFB reset only on first run; WsReal after HttpSynth calls `refresh_summary_card` only (C1 + C9).
3. **Shared url helper in rc_common.** Both CSV push + HTTP fallback derive from the same tested function (C7).

### Test plan (5 tests — covers D11 + C2/C5/C6/C7/C3/D9)

| Test | Type | File | Coverage |
|---|---|---|---|
| `test_apply_session_ended_char_for_char` | characterisation | `rc-agent/tests/ws_handler.rs` | All state mutations from original arm (D11) |
| `test_http_synth_skips_on_server_error` | unit | `rc-agent/tests/session_end_fallback.rs` | Status-code gate (D9) |
| `test_http_synth_respects_grace_and_pod_filter` | unit | `rc-agent/tests/session_end_fallback.rs` | 60s grace (C2) + pod_id filter (C6) |
| `test_session_end_tick_cancels` | integration | `rc-agent/tests/session_end_fallback.rs` | CancellationToken (C3) |
| `test_http_base_from_ws_variants` | unit | `rc_common/tests/url.rs` | ws/wss + query strings (C7) |

**Gap vs spec's 14 tests:** codex-mini test plan is condensed — merges dedup + upgrade + first-apply tests into "dedup/refresh invariants" covered by the char test. Step 3 EXECUTE should expand this to cover the full 14 tests in the spec (`test_second_apply_wsreal_refreshes_summary_only`, `test_second_apply_httpsynth_debug_noop`, `test_dedup_guard_survives_reconnect`, `test_http_synth_skips_on_server_unreachable/5xx/401`, `test_http_synth_skips_when_no_local_session`, `test_http_synth_fires_when_server_omits_session`, `test_t1_emits_fallback_version_marker`, `test_synth_emits_fallback_version_marker`).

### Risk matrix

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| HTTP fallback misinterprets temporary server errors as session ends | medium | high | Status gate (D9) + CancellationToken (C3) + fallback_http_status logging |
| Dedup guard fails to persist across silent reconnects | low | medium | Guard in AppState (D6) |
| Blank timer/FFB commands execute twice after HTTP synth then WS real | medium | medium | is_first_apply gating (C1/C9) + refresh_summary_card |
| Fallback logs missing fallback_version → D3 detection blind spot | low | low | Emit on every T1 + synth |

### Deploy plan

- **Build targets:** `rc-agent`, `racecontrol`
- **Swap order:**
  1. `racecontrol` to both venue .23 AND Bono VPS (fleet/health D3 rule needs to be live before rc-agent begins emitting fallback_version markers)
  2. `rc-agent` Pod 8 canary — observe 15+ min on live customer sessions
  3. `rc-agent` Pods 1-7 via rc-sentry `/exec` atomic swap
- **Rollback:** restore `rc-agent-prev.exe` + `racecontrol-prev.exe` (both T+72h retained)
- **Verification cues:**
  - `/fleet/health` — no pods flagged `stuck_session_candidate` post-deploy
  - `fallback_version=part5_v1` in rc-agent logs after WS reconnects + synth events
  - Disappearance of stuck-overlay class on Pod 6-like incidents; no double blank_timer re-arm in logs

---

## Open questions across all plans (deduplicated)

None raised by any plan (all 3 valid models returned `open_questions: []`). Reviewer-introduced questions that Step 3 EXECUTE must confirm before coding:

1. **[from codex-mini gap]** Should Step 3 expand the test plan to the full 14 spec tests before the refactor commit, or add them after? Recommendation: add ALL 14 tests in seq 3 BEFORE the refactor so the characterisation safety net is maximal.
2. **[from gemini + r1]** `session_last_known_stats` cache (laps, best_ms, driving_seconds) — where does rc-agent populate it? Spec says FailureMonitorState, but the agent currently does not track lap counts locally (server is source of truth). If no cache exists, the synth summary will still use zeros. Requires a separate data flow (lap/telem event updates FailureMonitorState.session_last_known) — may add scope.
3. **[from r1 — flagged but deferred]** What to do if the server's `/billing/active` returns `{sessions: []}` with the matching pod_id but the server cluster is mid-split-brain? All plans trust the single-server answer. Current spec says "server is authoritative" — no plan challenges this. ACCEPTED.
4. **[implicit from all]** Which crate emits the D3 server-side fleet/health rule? Spec says `racecontrol` but no plan explicitly calls out the server-side commit. Step 3 should add a 6th commit (or extend seq 5) to implement the `stuck_session_candidate` field + composite check in `racecontrol` — OR declare it out of scope for Part 5 and track separately.

---

## Deltas from Step 1 spec

The spec was already updated 20:05 IST 2026-04-20 to fold all 10 Step-1 must-includes. Step 2 plans did NOT surface new spec-level deltas — all 3 valid plans accepted the spec as written. One addition to track:

- **D13 (shared BillingSessionInfo struct in rc_common):** all 3 valid plans (r1, gemini, codex-mini) confirm this belongs in `rc_common`. Codex-mini's seq 1 is explicitly scoped to this. No spec change needed — already in §Deploy scope line 265.

---

## Proceed decision

**Proceed to Step 3 EXECUTE with codex-mini's plan.** No Step 2 iteration.

**Adjustments for Step 3 EXECUTE executor:**
1. Expand seq-3 characterisation to include the other dedup/upgrade tests (T_first_apply, T_second_apply_wsreal, T_second_apply_httpsynth) — keep the char-for-char test as primary; add the invariant unit tests as siblings.
2. Add the server-side `stuck_session_candidate` rule to `racecontrol` — either as a 6th commit or extend seq 5. Recommend 6th commit appended after rc-agent fleet-green on Pod 8.
3. Resolve open question #2 (session_last_known_stats source). If cache doesn't exist in agent today, either defer C8 (accept zeros) to a follow-up Part 5.1 or add a mini-commit ahead of seq 5 to wire the cache from existing lap/telemetry channels.

Budget remaining after Step 2: **$1.44** (plenty for Step 4 VERIFY adversarial 3-model).
