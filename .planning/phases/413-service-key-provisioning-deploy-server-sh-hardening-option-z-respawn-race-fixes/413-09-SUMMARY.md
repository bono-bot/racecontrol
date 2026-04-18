---
phase: 413-service-key-provisioning-deploy-server-sh-hardening-option-z-respawn-race-fixes
plan: "09"
subsystem: audit/mma
tags: [mma, phase-413, option-z, cross-system-bridge, audit-trail, consensus-4.00]
dependency-graph:
  requires:
    - 413-01, 413-02, 413-03, 413-04, 413-05, 413-06, 413-07 (the diff under review)
    - data/openrouter-mma-key.txt (OpenRouter provisioned key)
    - scripts/lib/openrouter-key-recovery.js (auto-recovery path — not triggered this session)
  provides:
    - .planning/phases/413-.../413-MMA-AUDIT.md (full audit trail)
    - 5 new tests across 2 crates (empty-key + whitespace guard tests)
    - 2 new code fixes (commits ac9cb838 + 2c530fc4)
    - LOGBOOK.md row with 11 models + 3 scores + ~$0.05 cost
  affects:
    - Plan 10 (integration test) — MMA gate PASSED, cleared to proceed
    - Plan 11 (fleet deploy) — remains gated on Plan 10 success
tech-stack:
  added: []
  patterns:
    - "Multi-Model Audit v3.0 with DIAGNOSE -> EXECUTE -> VERIFY -> iterate loop per UNIFIED-MMA-PROTOCOL.md"
    - "3-round audit pattern: DIAGNOSE 5-wide, VERIFY 3-adversarial (fresh vendors), repeat if score < 4.0"
    - "Per-round fresh vendor families (DIAGNOSE 4 -> VERIFY-1 3 disjoint -> VERIFY-2 3 disjoint-from-both = 10 unique families cumulative)"
    - "Parallel OpenRouter curl invocations via bash background jobs + wait (no Node runner needed)"
    - "JSON-output-only prompt shape for mechanical consensus extraction"
key-files:
  created:
    - .planning/phases/413-.../413-MMA-AUDIT.md (full report, 300+ lines)
    - .planning/phases/413-.../audit-prompt.md (DIAGNOSE prompt)
    - .planning/phases/413-.../verify-prompt.md (VERIFY-1 prompt)
    - .planning/phases/413-.../verify2-prompt.md (VERIFY-2 prompt)
    - .planning/phases/413-.../run-diagnose.sh, run-verify.sh, run-verify2.sh (runners)
    - .planning/phases/413-.../parse-diagnose.js (consensus extraction)
    - 11 raw OpenRouter response JSONs + 3 parsed-combined JSONs
  modified:
    - LOGBOOK.md (1 row appended)
    - crates/racecontrol/src/api/mesh_intelligence.rs (C-2 + NEW-1 fixes + 5 tests)
    - crates/rc-agent/src/csv_lap_fallback.rs (C-5 AUTH REJECTED warn branch)
    - crates/rc-agent/src/mesh_key_cache.rs (503 regression test added)
decisions:
  - "Gemini 2.5 Pro BANNED per CLAUDE.md cost rule — used Flash variant in DIAGNOSE"
  - "Moonshot kimi-k2-5 in plan spec -> used kimi-k2.5 (OpenRouter ID with dot not dash)"
  - "Nemotron 3 Super (120b-a12b) chosen over plan's llama-3.1-nemotron-70b-instruct per availability + SRE-role alignment"
  - "VERIFY-1 returned 2.33 avg -> iterated per UNIFIED-MMA-PROTOCOL.md instead of deferring; VERIFY-2 used 3 NEW vendor families (xai, meta, mistral) fresh from all prior rounds"
  - "Applied NEW-1 fix (whitespace bypass) as Rule 2 (critical correctness) — caught by VERIFY, missed by DIAGNOSE; textbook case for multi-round adversarial review"
  - "C-1 (IP-auth boundary) accepted as documented risk, NOT Phase 413 code fix — same trust pattern as /config/kiosk-allowlist + /guard/whitelist/{N}; mTLS upgrade is Rule 4 milestone scope"
  - "Kimi-K2.5 exhausted 4000-token reasoning budget on first VERIFY-1 call and returned empty content — rerun with max_tokens=12000 + reasoning.max_tokens=8000 to capture final JSON"
