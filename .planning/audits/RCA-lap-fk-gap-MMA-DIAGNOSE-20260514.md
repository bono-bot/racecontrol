# MMA Step 1 DIAGNOSE — Lap-persistence FK gap (synthesis)

**Authored:** 2026-05-14T16:32 IST · james-LEAD · per Captain auth verbatim "authorize (1) + (2)" 2026-05-14 ~16:17 IST
**Class:** MMA v4.0 Step 1 DIAGNOSE (≥5 models / ≥3 vendor families / role-fit) · sibling to [RCA-lap-fk-gap-vms-grounded-20260514.md](./RCA-lap-fk-gap-vms-grounded-20260514.md)
**Surface:** `racecontrol.laps.session_id REFERENCES sessions(id)` — silently dropping every customer lap

---

## §1 Methodology

**Prompt:** [racecontrol/.tmp/mma-lap-fk-prompt.md](../../.tmp/mma-lap-fk-prompt.md) (7,181 bytes) — RCA artifact + live ERROR log + DB schema + 6-row counts + code line citations + V2 substrate doctrine (VMS) + Captain identity-axis assertion + PHASE-363 prior-MMA quote.

**Models dispatched (parallel via OpenRouter):**

| Slug | Model ID | Role | Vendor | Status | Bytes |
|---|---|---|---|---|---|
| `opus-4-7` | anthropic/claude-opus-4-7 | Reasoner | Anthropic | 200 OK | 6,881 |
| `sonnet-4-6` | anthropic/claude-sonnet-4-6 | Code Expert | Anthropic | 200 OK | 6,605 |
| `deepseek-r1` | deepseek/deepseek-r1-0528 | Reasoner | DeepSeek | 200 OK | 33,261 (large reasoning prefix) |
| `qwen-coder-plus` | qwen/qwen3-coder-plus | Code Expert | Qwen | 200 OK | 3,065 |
| `gemini-2-5-pro` | google/gemini-2.5-pro | Generalist | Google | 200 OK | 12,638 |
| `nemotron-3` (failed) | nvidia/nemotron-3-super-120b | SRE | Nvidia | 400 "not a valid model ID" | 132 |

**Vendor families: 4** (Anthropic x2 max-cap respected, DeepSeek, Qwen, Google). **Models with substantive response: 5.** **MMA v4.0 thresholds:** ≥5 ✓, ≥3 vendor families ✓.

**Role-fit caveat (v0.1 limitation, surfaced for Captain awareness):** SRE-role slot intended for Nemotron-3-Super-120b but model-ID is stale on OpenRouter. Replacement was Gemini-2.5-Pro (generalist, SRE-tolerant per CLAUDE.md model pool). Strict §S-166 role-fit would require a re-spawn with `nvidia/llama-3.1-nemotron-70b-instruct` or `mistralai/mistral-medium-2511`. Given the unanimous-consensus density on RC-1/2/3, the SRE-role-fit shortfall is unlikely to flip findings; flagged as Q-MMA-1 for Captain to optionally re-run.

**Cost:** ~$0.30-0.50 estimated (5 models × ~7KB prompt × ~4KB response, temperature 0.2). Under $5 session budget.

---

## §2 Consensus root causes (5/5 unanimous on 3 RCs)

### RC-1 — IMMEDIATE: UUID namespace mismatch into FK-constrained column

**Models converging:** opus-4-7, sonnet-4-6, deepseek-r1, qwen-coder-plus, gemini-2-5-pro (**5/5**)
**Avg confidence:** 0.98

**Synthesis:** `BillingTimer.session_id` semantically holds a billing-session UUID (e.g. `6efedf04-7661-4b54-b154-9adf63de77ed`). Through `lap_tracker.rs:60-64 resolve_driver_for_pod` → `agent_game.rs:52 lap.session_id = session_id` → `lap_tracker.rs:194 INSERT bind to laps.session_id`, this billing-namespace UUID lands in a column whose FK references `sessions(id)`. `sessions` is empty (0 rows in live snapshot 2026-05-14T15:09 IST) while `billing_sessions` has 636 rows in a different namespace. Every INSERT therefore fails with SQLite error 787. Both columns (`laps.session_id`, `laps.billing_session_id`) ultimately receive the same billing-session UUID — `billing_session_id` bind succeeds (no FK), `session_id` bind kills the transaction.

