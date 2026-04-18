# MMA Triage Router + L1 Retrieval — Design Spec

> Build-ready spec for Steps 2 and 3 of the MAP pipeline. Successor to [session_handoff_20260417_mma_pipeline_design.md](../../../../.claude/projects/C--Users-bono/memory/session_handoff_20260417_mma_pipeline_design.md) and [session_handoff_20260418_mi_seed_verification_and_gap4.md](../../../../.claude/projects/C--Users-bono/memory/session_handoff_20260418_mi_seed_verification_and_gap4.md). Consumes [UNIFIED-MMA-PROTOCOL.md](UNIFIED-MMA-PROTOCOL.md). Step 1 (seeder + Gap 1/2/3 fixes) is ALREADY DEPLOYED (`68f4d61e`).

## Why this exists

**Symptom:** 2026-04-17 MMA got 3/4 crash patterns wrong while BUG-TRACKER.md contained the correct diagnoses ([session_handoff_20260417_mma_comprehension.md](../../../../.claude/projects/C--Users-bono/memory/session_handoff_20260417_mma_comprehension.md)).

**Root cause (FM-4 in the handoff catalog):** when James types "why does pod_3 iRacing fail?", nothing routes through the existing Tier-0 oracle first. The query goes straight to `multi-model-audit.js`, which retrieves by keyword similarity (not structural proximity) from a full-directory bundle. Models hallucinate because the bundle contains more distractor than signal.

**Structural fix:** insert a triage router BEFORE the MMA call. The router classifies the query, attempts Tier-0 oracle hit (existing HTTP endpoint `GET /api/v1/mesh/audit-check`), and only falls through to MMA when the oracle has nothing useful. When it does fall through, Step 3 pre-filters the codebase bundle to query-relevant subset.

## Scope

| In | Out |
|---|---|
| Step 2: triage router (Node script + new server endpoint) | Step 1 seeder (already shipped) |
| Step 3: L1 retrieval helper (Node function in `multi-model-audit.js`) | Step 4/5 of the MAP pipeline (validate + synthesize — use existing MMA adversarial verify) |
| Tier-0 server-side oracle path | Pod-local Tier-0 short-circuit via `mi_tier_engine.rs` (requires separate seeder to `mesh_kb.db` in each pod — deferred) |
| Static synonym map for symptom-class expansion | LLM-based expansion (keep deterministic; add LLM later if recall is insufficient) |
| Node 18+ runtime | Rust binary changes (keep router out of `racecontrol` to avoid redeploy churn) |

## Step 2 — Triage Router

### Contract

```
POST /api/v1/diagnose/triage
  Request:
    {
      "query": "why does pod_3 iRacing fail to launch?",
      "caller_context": "ops-triage" | "gsd-debug" | "mma-audit" | "unknown",
      "targets_hint": ["pod_3"] | null,              // optional — narrows pattern match
      "max_category_hits": 5                          // cap on category-class returns
    }

  Response (one of four verdicts):
    {
      "verdict": "TRIAGE_FAST",                       // Tier-0 hit
      "bug_id": "INV-9",
      "fix_status": "CODE_FIXED",
      "affects_targets": ["pod_1","pod_2","pod_3","pod_4","pod_5","pod_6","pod_7"],
      "escalation_message": "...",
      "confidence": 0.95,
      "source": "audit_known_issues",
      "hit_type": "EXACT" | "CATEGORY",
      "should_run_mma": false
    }

    {
      "verdict": "AUDIT_DEEP",                        // oracle miss → defer to MMA
      "relevant_symptom_classes": ["exit-code-1","orphan-process"],
      "candidate_files": ["crates/rc-agent/.../f1_25.rs", ...],  // from Step 3 pre-filter
      "confidence": 0.0,
      "should_run_mma": true
    }

    {
      "verdict": "RESEARCH_NEW",                      // novel symptom, no known pattern
      "rationale": "No symptom class matched. Escalate to human or targeted research.",
      "should_run_mma": true
    }

    {
      "verdict": "REJECT_AMBIGUOUS",                  // query unparseable
      "rationale": "Query too vague — no pod, sim, or symptom class extractable",
      "clarifying_questions": ["Which pod?","What symptom?"],
      "should_run_mma": false
    }
```

### Decision pipeline (layered rules, fail-open to next layer)