metrics:
  duration_seconds: 3120
  duration_human: "~52m"
  tasks_completed: 3
  tasks_total: 3
  rounds: 3
  models_unique: 11
  vendor_families_cumulative: 10
  tokens_total: 43519
  cost_estimated_usd: 0.05
  budget_usd: 5
  final_score: 4.00
  gate: "PASS"
  fixes_applied: 3
  tests_added: 5
  completed_date: 2026-04-18
requirements-completed: []
---

# Phase 413 Plan 09: MMA Audit Summary

Three-round Multi-Model Audit of Phase 413 (Plans 01-07) cleared the deploy gate at **4.00/5.0** consensus with 3/3 SHIP recommendations from fresh adversarial reviewers.

## One-liner

11 models across 10 vendor families reviewed the Option-Z mesh-service-key bridge + deploy-server.sh 3-factor fixes; DIAGNOSE flagged 2 HIGH + 3 MEDIUM concerns, VERIFY caught 1 additional HIGH (whitespace bypass) that DIAGNOSE missed, all fixable concerns were fixed, final consensus is PROCEED.

## What Was Done

### Round 1 — DIAGNOSE (5 models, 4 vendor families)

Ran in parallel on OpenRouter:

- `deepseek/deepseek-r1-0528` (reasoner, thinking)
- `deepseek/deepseek-chat-v3-0324` (code expert, non-thinking)
- `xiaomi/mimo-v2-pro` (SRE)
- `qwen/qwen3-235b-a22b-2507` (generalist)
- `google/gemini-2.5-flash` (generalist — Flash NOT Pro, cost-compliant)

**Result: avg 3.10/5.0** — below 4.0 threshold. 2 HIGH-consensus findings:
- **C-1** (Q1, 4/5): IP-based `classify_ip` spoofable from customer WiFi on 192.168.31.*
- **C-2** (Q2, 3/5): Server handler returns 200+empty when `pods.sentry_service_key` is empty -> rc-agent caches overwrite to None fleet-wide

Plus 3 MEDIUM (WMIC deprecation 5/5, boot-race 4/5, csv_lap_fallback obs gap 3/5).

### EXECUTE Round 1 (commit `ac9cb838`)

Fixed C-2 + C-5:

1. `crates/racecontrol/src/api/mesh_intelligence.rs`: handler returns `503 SERVICE_UNAVAILABLE` + `tracing::error!` when key is empty/unset. rc-agent `fetch_from_server` sees `error_for_status()?` -> Err, preserves last-known-good.
2. `crates/rc-agent/src/csv_lap_fallback.rs`: distinct `tracing::warn!` branch on 401/403 with `"AUTH REJECTED"` message, matching the W5 pattern already in `ai_debugger` + `mesh_key_cache`.
3. Added 3 tests: `mma_c2_empty_toml_key_does_not_serve`, `mma_c2_non_empty_toml_key_serves` (racecontrol), `fetch_preserves_last_known_good_on_503` (rc-agent).

All 11 mesh_key_cache tests + 5 phase413 tests pass. Release builds clean.

### Round 2 — VERIFY (3 models, 3 fresh vendor families)

Adversarial review with models NOT in DIAGNOSE: `moonshotai/kimi-k2.5` (Moonshot), `nvidia/nemotron-3-super-120b-a12b` (Nvidia), `openai/gpt-5.4-nano` (OpenAI). Fully disjoint from DeepSeek/Xiaomi/Qwen/Google.

**Result: avg 2.33/5.0** — STILL below threshold. 2/3 models caught a new HIGH severity issue that DIAGNOSE missed:

