# MMA Step 4 VERIFY — adversarial gate on CF-1+CF-2 bundle PLAN — VERDICT: BLOCK

**Captain-authorized**: Recommended sequencing per session_handoff_20260509_deploy_mechanism_rca_step2_plan.md §11 option 2 + Captain "go" verb 2026-05-09 ~20:18 IST
**Consumes**: `MMA-DEPLOY-RCA-STEP2/CONSENSUS-PLAN.md` (CF-1+CF-2 bundle, 7 actions, 5 Q-DECISIONs)
**Date**: 2026-05-09 ~20:24 IST
**Models**: 3 vendor-disjoint from Steps 1+2 (anthropic, openai, nvidia attempted)
**Wall time**: 97.7s (sonnet longest; nano cleared in 13.7s; nemotron failed at 1.4s)
**Cost**: $0.0728 / $5 budget
**Cumulative MMA-day spend**: ~$0.566 / $5

---

## §1 — Verdict

**OVERALL VERDICT: BLOCK** (overall score average 2.12 across 2 valid models; PASS threshold = 4.0)

The PLAN does NOT pass Step 4 VERIFY. Recommended sequencing halts here. PR authoring blocked pending PLAN amendments OR pivot to alternative architecture.

| Model | Vendor | Role | Overall Score | Verdict | Notes |
|---|---|---|---|---|---|
| **openai/gpt-5.4-nano** | openai | reasoner | **1.9** | **BLOCK** | finish=stop; clean JSON; complete |
| **anthropic/claude-sonnet-4.6** | anthropic | code-expert | **2.33** | (implied BLOCK; truncated before verdict field) | finish=length @ 4000 tokens; scores + 8 flaws + 5 amendments captured; verdict field cut off mid-FL-8 |
| **nvidia/llama-3.1-nemotron-70b** | nvidia | sre | **API_404** | — | "No endpoints found for nvidia/llama-3.1-nemotron-70b-instruct" — model not available on OpenRouter |

**Why no third-model re-run**: Even with a perfect 5.0 from a substitute 3rd model, arithmetic max overall = (1.9 + 2.33 + 5.0) / 3 = **3.08** — still below 4.0 PASS threshold. BLOCK verdict cannot be flipped by additional models. Re-run deferred unless Captain explicitly requests.

---

## §2 — Per-dimension scores

| Dimension | gpt-5.4-nano | sonnet-4.6 | Average | Notes |
|---|---|---|---|---|
| Correctness | 2.0 | 3.0 | 2.5 | Race not eliminated, only probabilistically mitigated; sonnet adds timing analysis |
| Risk coverage | 1.5 | 2.0 | 1.75 | Both flag FL-1 silent-fleet-death; rollback plan thin |
| Backward compatibility | 2.0 | 2.5 | 2.25 | Both flag old-watchdog-on-Pod-8 reads JSON via is_file() → suppresses indefinitely |
| Test plan adequacy | 2.0 | 2.0 | 2.0 | T5 doesn't test the original failing scenario; no race-injection tests |
| Concreteness | 2.0 | 2.5 | 2.25 | A2 JSON parse fail behavior unspecified; A6 bash portability gap; CF12-Q4 file missing |
| Independence from anchoring | 1.5 | 2.0 | 1.75 | Both flag CLAUDE.md "Remote deploy sequence" anchored Step 2 toward `single_exec_chain` |
| **Overall** | **1.9** | **2.33** | **2.12** | **BLOCK** (< 4.0 PASS threshold) |

---

## §3 — Convergent flaws (both models)

### FL-CONV-1 (P0) — Sentinel-before-chain ordering creates silent fleet death

**Both models flag**. A6 writes `OTA_DEPLOYING` JSON sentinel BEFORE A5 executes the atomic chain. If the deploy script crashes, `/exec` call to rc-sentry times out, network drops, or any failure between A6 and A5:
- State: sentinel present (suppressing rollback) + rc-agent.exe absent or in old state + rc-agent-new.exe not swapped
- Watchdog observes rc-agent unhealthy but rollback is suppressed
- **Pod is silently dead for up to 300s (TTL) before sentinel auto-clears**
- With 7 pods to deploy, this failure mode has 7 opportunities to fire

**Sonnet recommendation**: Two-phase sentinel — write sentinel, execute chain, on chain failure immediately delete sentinel and surface error. Parse `/exec` response body for "SWAPPED" success marker before considering deploy successful. Strongest argument for gemini's `new_atomic_endpoint`.

**Nano recommendation**: Add deploy-side barrier OR implement deferred CF-9 `/exec_atomic_deploy` server-side endpoint.

### FL-CONV-2 (P0) — Pod 8 OLD rc-watchdog suppresses indefinitely on JSON sentinel

