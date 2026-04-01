# Roadmap: RaceControl Ops

## Milestones

- ✅ **v1.0** — Phases 01-36 (shipped)
- ✅ **v10.0** — Phases 41-50 (shipped)
- ✅ **v11.0** — Phases 51-60 (shipped)
- ✅ **v16.1** — Camera Dashboard Pro (shipped)
- ✅ **v17.1** — Phases 66-80 (shipped)
- ✅ **v21.0** — Cross-Project Sync (shipped)
- ✅ **v25.0** — Phases 81-96 (shipped)
- ✅ **v32.0 Autonomous Meshed Intelligence** — Phases 273-279 (shipped 2026-04-01)
- 🔄 **v35.0 Structured Retraining & Model Lifecycle** — Phases 290-294 (in progress)

See `.planning/milestones/` for archived roadmaps and requirements per milestone.

## v32.0 Summary (Shipped 2026-04-01)

Close all action loops in Meshed Intelligence: anomaly → diagnose → fix → verify → KB encode → fleet cascade.

7 phases, 38 requirements, 10 key files (~2,351 lines), 55 commits.

<details>
<summary>v32.0 Phases (273-279)</summary>

- [x] Phase 273: Event Pipeline & Safety Foundation — FleetEvent bus, blast radius limiter, circuit breaker, KB-first gate
- [x] Phase 274: WhatsApp Escalation — Tier 5 via Bono VPS Evolution API, dedup, INBOX.md fallback
- [x] Phase 275: Autonomous Game Launch Fix — 60s recovery, retry, KB encode, fleet cascade
- [x] Phase 276: Predictive Alerts & Experience Scoring — Alerts→tier engine, per-pod CX score
- [x] Phase 277: Revenue Protection & Model Reputation — Billing/game mismatch, model auto-demotion
- [x] Phase 278: KB Hardening Pipeline — Observed→Shadow→Canary→Quorum→Deterministic Rule
- [x] Phase 279: Weekly Report & Integration Audit — Sunday midnight IST KPI report via WhatsApp

</details>

---

## v35.0 Structured Retraining & Model Lifecycle

**Goal:** Close the continuous learning loop — solutions that work get promoted, models that underperform get demoted, the system gets measurably smarter each week.

**Phases:** 5  |  **Granularity:** Standard  |  **Coverage:** 20/20 requirements mapped

### Phases

- [x] **Phase 290: Model Evaluation Store** — SQLite persistence for every AI diagnosis and weekly accuracy rollups (completed 2026-04-01)
- [x] **Phase 291: KB Promotion Persistence** — Shadow/Canary/Quorum/Hardened ladder survives restarts, 6-hour cron evaluator (completed 2026-04-01)
- [ ] **Phase 292: Model Reputation Persistence** — Per-model accuracy persisted, auto-demotion and promotion on 7-day windows
- [x] **Phase 293: Retrain Data Export** — Weekly JSONL export in Ollama/Unsloth training format (completed 2026-04-01)
- [x] **Phase 294: Intelligence Report v2** — Weekly WhatsApp with accuracy rankings, KB promotion count, cost savings, prediction trends (completed 2026-04-01)

### Progress Table

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 290. Model Evaluation Store | 3/3 | Complete   | 2026-04-01 |
| 291. KB Promotion Persistence | 3/3 | Complete   | 2026-04-01 |
| 292. Model Reputation Persistence | 1/2 | In Progress|  |
| 293. Retrain Data Export | 1/1 | Complete   | 2026-04-01 |
| 294. Intelligence Report v2 | 1/1 | Complete   | 2026-04-01 |

---

## Phase Details

### Phase 290: Model Evaluation Store
**Goal**: Every AI diagnosis is recorded with its prediction, outcome, and cost so accuracy can be measured over time
**Depends on**: Nothing (foundation for all other phases)
**Requirements**: EVAL-01, EVAL-02, EVAL-03
**Success Criteria** (what must be TRUE):
  1. After any AI diagnosis runs, a new row exists in `model_evaluations` with prediction, actual outcome, correctness flag, and cost
  2. A weekly cron job produces one `model_eval_rollups` row per model with accuracy and cost-per-correct-diagnosis
  3. `GET /api/v1/models/evaluations?model=X&from=Y&to=Z` returns filtered evaluation records with correct data
  4. Evaluation data persists across rc-agent restarts — records written before a restart are readable after
**Plans**: 3 plans
- [x] 290-01-PLAN.md — model_eval_store.rs schema + store.insert() wired into tier_engine (EVAL-01)
- [x] 290-02-PLAN.md — eval_rollup.rs weekly cron producing model_eval_rollups (EVAL-02)
- [x] 290-03-PLAN.md — AgentMessage::ModelEvalSync + server DB table + GET /api/v1/models/evaluations (EVAL-03)