- **NEW-1** (Kimi + GPT-5.4-nano): The C-2 fix used `!k.is_empty()` which accepts `" "` (single space) as a "non-empty" key. A fat-finger whitespace value in TOML would be served 200 OK, cached as "valid", and then auth would fail 401 fleet-wide — **same silent-outage class as C-2, NOT closed by the C-2 fix.**

Nemotron scored 2.0 but did not catch NEW-1 — it re-raised the C-1 severity only.

### EXECUTE Round 2 (commit `2c530fc4`)

Applied NEW-1 fix:

- `crates/racecontrol/src/api/mesh_intelligence.rs`: `!k.is_empty()` -> `!k.trim().is_empty()`. Now blocks `" "`, `"   "`, `"\t"`, `"\n"`, `" \t\n "`. **Does NOT trim the served value** — real keys with accidental surrounding whitespace are still served as-is (intentional: different bug class, don't risk breaking currently-working keys).
- Added 2 tests: `mma_verify_new1_whitespace_key_does_not_serve` (5 whitespace variants), `mma_verify_new1_whitespace_surrounding_real_key_still_serves` (guard against over-aggressive trim).

7/7 phase413 tests pass.

### Round 3 — VERIFY-2 (3 models, 3 new vendor families)

Because VERIFY-1 returned 2.33 and two FIX_BLOCKING recommendations, iterated with yet another fresh set of vendors: `x-ai/grok-4.1-fast` (xAI), `meta-llama/llama-4-maverick` (Meta), `mistralai/mistral-large-2512` (Mistral). None of these had participated in DIAGNOSE or VERIFY-1.

**Result: avg 4.00/5.0 — AT THRESHOLD. 3/3 SHIP. No new HIGH concerns.**

All 3 models:
- Confirmed whitespace fix is ADEQUATE.
- Accepted C-1 as PARTIAL-defensible after full mitigation disclosure (IP-gating matches existing trust pattern; constant-time ct_eq on rc-agent /exec is defense-in-depth; key grants only pod-level ops).
- Returned zero new HIGH concerns.

## Final Consensus Decision Table

| Concern | Severity | Models flagging | Status | Final action |
|---------|----------|-----------------|--------|--------------|
| C-1 | HIGH | 4+1 (DIAGNOSE + Kimi VERIFY-1) | ACCEPTED_RISK | Documented; matches existing trust pattern; mTLS = future phase |
| C-2 | HIGH | 3/5 DIAGNOSE | FIXED | 503 + error-log; cache preserves LKG |
| NEW-1 | HIGH | 2/3 VERIFY-1 | FIXED | .trim().is_empty() + 2 tests |
| C-3 | MEDIUM | 5/5 DIAGNOSE | DEFERRED | Plan 10 live-verify WMIC; 24H2 = future ops |
| C-4 | MEDIUM | 4/5 DIAGNOSE | ACCEPTED_RISK | 300s self-heal matches boot-resilience standard |
| C-5 | MEDIUM | 3/5 DIAGNOSE | FIXED | AUTH REJECTED distinct warn log |
| C-6 | LOW | 2/5 DIAGNOSE | DEFERRED | Plan 10 runtime verification |

## Tasks Completed

| Task | Commit | Artifact |
|------|--------|----------|
| 1 (checkpoint:decision model selection) | n/a | Auto-approved per Auto Mode — accepted plan's option-a model selection with OpenRouter-ID corrections (kimi-k2.5 dot, nemotron-3-super, gpt-5.4-nano) |
| 2 (audit run) | `ac9cb838`, `2c530fc4`, `fc931b02` | 413-MMA-AUDIT.md + 23 audit artifacts (prompts, runners, parsers, 11 raw JSON responses, 3 combined-findings JSONs) + 3 code fixes + 5 new tests |
| 3 (LOGBOOK entry) | `fc931b02` | Single consolidated row with all 11 models, 3 scores (3.10 -> 2.33 -> 4.00), cost (~$0.05), and gate result (PASS) |

Task 1 (decision checkpoint) was auto-approved per Auto Mode instead of stopping. The plan's model selection was accepted with 3 corrections for OpenRouter ID availability:

- `moonshotai/kimi-k2-5` -> `moonshotai/kimi-k2.5` (dot, not dash)
- `nvidia/llama-3.1-nemotron-70b-instruct` -> `nvidia/nemotron-3-super-120b-a12b` (the 70b Llama-based variant was not in the OpenRouter registry; the 120b Nemotron-3-Super is and fills the same SRE role)
- `openai/gpt-5.4-nano` -> confirmed available on OpenRouter

## Verification Results

```
cargo test -p racecontrol-crate --lib phase413_tests       -> 7 passed
cargo test -p rc-agent-crate --bin rc-agent mesh_key_cache -> 11 passed (incl. new 503 test)
cargo test -p rc-agent-crate --bin rc-agent csv_lap_fallback -> 7 passed
cargo build --release --bin racecontrol                    -> clean (1 pre-existing warning)
cargo build --release --bin rc-agent                       -> clean (99 pre-existing warnings)
```

### Acceptance-criteria grep counts

```
grep -c "consensus" 413-MMA-AUDIT.md                   -> 6 (well > 0 required)
wc -l 413-MMA-AUDIT.md                                 -> 300+ (well > 80 required)
grep -c "Phase 413" LOGBOOK.md                          -> 2 (existing plan row + new MMA row)
grep -c "MMA-manual" LOGBOOK.md                         -> 1 (this session's row)
```

All required audit sections present (Model Selection, DIAGNOSE outputs, Consensus Findings, EXECUTE, VERIFY, Final Score). VERIFY section contains 3 DIFFERENT models NOT in DIAGNOSE. Gemini Flash appears in DIAGNOSE only (not VERIFY) — W6 rule honored.

## Deviations from Plan

### Rule 2 — Auto-add missing critical functionality (whitespace bypass)

**Found during:** VERIFY-1 analysis.
**Issue:** DIAGNOSE-consensus C-2 fix used `!k.is_empty()` predicate. Kimi K2.5 + GPT-5.4-nano independently caught that this accepts `" "` and reintroduces the silent-outage class.
**Fix:** Changed to `!k.trim().is_empty()` + 2 new tests. Files: `crates/racecontrol/src/api/mesh_intelligence.rs`.
**Commit:** `2c530fc4`.

This is the single most important outcome of the audit — a real HIGH-severity bug caught by adversarial VERIFY that DIAGNOSE missed. Evidence for the UNIFIED-MMA-PROTOCOL.md requirement that VERIFY use DIFFERENT models from DIAGNOSE: a second DIAGNOSE pass with the same 5 models would have suffered self-review blindness and likely missed it.

### Rule 3 — Blocking: VERIFY-1 returned 2.33/5 below threshold, plan allowed for iteration

**Found during:** VERIFY-1 parse.
**Issue:** Two FIX_BLOCKING and one DEFER recommendation; avg 2.33/5.0.
**Fix:** Applied NEW-1 code fix AND iterated with VERIFY-2 using 3 fresh vendor families (xAI, Meta, Mistral). Per UNIFIED-MMA-PROTOCOL.md Step 4, this is the prescribed path.
**Commit:** `2c530fc4` (NEW-1 fix) + VERIFY-2 artifacts in `fc931b02`.

### Rule 3 — Blocking: Kimi K2.5 exhausted reasoning budget

**Found during:** VERIFY-1 parse.
**Issue:** First Kimi call truncated with `finish_reason: "length"`, content field empty — all response tokens consumed by the thinking-track buffer.
**Fix:** Reran Kimi alone with `max_tokens=12000` + explicit `reasoning.max_tokens=8000`. Captured full JSON response on retry.
**Impact:** No audit quality degradation — the reasoning-track output shown in the partial response matched the final JSON answer. Documented so future operators know to bump the budget for Moonshot thinking-mode models.

### Task 1 auto-approval

Per Auto Mode directive + `references/checkpoints.md` checkpoint:decision behavior, the plan's option-a model selection was auto-accepted with the 3 OpenRouter-ID corrections listed above. Log line: `Auto-selected: option-a (proposed 5+3 model selection with OpenRouter-ID adjustments for availability)`.

## Authentication Gates

None. OpenRouter key was pre-provisioned at `data/openrouter-mma-key.txt` (74 bytes, `sk-or-v1-620494...`). No 401 recovery triggered.

## Known Stubs

None. The MMA audit is a deliverable with full content; no placeholder data; every claim in 413-MMA-AUDIT.md traces to a raw JSON response committed to disk.

## Deferred Issues (out of scope of this plan)

| Item | Reason | Future action |
|------|--------|---------------|
| C-1 mTLS upgrade | Rule 4 architectural change spanning LAN-auth layer | v51.0+ milestone planning |
| C-3 WMIC replacement for Win11 24H2+ | Server .23 currently pre-24H2; future ops phase | Trigger on OS migration |
| C-4 boot-race eliminated (vs 300s self-heal) | Matches existing boot-resilience pattern (CLAUDE.md) | Reconsider if customer-visible outage occurs |
| C-6 periodic_refetch timer integration test | Unit test via tokio::time::pause adds dev-dep overhead | Plan 10 live-runtime observation |

## Ready for Plan 10

- Contract PROVEN: `GET /api/v1/pods/mesh-service-key` returns `{"mesh_service_key": "<key>"}` when key is configured, `503` otherwise.
- Cache preservation PROVEN: rc-agent cache survives all 4xx/5xx/network-error paths (11 unit tests).
- csv_lap_fallback auth observability PROVEN: distinct `AUTH REJECTED` warn log on 401/403 (grepable).
- **MMA gate PASSED**. Plan 10 integration test + live-fleet deploy on one canary pod may proceed.

## Deployment (Manifest per CLAUDE.md DMP)

- `rust_binary`: racecontrol + rc-agent — release builds verified locally (this plan). NOT DEPLOYED — Plan 10 handles deploy + live verification.
- `frontend_rebuild`: none
- `config_change`: none
- `db_migration`: none
- `infrastructure`: none
- `data_files`: none
- `bat_file`: none
- `cloud_parity`: none (this plan is audit+docs only)
- `targets`: none (the audit is a review of Plans 01-07's output; it does not itself deploy anything)

Plan 10 is the first plan that deploys binaries. Plan 11 rolls fleet-wide.

## Self-Check: PASSED

- [x] `.planning/phases/413-.../413-MMA-AUDIT.md` present (300+ lines, well above 80-line min)
- [x] `grep -c "consensus" 413-MMA-AUDIT.md` returns 6 (>= 1 required)
- [x] 5 unique DIAGNOSE models present in document (deepseek-r1, deepseek-v3, mimo-v2-pro, qwen3-235b, gemini-flash)
- [x] 4 vendor families in DIAGNOSE (DeepSeek ×2, Xiaomi, Qwen, Google) - exceeds 3-vendor minimum
- [x] VERIFY section contains 3 DIFFERENT models NOT in DIAGNOSE (Kimi, Nemotron, GPT-5.4-nano)
- [x] No Gemini Flash in VERIFY section (W6 rule)
- [x] Final score documented numerically ("4.00 / 5.0 — PROCEED")
- [x] All HIGH findings either APPLIED (C-2, NEW-1) or DEFERRED with rationale (C-1 architectural)
- [x] LOGBOOK.md row appended, grep -c "MMA-manual" returns 1 for this session
- [x] 3 commits traceable in `git log --oneline -5` (ac9cb838, 2c530fc4, fc931b02)
- [x] Plan-specified acceptance command verified: `test -f 413-MMA-AUDIT.md && grep -c "consensus" ...` = 6 (> 0)
- [x] `cargo build --release` clean for both binaries
- [x] All 5 new tests + 11 mesh_key_cache tests + 7 phase413 tests + 7 csv_lap_fallback tests pass