1. **Caller-intent gate.** If `caller_context` == `gsd-debug`, default `should_run_mma` to `true` regardless of oracle hit (debug path wants diversity). If `ops-triage`, prefer TRIAGE_FAST hits.
2. **Query parse.** Extract: pod ids, sim names, error strings, symptom keywords. Normalize (pod_3 / Pod 3 / pod 3 → `pod_3`). If nothing extractable → REJECT_AMBIGUOUS.
3. **Symptom-class expansion.** Static synonym map in `scripts/diagnose/symptom-classes.json`:
   ```json
   {
     "exit-code-1": ["exit 1","exit code 1","exited unexpectedly","crash on start"],
     "orphan-process": ["orphan","F1_25.exe running","won't kill","resists taskkill","EA","anti-cheat"],
     "steam-dialog": ["steam dialog","vguiPopupWindow","dialog visible","60s timeout"],
     "zero-laps": ["no laps","laps not recorded","0 laps","empty lap table"],
     "rally-3min": ["AC Rally","Assetto Corsa Rally","3 minute","3-min","180s"]
   }
   ```
4. **Tier-0 oracle call.** `GET /api/v1/mesh/audit-check?symptom=<expanded>&target=<pod_id>`. Use existing `fleet_kb.rs:check_audit_known_issues` — matches stored patterns as substring of expanded query (per [Gap 2 fix](../../../../.claude/projects/C--Users-bono/memory/session_handoff_20260417_mma_pipeline_design.md) in `4f115314`).
5. **Hit classification.**
   - EXACT: returned entry's `affects_targets` includes the queried target → verdict TRIAGE_FAST
   - CATEGORY: entry matched but target not in `affects_targets` → verdict TRIAGE_FAST with `hit_type: CATEGORY` + confidence penalty (mechanism may apply, targets differ). Stage 3 consumer should annotate PARTIAL_MATCH.
   - MISS: no hit → verdict AUDIT_DEEP (call Step 3 for candidate files) OR RESEARCH_NEW (if no symptom class matched either).

### Endpoint implementation

- New handler `crates/racecontrol/src/api/triage.rs` registered as `POST /api/v1/diagnose/triage` (staff-JWT or X-Service-Key gated).
- Reuses existing `fleet_kb::check_audit_known_issues`. No new DB calls beyond what the oracle already does.
- Timeout budget: 200ms p95 (pure DB + hashmap lookups; no LLM).

### CLI wrapper

`scripts/diagnose/triage.js` — ~150 lines Node, usable standalone:
```bash
cd ~/racingpoint/racecontrol
node scripts/diagnose/triage.js "why does pod_3 iRacing fail to launch?"
# → prints JSON verdict + (if AUDIT_DEEP) forwards to multi-model-audit.js
```

Caller-intent detection: check `$GSD_PHASE` env var (set by `/gsd:debug`) to decide `caller_context`. Defaults to `ops-triage`.

### Not in scope

- **Pod-local Tier-0 short-circuit via `mi_tier_engine.rs`.** Requires seeder into each pod's `mesh_kb.db` `hardened_rules` table and an 8-pod deploy cycle. Deferred per Gap 1 decision.
- **LLM-based symptom expansion.** Keep deterministic until static map demonstrates insufficient recall on real queries. Add as optional fallback tier when miss-rate > 30%.
- **Writeback to `fleet_solutions` on verdict.** Stage 5 writeback is in the MMA orchestrator's scope, not the router's.

## Step 3 — L1 Retrieval Helper

### Problem

`multi-model-audit.js` currently bundles whole directories for model context. For a query like "why does pod_3 iRacing fail?", bundling `crates/rc-agent/src/game/` (50+ files) means each model burns 40k+ tokens of distractor. The router's `candidate_files` verdict field exists to replace this with a query-relevant subset.

### Contract

```js
async function prepareQueryRelevantBundle(query, symptomClasses, options = {}) {
  // Returns: {
  //   files: [{ path, content, why_selected }],
  //   tokens_estimated: number,
  //   glob_patterns_used: [...],
  //   grep_patterns_used: [...],
  //   excluded_reasons: { too_large: [...], not_matched: [...] }
  // }
}
```

### Selection algorithm

1. **Glob by symptom class.** Static map `scripts/diagnose/class-to-globs.json`:
   ```json
   {
     "exit-code-1": [
       "crates/rc-agent/src/game_process.rs",
       "crates/rc-agent/src/event_loop.rs",
       "crates/racecontrol/src/game_launcher_support.rs"
     ],
     "orphan-process": [
       "crates/rc-agent/src/pre_flight.rs",
       "crates/rc-agent/src/game_process.rs",
       "crates/racecontrol/src/game_launchers/*.rs"
     ],
     "steam-dialog": [
       "crates/rc-agent/src/steam_checks.rs"
     ]
   }
   ```
2. **Grep-narrow.** For each globbed file, run `rg -l <error_strings>` to confirm the file mentions at least one extracted error string. Files that don't mention any extracted string are excluded with reason `not_matched`.
3. **Cap by token budget.** Default 20k token budget (p50 of what a 5-model audit can cleanly ingest). Sort included files by grep match density, truncate tail files at budget limit. Record truncations in `excluded_reasons.token_cap`.
4. **Annotate each file with `why_selected`.** One line explaining which symptom class + which grep pattern placed it in the bundle. Models see this as a header comment — keeps them oriented.