**Both models flag**. Pods on OLD rc-watchdog (Pod 8 canary; potentially Pods 1-7 if `deploy-watchdog.sh` not created+deployed first) use bare `is_file()` check. JSON file exists → `is_file()` returns true → rollback suppressed forever (no TTL). The mtime fallback in A2 lives in NEW code; OLD watchdog has no TTL enforcement at all.

This is the **original CF-2 bug in inverse form**: instead of rollback firing when it shouldn't, rollback is suppressed when it should fire.

**Both models recommend**: CF12-Q4 (`scripts/deploy-watchdog.sh`) must be a **BLOCKING dependency**, not "open question." deploy-watchdog.sh must be created and rc-watchdog must be deployed to ALL pods (including Pod 8) BEFORE deploy-pod.sh JSON sentinel change is rolled out. Alternative: gate JSON sentinel format change behind a feature flag until all pods are on new rc-watchdog.

### FL-CONV-3 (P1) — A2 JSON parse failure behavior unspecified

**Both models flag**. When `auto_clear_ota_deploying_json()` reads the sentinel file and JSON parsing fails (corrupted write, partial write during crash, literal `$(date +%s)` from cmd.exe execution), the code must decide: treat as "sentinel active" (suppress rollback) or treat as "no sentinel" (allow rollback). Either default has dangerous edge cases. PLAN specifies neither.

**Sonnet recommendation**: On JSON parse failure, log WARNING and fall back to mtime of the file itself (same as legacy bare-file path). Add unit test `test_auto_clear_ota_json_corrupted_mtime_fallback` to T1-T3.

### FL-CONV-4 (P1) — Race window probabilistically mitigated, not eliminated

**Both models flag**. PLAN asserts single `&` chain "wins the race" but provides no timing analysis:
- taskkill (may wait for graceful shutdown, 1-3s if rc-agent has open handles)
- AV scanning may delay file ops
- Watchdog polls every 5-10s — race window <1s in normal conditions but not guaranteed
- No minimum `timeout_ms` specified for `/exec` curl payload

**Recommendation**: Add timing analysis. Specify `timeout_ms >= 10000` in `/exec` payload. Document that JSON sentinel (CF-2) is the primary correctness guarantee, not chain speed.

### FL-CONV-5 (P1) — rc-watchdog deploy has no self-recovery if `sc start` fails

**Both models flag**. Rollback procedure and CF12-Q4 both involve `sc stop / copy / sc start RCWatchdog`. If `sc start` fails (binary corrupt, DLL missing, service account issue), pod has no watchdog, rc-agent can crash and never be restarted. No watchdog-of-the-watchdog. PLAN lists sequence without failure handling.

**Sonnet recommendation**: Post-start health check — poll `sc query RCWatchdog` for RUNNING state with 30s timeout. If not RUNNING, alert and do NOT proceed with rc-agent deploy. Document Windows Service Recovery settings.

---

## §4 — Sonnet-only flaws (single-model signal, P1-P2)

### FL-SONNET-5 (P1) — A6 uses bash `$(date +%s)` — Windows portability not guaranteed

`echo {"timestamp_epoch":$(date +%s)} > C:\RacingPoint\OTA_DEPLOYING` works in Git Bash but writes literal `$(date +%s)` if executed via cmd.exe/PowerShell. PLAN does not specify required execution environment.

**Recommendation**: Add shebang assertion (`if [ -z "$BASH_VERSION" ]; then exit 1; fi`) OR have rc-sentry write sentinel server-side (again pointing toward `new_atomic_endpoint`).

### FL-SONNET-7 (P2) — T5 chicken-and-egg with new rc-watchdog deploy

T5 tests TTL behavior (requires NEW rc-watchdog) on Pod 1 (currently OLD rc-watchdog). To deploy new rc-watchdog to Pod 1, deploy-watchdog.sh must exist (CF12-Q4 open). T5 cannot execute as written until CF12-Q4 resolved.

**Recommendation**: T5 pre-conditions = (1) create deploy-watchdog.sh, (2) deploy new rc-watchdog to Pod 1, (3) verify RCWatchdog service running new binary, (4) THEN run T5.

### FL-SONNET-8 (P2, partial-truncated) — A5 chain `&` doesn't fail-fast

If any `ren`/`del` fails (file locked, antivirus, permission), chain continues (because `&` continues regardless). Can leave rc-agent-new absent or rc-agent-prev deleted, breaking rollback.

**Recommendation**: Use `&&` with explicit error handling, OR transactional rename scheme (rename to temp names first, verify, swap), OR PowerShell script with structured checks.

---

## §5 — Nano-only flaws

### FL-NANO-7 (P2) — T4/T5 insufficient for race observation