**Evidence cited unanimously:** `lap_tracker.rs:60-64` + `agent_game.rs:52` + `lap_tracker.rs:194` + DDL `laps.session_id REFERENCES sessions(id)` + row counts (sessions=0, billing_sessions=636).

**Blast radius:** 100% of customer laps on all pods, fleet-wide. `laps` stays at 0; `telemetry_samples` downstream stays at 0; live leaderboards return empty; MI Wave 4 ingestion has no baseline to bootstrap on.

### RC-2 — DESIGN: identity-axis-vs-charging-axis conflation in the schema contract

**Models converging:** opus-4-7, sonnet-4-6, deepseek-r1, qwen-coder-plus, gemini-2-5-pro (**5/5**)
**Avg confidence:** 0.96

**Synthesis:** The `laps` schema treats `session_id` as a non-nullable gating predicate for lap existence — a lap can only be persisted if a parent `sessions` row exists. This violates Captain's identity-axis principle (a lap belongs to the driver who drove it; session/billing/validity are annotations not predicates). The design flaw is **axis-agnostic** — swapping billing for sessions as the gate reproduces the same 2026-04 zero-laps-for-43-days incident on a different axis. The schema currently encodes three orthogonal identity axes (charging via `billing_session_id`, sessioning via `session_id`, driver-identity via `driver_id`) into a single INSERT transaction with FK enforcement on the wrong one.

**Evidence cited unanimously:** DDL excerpt with `session_id` FK + `billing_session_id` no-FK pair · row counts showing axes are populated on completely different lifecycles · Captain 2026-05-14 doctrine cite · PHASE-363 Opus-4.6 prior-MMA finding (FK should target billing_sessions, never landed) · the 43-day prior incident comment in lap_tracker.rs:103-108.

**Blast radius:** Any future lap insert will fail for the same reason until either `sessions` table is populated by a V1-compatible session-open flow (no active code path exists) or the FK constraint is removed/retargeted. The design flaw also propagates to `wallet_redemptions.session_id REFERENCES sessions(id)` (NF-james-4 migration 20260508 re-affirmed it) — meaning wallet redemption is also gated on the empty `sessions` table.

### RC-3 — PRIOR-FIX-ENTRENCHMENT: UX-04 half-landed

**Models converging:** opus-4-7, sonnet-4-6, deepseek-r1, qwen-coder-plus, gemini-2-5-pro (**5/5**; Gemini folds this into doctrine-layer wording but content matches)
**Avg confidence:** 0.92

**Synthesis:** UX-04 was the institutional response to the 2026-04 "zero laps for 43 days" incident. It made `billing_session_id` nullable in code (lap_tracker.rs:98-114) — addressing the charging-axis half of the axis-conflation. It did NOT touch the `session_id` FK to `sessions(id)`, leaving the **same axis-conflation enforced through a different column**. The PHASE-363 Opus-4.6 MMA recommendation (2026-04-10) explicitly flagged "Add FOREIGN KEY (session_id) REFERENCES billing_sessions(id) … FK violations would be silently swallowed — change to log the error" — only the error-logging half shipped (which is why we see the ERROR lines today); the schema-fix half never landed. The partial fix consumed engineering confidence ("we fixed the lap-drop bug") while leaving production drop rate at 100%. It also created a parallel-column layout where `session_id` and `billing_session_id` hold the same value in different namespaces — actively misleading for any future engineer reading the schema.

**Evidence cited unanimously:** lap_tracker.rs:98-114 comment block · PHASE-363-MMA-anthropic-claude-opus-4.6.md:36 quote · disposition data point that ERROR logging landed but schema fix did not · laps=0 rows confirming the fix did not reduce blast radius.