### Slot-in point

`multi-model-audit.js` currently calls a function like `bundleCodeContext()`. Replace it with a check:
```js
const bundle = query && symptomClasses.length > 0
  ? await prepareQueryRelevantBundle(query, symptomClasses)
  : await bundleWholeDirectory(defaultPath);  // legacy fallback
```

Legacy full-directory path kept for MMA runs without a query (milestone-ship audit, security audit) — those want the whole codebase.

### Not in scope

- **Graphify-based retrieval.** L3 was explicitly deprioritized per [session_handoff_20260417_mma_pipeline_design.md](../../../../.claude/projects/C--Users-bono/memory/session_handoff_20260417_mma_pipeline_design.md) design principle "Graphify enhances, never gates." Optional add-on; don't block Step 3 on it.
- **LLM-assisted ranking.** Keep deterministic until measurable recall gaps exist.

## CGP invariants the router must uphold

| Source | Invariant | Router obligation |
|---|---|---|
| H1 | PROBLEM + PLAN before action | CLI wrapper emits PROBLEM/PLAN block when run interactively |
| H3 | Evidence + raw output + WHERE + NOT-TESTED | Verdict includes `source`, `hit_type`, `confidence`, `should_run_mma`. CLI output prints these verbatim. |
| H4 | Enumerate before "everywhere" | Router outputs that claim "no match found" must include `oracle_endpoints_queried` list |
| H5 | Corrections feed back | Verdict writeback endpoint (POST `/api/v1/diagnose/feedback`) — out of scope this doc, tracked as follow-up |
| Rule 0 | Enumerate before asserting | Symptom-class match lists every class considered + why matched/not |
| SR #15 | Findings feed MI | When router produces RESEARCH_NEW verdict + MMA later converges on answer, Stage 5 (existing MMA) seeds via `/audit-seed-service` |

## Test plan

### Deterministic tests (no LLM)

1. **Exact-hit test.** Query "pod_3 iRacing Steam dialog 60s" → expect TRIAGE_FAST + bug_id=INV-9 + hit_type=EXACT.
2. **Category-hit test.** Query "pod_1 F1 25 exit code 1" (Pod 1 NOT in INV-3's affected pods) → expect TRIAGE_FAST + hit_type=CATEGORY + confidence < 0.8.
3. **Miss test.** Query "random gibberish xyz" → expect RESEARCH_NEW.
4. **Ambiguous test.** Query "it's broken" → expect REJECT_AMBIGUOUS + clarifying_questions populated.
5. **Caller-intent override.** Same as test 1 but `caller_context=gsd-debug` → expect `should_run_mma: true` even with EXACT hit.

### Integration test (requires server)

6. **End-to-end from CLI.** `node scripts/diagnose/triage.js "why does pod_3 iRacing fail?"` → verdict JSON printed, exit code 0 on any non-error verdict.
7. **L1 retrieval sanity.** For a real past query with known-relevant files (Pattern C iRacing → `steam_checks.rs`), assert the file appears in `candidate_files` output.

### Backtest against 2026-04-17 MMA failures

8. **Pattern C (MMA got wrong):** router should return TRIAGE_FAST with INV-9 instead of calling MMA. Expected to prevent the hallucination outright.
9. **Pattern B (BILL-14, now CLOSED):** before `5fcabd38`, router should have returned CATEGORY hit on INV-3 with Pod 1 partial-match annotation. Validates that category-hit logic surfaces useful mechanism even when targets differ.

## Deploy plan (when ready to build)

1. **Commit 1 (router endpoint):** new `crates/racecontrol/src/api/triage.rs` + route registration + unit tests for exact/category/miss logic. Ship separately — only adds a new endpoint, no existing behavior changes.
2. **Commit 2 (CLI wrapper + synonym map):** `scripts/diagnose/triage.js` + `scripts/diagnose/symptom-classes.json`. No server impact.
3. **Commit 3 (L1 retrieval helper):** new `prepareQueryRelevantBundle()` function in `multi-model-audit.js` + `scripts/diagnose/class-to-globs.json`. Slot-in with legacy fallback; no breaking change.
4. **Deploy:** only commit 1 requires server binary rebuild + fleet parity (per Deploy Parity standing rule). Commits 2 and 3 are Node-side, no redeploy.

Estimated LOC: ~300 Rust, ~400 JS, ~150 JSON. 2 focused sessions with test coverage.

## Not done in this spec (deliberate)

- No implementation — this is a build-ready design, not code.
- No deploy — commits above will land in subsequent sessions per the one-pattern-per-deploy rule.
- No Graphify integration — L3 tier stays optional.
- No LLM-based expansion — static map first, measure, then escalate if needed.
- No writeback endpoint contract — follow-up doc when Stage 5 integration is designed.
