# Requirements: v35.0 Structured Retraining & Model Lifecycle

**Defined:** 2026-04-01
**Core Value:** Close the continuous learning loop — system gets measurably smarter each week

## Model Evaluation Store (EVAL)

- [ ] **EVAL-01**: Every AI diagnosis writes prediction, actual outcome, correctness, and cost to SQLite `model_evaluations` table
- [ ] **EVAL-02**: Weekly rollup computes per-model accuracy and cost-per-correct-diagnosis (stored in `model_eval_rollups`)
- [ ] **EVAL-03**: Evaluation data queryable via API: `GET /api/v1/models/evaluations?model=X&from=Y&to=Z`

## KB Promotion Persistence (KBPP)

- [ ] **KBPP-01**: KB promotion state (Shadow/Canary/Quorum/Hardened) persists in SQLite across rc-agent restarts
- [ ] **KBPP-02**: Shadow mode tracks candidate fix executions for 1 week or 25 applications (whichever first), logging only
- [ ] **KBPP-03**: Canary stage restricts candidate fix to Pod 8 only and verifies success before fleet promotion
- [ ] **KBPP-04**: Quorum requires 3+ successes across 2+ distinct pods before promoting to Hardened (Tier 1 rule)
- [ ] **KBPP-05**: Hardened rules stored as typed `Rule` structs with matchers, actions, verifier, and TTL — applied at $0 cost
- [ ] **KBPP-06**: 6-hour cron evaluator checks all candidate promotions and advances eligible entries

## Model Reputation Persistence (MREP)

- [ ] **MREP-01**: Per-model accuracy and run count tracked persistently in SQLite (survives rc-agent restart)
- [ ] **MREP-02**: Models with 7-day accuracy below 30% across 5+ runs are automatically removed from MMA roster
- [ ] **MREP-03**: Models with 7-day accuracy above 90% across 10+ runs are promoted to higher priority in roster
- [ ] **MREP-04**: Model reputation dashboard visible at `/api/v1/models/reputation` (per-model trends, cost efficiency)

## Retrain Data Export (TRAIN)

- [ ] **TRAIN-01**: Weekly cron exports diagnosis evaluations + KB solutions as JSONL training data
- [ ] **TRAIN-02**: Export format compatible with Ollama fine-tuning and Unsloth (conversation pairs with system/user/assistant)
- [ ] **TRAIN-03**: Export includes model name, prompt, response, correct/incorrect, and fix outcome for each entry

## Intelligence Report v2 (RPTV2)

- [ ] **RPTV2-01**: Weekly report includes per-model accuracy rankings (not just aggregate MTTR)
- [ ] **RPTV2-02**: Weekly report includes KB promotion count (how many rules advanced this week)
- [ ] **RPTV2-03**: Weekly report includes cost savings from Tier 1 hardened rules ($0 vs estimated model cost)
- [ ] **RPTV2-04**: Weekly report includes prediction accuracy trends (improving/declining/stable per model)

## Traceability

| REQ | Phase | Status |
|-----|-------|--------|
| EVAL-01 | Phase 280 | Pending |
| EVAL-02 | Phase 280 | Pending |
| EVAL-03 | Phase 280 | Pending |
| KBPP-01 | Phase 281 | Pending |
| KBPP-02 | Phase 281 | Pending |
| KBPP-03 | Phase 281 | Pending |
| KBPP-04 | Phase 281 | Pending |
| KBPP-05 | Phase 281 | Pending |
| KBPP-06 | Phase 281 | Pending |
| MREP-01 | Phase 282 | Pending |
| MREP-02 | Phase 282 | Pending |
| MREP-03 | Phase 282 | Pending |
| MREP-04 | Phase 282 | Pending |
| TRAIN-01 | Phase 283 | Pending |
| TRAIN-02 | Phase 283 | Pending |
| TRAIN-03 | Phase 283 | Pending |
| RPTV2-01 | Phase 284 | Pending |
| RPTV2-02 | Phase 284 | Pending |
| RPTV2-03 | Phase 284 | Pending |
| RPTV2-04 | Phase 284 | Pending |

## Future Requirements (deferred)

- Chart image attached to weekly WhatsApp report (requires image generation library)
- Real-time model accuracy dashboard in admin UI (wait for v34 metrics TSDB)
- Automated Ollama fine-tuning trigger (wait for Ollama to support fine-tuning API)

## Out of Scope

- MLflow/Kubeflow integration — SQLite evaluation store is sufficient at venue scale
- Feature store — fleet_solutions + model_evaluations table is the feature store
- Multi-venue model training — single-venue only for now
- GPU-based training on venue hardware — export JSONL for external training only