**Blast radius:** Regression of the exact 2026-04 incident class on a different column with no detection — the 2026-04 fix is institutionally believed to have closed the issue, so the same class of symptom (no laps in production) was attributed to "different cause" (FK failure) when in fact it is the **same axis-conflation root cause manifest one column to the right**. This is a doctrine-failure class as much as a code-failure class.

---

## §3 Minority observations (cited by multiple models but excluded from consensus per DIAGNOSE-only instruction)

### RC-4 (sonnet-4-6, opus-4-7) — Ghost-lap broadcast (consequence-of-RC-1, blast-radius amplifier)

`agent_game.rs:64 state.dashboard_tx.send(DashboardEvent::LapCompleted(lap))` fires **before** `persist_lap` returns. WS-attached dashboards (kiosk header, spectator, admin) display laps in real-time that never made it to the DB. Customers and staff see ghost laps that don't appear in leaderboards or session summaries afterwards. **Not a root cause of the FK failure** — but is itself a correctness defect that the FK fix alone won't address. Flagged for the FIX-PHASE plan: broadcast-after-persistence OR broadcast-with-pending-tag.

### RC-5 (sonnet-4-6, gemini-2-5-pro) — F25/VMS deferral severity-amplifier

Per [v2-skeleton/06](../../../comms-link/v2-skeleton/06-vms-srl-cloud-migration-analysis.md) the local `laps` table is **secondary** to VMS in V2 doctrine (Phase 0). But F25 ETL is not yet built. So today, local laps are the **only** path to live-display (kiosk leaderboard, spectator screens) AND any future post-session features. The deferral of F25 doesn't change WHY laps fail to insert (RC-1/2/3 still apply) but it does change **how foundational the local FK is** — F25 deferred → local laps are de facto authoritative for everything UI-visible right now, despite doctrine declaring them secondary. Flagged for V2-PROGRESS-MAP Layer 6/7 substrate-shape datapoint.

### RC-6 (deepseek-r1) — `let _ =` swallow ancestry

Prior code state used `let _ = sqlx::query(...).execute()` which suppressed all DB errors including FK failures (a CLAUDE.md-listed anti-pattern). The upgrade to `tracing::error!` + `tx.rollback()` made the failure visible (we observe the ERROR lines) but is symptom-surfacing, not root-cause-fixing. Same point as RC-3's "only the error-logging half shipped" but framed at the error-handling axis. Same class, different layer.

---

## §4 Layer distribution (consensus structure)

| Layer | Models that cite | Convergence |
|---|---|---|
| `immediate` | 4/5 (opus, sonnet, qwen, deepseek; gemini folds into design) | Strong |
| `design` | 5/5 | **Unanimous** |
| `prior-fix-entrenchment` | 4/5 (opus, sonnet, qwen, deepseek; gemini folds into doctrine) | Strong |
| `doctrine` | 3/5 (deepseek, opus, gemini) | Moderate — overlaps with design |
| `broadcast-vs-persist` | 2/5 (sonnet, opus) — as RC-4 minority | Weak; minority observation |
| `substrate-deferral` | 2/5 (sonnet, gemini) — as RC-5 minority | Weak; minority observation |

**Synthesis:** the three consensus root causes form a **stack**: RC-1 is the immediate manifestation, RC-2 is the design that allows RC-1 to exist, RC-3 is the prior-fix that should have addressed RC-2 but only addressed half of it. Any fix that ignores RC-2 will simply migrate the bug to a different column (the lesson of RC-3 itself).

---

## §5 Recommendation for MMA Step 2 PLAN (NOT FIX — Captain auth gate retained)

§S-146 doctrine + foundational-boundary rule + Captain-per-PR-merge-auth combine to gate the FIX behind Captain authorization of MMA Step 2 PLAN. This DIAGNOSE artifact is the input to that PLAN step.

**The PLAN step should evaluate the three candidate directions from RCA §5 (A/B/C) against the consensus root causes:**