T4 asserts "exactly ONE POST containing the chain" — verifies packaging, not concurrency safety. T5 "zero rollback events for 5min" doesn't prove worst-case interleaving was eliminated.

**Recommendation**: Deterministic interleaving test with instrumented watchdog poll interval and artificial delay between taskkill and ren.

### FL-NANO-8 (P2) — `timeout_ms` / HTTP execution duration not specified

If `/exec` call times out client-side mid-chain, operator may retry while server continues partial completion. Sentinel/agent states alter unpredictably.

**Recommendation**: Specify and enforce `timeout_ms >= worst-case chain duration + buffer`; ensure idempotency.

---

## §6 — Independence from prompt-anchoring (sonnet's strongest critique)

Sonnet's score on this dimension (2.0) plus dedicated rationale paragraph:

> "The PLAN was synthesized from models that were shown the CLAUDE.md 'Remote deploy sequence' canonical pattern in the Step 1+2 prompts. This almost certainly anchored all 5 models toward `single_exec_chain` as the natural solution — it mirrors the existing pattern. The gemini dissent for `new_atomic_endpoint` was the only voice questioning this framing, and it was dismissed as '~200 LOC, deferred.' However, `new_atomic_endpoint` has a fundamentally different correctness property: it moves the atomicity guarantee into the server (rc-sentry), where it can hold a mutex, check sentinel state, and perform the swap as a single operation with proper error handling and rollback. The `single_exec_chain` approach is a client-side hack that relies on OS command sequencing speed beating a timer — it's fragile by construction. The anchoring bias caused the consensus to optimize for LOC minimization (25 LOC vs 200 LOC) over correctness guarantees. A fresh review without the canonical pattern would likely weight `new_atomic_endpoint` more seriously. The 4/5 consensus for `single_exec_chain` may reflect prompt anchoring rather than independent technical judgment."

This is the central architectural critique. The PLAN's selected approach is "fragile by construction" per sonnet. Gemini's deferred `new_atomic_endpoint` is the structurally-correct alternative.

---

## §7 — Disposition options for Captain

### Option A — Re-spin Step 2 PLAN with FL-CONV-1..5 amendments (recommended for next session)

- Cost: ~$0.05-0.10 (5 models with FL-CONV findings included in prompt)
- Carry-forward: amended PLAN addresses sentinel ordering, deploy-watchdog.sh blocking dep, JSON parse fail policy, race timing analysis, sc-start failure handling
- Risk: same anchoring bias may persist if prompt still includes "Remote deploy sequence" pattern
- Time: ~5min wall + Captain review

### Option B — Pivot to gemini's `new_atomic_endpoint` (`/exec_atomic_deploy` in rc-sentry)

- Cost: ~$0.05-0.10 (Step 2 re-spin focused on the alternative architecture; bundles with CF-9 watchdog deploy-aware)
- LOC: ~200 (vs ~183 for current PLAN); both are SMALL-MEDIUM PR shape
- Correctness: server-side mutex + atomic sentinel lifecycle eliminates FL-CONV-1 (sentinel ordering); explicit success/fail response eliminates FL-CONV-3 (parse fail) and FL-CONV-4 (race timing)
- Risk: bigger architectural change, more code paths to test, but structurally correct
- Time: ~5min wall + Captain review

### Option C — Amend CONSENSUS-PLAN.md inline (single-pilot, no MMA)

- Cost: $0
- Risk: single-pilot synthesis without cross-model validation; FL-CONV-1 fix (two-phase sentinel + response parsing) is non-trivial — single-author may miss edge cases that another MMA round would catch
- Time: ~10min james-side authoring

### Option D — Accept BLOCK + halt fleet rollout

- Cost: $0
- Carry-forward: Pod 8 stays on PR #66 binary (continues SHIPPED + stable); Pods 1-7 stay on `c5f94e31-dirty` (silent-loop-death NOT mitigated); Pod 5 stays OFFLINE
- Risk: silent-loop-death exposure on 6 pods continues until amended PLAN ships
- Time: 0; defer to next session

### Option E — Manual atomic-chain-with-sentinel for Pods 1,2,3,4,6,7 as one-time bridge