### Phase 291: KB Promotion Persistence
**Goal**: The KB promotion ladder (Shadow/Canary/Quorum/Hardened) survives process restarts and advances automatically every 6 hours
**Depends on**: Phase 290 (evaluation data feeds promotion decisions)
**Requirements**: KBPP-01, KBPP-02, KBPP-03, KBPP-04, KBPP-05, KBPP-06
**Success Criteria** (what must be TRUE):
  1. After rc-agent restarts, all KB candidate rules resume at their previous promotion stage (Shadow/Canary/Quorum/Hardened) — no regression to Observed
  2. A Shadow-stage rule executes on all pods for 1 week or 25 applications (whichever comes first) and only logs — it does not modify pod state
  3. A Canary-stage rule is applied only to Pod 8; the 6-hour cron does not promote it until Pod 8 verifies success
  4. A Quorum-stage rule requires success on 3+ distinct pods from 2+ distinct pod IDs before advancing to Hardened
  5. A Hardened rule is stored as a typed `Rule` struct (matchers, actions, verifier, TTL) and applied at $0 model cost
  6. The 6-hour cron evaluator runs, checks all candidate promotions, and advances or holds each based on their stage criteria
**Plans**: 3 plans
- [x] 291-01-PLAN.md — kb_promotion_store.rs schema + KbPromotionStore wired into kb_hardening.rs for restart persistence + stage gates (KBPP-01, KBPP-02, KBPP-03, KBPP-04)
- [x] 291-02-PLAN.md — tier0_hardened_rule() in tier_engine.rs — Hardened rules applied at $0 cost before model tiers (KBPP-05)
- [x] 291-03-PLAN.md — 6-hour cron constant + promotion audit logging + integration tests (KBPP-06)

### Phase 292: Model Reputation Persistence
**Goal**: Per-model accuracy and run counts are durable so the roster self-curates over rolling 7-day windows without requiring a human to intervene
**Depends on**: Phase 290 (accuracy data from evaluation store)
**Requirements**: MREP-01, MREP-02, MREP-03, MREP-04
**Success Criteria** (what must be TRUE):
  1. After rc-agent restarts, per-model accuracy and run count are restored from SQLite — no reset to zero
  2. A model with 7-day accuracy below 30% across 5+ runs is absent from the MMA roster for subsequent diagnoses
  3. A model with 7-day accuracy above 90% across 10+ runs appears at higher priority in the MMA roster than a model with lower accuracy
  4. `GET /api/v1/models/reputation` returns per-model trends including accuracy, run count, and cost efficiency
**Plans**: 2 plans
- [ ] 292-01-PLAN.md — model_reputation_store.rs + updated run_reputation_sweep() using 7-day eval window + boot load in main.rs (MREP-01, MREP-02, MREP-03)
- [x] 292-02-PLAN.md — AgentMessage::ModelReputationSync + server DB table + GET /api/v1/models/reputation (MREP-04)

### Phase 293: Retrain Data Export
**Goal**: Every week, the system produces a training-ready JSONL file that captures what the AI diagnosed, whether it was correct, and what fix was applied — usable directly with Ollama or Unsloth
**Depends on**: Phase 290 (evaluation data is the source of the export)
**Requirements**: TRAIN-01, TRAIN-02, TRAIN-03
**Success Criteria** (what must be TRUE):
  1. A weekly cron job produces a JSONL file containing diagnosis evaluations and KB solutions from the past 7 days
  2. Each JSONL entry has `system`, `user`, and `assistant` fields — the file loads without modification in an Ollama fine-tune run and in Unsloth's conversation format
  3. Each entry includes model name, original prompt, model response, correct/incorrect flag, and fix outcome
**Plans**: 1 plan
- [x] 293-01-PLAN.md — retrain_export.rs: TrainEntry + JSONL writer + weekly cron wired into main.rs (TRAIN-01, TRAIN-02, TRAIN-03)

### Phase 294: Intelligence Report v2
**Goal**: Uday's Sunday midnight WhatsApp report tells him which models are improving, how many KB rules were promoted, how much cost was saved by Hardened rules, and whether model accuracy is trending up or down
**Depends on**: Phase 290 (evaluation data), Phase 291 (KB promotion counts), Phase 292 (model reputation trends)
**Requirements**: RPTV2-01, RPTV2-02, RPTV2-03, RPTV2-04
**Success Criteria** (what must be TRUE):
  1. The weekly WhatsApp report includes a per-model accuracy ranking (e.g. "Model A: 87%, Model B: 63%, Model C: 41%")
  2. The weekly report includes a count of KB rules that advanced promotion stage during the past week
  3. The weekly report includes a dollar figure for cost savings: number of Hardened-rule applications multiplied by estimated model call cost that would otherwise have been incurred
  4. The weekly report characterizes each model's accuracy trend as "improving", "declining", or "stable" based on the last two rolling weeks
**Plans**: 1 plan
- [x] 294-01-PLAN.md — enhance weekly_report.rs with RPTV2 sections + main.rs wiring (RPTV2-01, RPTV2-02, RPTV2-03, RPTV2-04)

---

*Last updated: 2026-04-01 — Phase 294 complete (1 plan, v35.0 milestone complete)*