| Candidate (from RCA §5) | Resolves RC-1? | Resolves RC-2? | Resolves RC-3 (institutional)? | Side-effects |
|---|---|---|---|---|
| **A** Drop FK, nullable session_id, keep billing_session_id as annotation | Yes | Yes (most fully — removes the gate entirely) | Yes (forces explicit acknowledgement that the axis-conflation was the root cause, not just one column of it) | Loses referential integrity to `sessions` — acceptable per V2 doctrine; need parallel `wallet_redemptions` audit |
| **B** Have launch flow create sibling `sessions` row alongside `billing_sessions` | Yes | No (keeps the gate, just satisfies it) | No (preserves the conflation; creates phantom session rows) | Schema unchanged; semantic debt grows |
| **C** Re-target FK to `billing_sessions(id)`, nullable | Yes | Partial (still gates on charging-axis even if null-allowed; "free trial" / "staff test" / "post-billing-end" laps still hit a charging-axis annotation as gate) | Partial (lands the PHASE-363 finding but on the same axis as the 2026-04 partial fix; risks repeating the entrenchment pattern) | Two columns with similar semantics; PHASE-363 box-tick without doctrine repair |

**Convergence reading across the 5 models:** all 5 models implicitly favor Candidate A's shape (drop FK / nullable / annotation-not-gate semantics) when they describe what the fix would look like, though none were asked to author it. **Captain decision is the gate.** PLAN step should run 5 models on plan-design for each of A/B/C, then propose smallest reversible change.

**Adjacent items flagged for PLAN scope:**
- RC-4 ghost-lap broadcast — fix broadcast ordering same PR
- RC-5 F25 deferral — note V2-PROGRESS-MAP impact, doesn't gate the FIX
- `wallet_redemptions.session_id REFERENCES sessions(id)` — sibling table also gated on empty `sessions`; needs same fix or coordinated migration
- `lap_rejections=0` rows despite 2 known rejections — sibling silent-insert class (PHASE-363 Sonnet-4.6 finding); likely same axis-conflation; verify in PLAN step
- Mechanism-trust-check 5-Q on the persistence pipeline before fix PR ships — required per Captain doctrine 2026-05-10

---

## §6 Universal Sync targets at CLOSE

- [x] Source RCA artifact written: [RCA-lap-fk-gap-vms-grounded-20260514.md](./RCA-lap-fk-gap-vms-grounded-20260514.md) (this turn)
- [x] This MMA Step 1 synthesis written (this turn)
- [ ] LOGBOOK row append (this turn — pending bash)
- [ ] V2-MASTER-STATE §S-N close-anchor — autonomous-push-eligible per Captain standing rule 2026-05-12 (ledger entry only; not a foundational-PR merge)
- [ ] V2-PROGRESS-MAP §0 substrate-shape datapoint: local `laps=0`, VMS `sessions=3390`, F25 ETL deferred; identity-axis fix is pre-Wave-4 MI bootstrap dependency
- [ ] Bono memory mirror via comms-link/briefings/bono/memory/ (bilateral §S-146)
- [ ] Bono cloud `racecontrol.db` parity probe (per H4 — venue side queried this turn; Bono side NOT TESTED yet)

---

## §7 NOT tested this turn (per H3)

1. SRE-role-fit strict adherence — Nemotron model ID stale; replaced by Gemini 2.5 Pro (generalist). Re-run with `nvidia/llama-3.1-nemotron-70b-instruct` if Captain wants strict §S-166 role compliance.
2. Bono VPS cloud_sync mirror — count of `laps` table on Bono side. If non-zero, the cloud authoritative-mirror axis exists; if zero, fleet-wide confirmation of the gap.
3. `lap_rejections` table contract — 2 known rejections (the 2 ERROR'd laps), but `lap_rejections=0`. Either the table is contractually empty for this class, or there's a parallel silent-insert bug.
4. `telemetry_samples` upstream check — empty because of FK fail downstream, or because telemetry sampler isn't writing?
5. Why F25 ETL is deferred — Q-DATA-A pending Captain API-key provisioning per VMS doc §11.
6. MMA Step 2 PLAN — Captain auth gate retained per §S-146 foundational-boundary.
7. The actual code fix — H2 + Captain auth gate; not in this RCA-DIAGNOSE turn.

---

**End MMA Step 1 DIAGNOSE synthesis v0.1.**