- Cost: $0; ~30min james-side
- Pre-condition: explicit OTA_DEPLOYING sentinel write + dwell + clear (per CLAUDE.md atomic chain) — same pattern that worked on Pod 8
- Risk: bypasses deploy-pod.sh entirely; manual ops are not permanence-gated (won't survive next deploy attempt unless deploy-pod.sh is also fixed)
- Outcome: PR #66 silent-loop-death fix lands fleet-wide via manual bridge while CF-1+CF-2 PR is being amended

---

## §8 — What I observed (CGP H3 evidence)

**BEHAVIOR**: Authored Step 4 PROMPT.md (9778 chars), invoked 3 vendor-disjoint adversarial models in parallel, parsed JSON responses, scored 6 dimensions per model, identified 8+ flaws, synthesized BLOCK verdict.

**RAW OUTPUT**:
- `resp-gpt-5.4-nano.md` (9905 chars; finish=stop; clean JSON)
- `resp-sonnet-4.6.md` (14809 chars; finish=length; truncated mid-FL-8 description; scores + 6 dimension rationales + 7+ flaws complete; verdict field cut off but inferable from scores)
- `meta-gpt-5.4-nano.json` + `meta-sonnet-4.6.json` (per-model usage + cost)
- `results.json` (3-model summary)
- Spend ledger entry appended to `comms-link/data/openrouter-spend-james.jsonl` at 2026-05-09T14:54:53Z (timestamp from runner)

**WHERE**: Models invoked from James .27 (this terminal); responses written to `racecontrol/.planning/specs/v2/MMA-DEPLOY-RCA-STEP4/`. No live-pod activity.

**NOT TESTED**:
- Substitute model for nvidia/llama-3.1-nemotron-70b 404 — deferred (cannot flip BLOCK to PASS arithmetically)
- Sonnet response tail (verdict field + last 1-2 flaws + 5 amendments tail) — deferred (signal already conclusive)
- Whether FL-CONV-1..5 are actually fixable — that's Step 2 PLAN re-spin territory
- Whether Captain accepts Option A / B / C / D / E
- Bono cross-pilot AMPLIFIER vote on this Step 4 verdict — single-pilot disposition

---

## §9 — Spend log entry (appended)

```json
{
  "timestamp": "2026-05-09T14:54:53Z",
  "ts_ist": "2026-05-09 20:24 IST",
  "pilot": "james",
  "session_purpose": "MMA Step 4 VERIFY — adversarial gate on CF-1+CF-2 bundle PLAN — deploy-mechanism RCA",
  "mma_step": "VERIFY",
  "models": [
    {"model": "anthropic/claude-sonnet-4.6", "status": "OK", "cost": 0.0696, "finish_reason": "length", "score": 2.33},
    {"model": "openai/gpt-5.4-nano", "status": "OK", "cost": 0.0032, "finish_reason": "stop", "score": 1.9},
    {"model": "nvidia/llama-3.1-nemotron-70b-instruct", "status": "API_404", "cost": null, "error": "No endpoints found"}
  ],
  "vendor_families": ["anthropic", "openai", "nvidia(failed)"],
  "valid_responses": 2,
  "total_responses": 3,
  "total_cost_usd": 0.0728,
  "anchor": ".planning/specs/v2/MMA-DEPLOY-RCA-STEP4/CONSENSUS-VERIFY.md",
  "consumes_step2": ".planning/specs/v2/MMA-DEPLOY-RCA-STEP2/CONSENSUS-PLAN.md",
  "verdict": "BLOCK",
  "verdict_score": 2.12,
  "verdict_threshold": 4.0,
  "p0_flaws": ["FL-CONV-1 sentinel-before-chain silent-fleet-death", "FL-CONV-2 Pod-8-OLD-watchdog-suppresses-indefinitely"],
  "structural_critique": "PLAN's single_exec_chain is fragile by construction (sonnet); gemini's deferred new_atomic_endpoint is structurally correct alternative",
  "authorization": "Captain go 2026-05-09 ~20:18 IST → Path B Step 4 first per recommended sequencing"
}
```

---

## §10 — Pickup hook for next session

**Read these first** (in order):
1. This document (CONSENSUS-VERIFY.md)
2. `MMA-DEPLOY-RCA-STEP2/CONSENSUS-PLAN.md` (the BLOCKED PLAN)
3. Per-model raw responses: `resp-gpt-5.4-nano.md` + `resp-sonnet-4.6.md`
4. `MMA-DEPLOY-RCA-DIAGNOSE/CONSENSUS.md` (Step 1, for context)

**Captain Q-DECISION queued**: Pick A / B / C / D / E from §7 disposition options.

**Verb shortcuts**:
- "respin step 2" → Option A
- "pivot to atomic endpoint" → Option B
- "amend inline" → Option C
- "halt rollout" → Option D
- "manual bridge" → Option E

---

— james / 2026-05-09 ~20:24 IST · MMA Step 4 VERIFY complete · 2/3 valid models (nemotron 404; substitute deferred — arithmetic-impossible to flip BLOCK to PASS) · overall score 2.12/5 · 5 P0/P1 convergent flaws + 5 single-model flaws · gemini's deferred new_atomic_endpoint surfaces as structurally-correct alternative · Captain disposition required · cumulative MMA-day spend ~$0.566/$5
