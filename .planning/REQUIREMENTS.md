# Requirements: v35.0 Structured Retraining & Model Lifecycle

**Defined:** 2026-04-01
**Core Value:** Close the continuous learning loop — system gets measurably smarter each week

## Model Evaluation Store (EVAL)

- [x] **EVAL-01**: Every AI diagnosis writes prediction, actual outcome, correctness, and cost to SQLite `model_evaluations` table
- [x] **EVAL-02**: Weekly rollup computes per-model accuracy and cost-per-correct-diagnosis (stored in `model_eval_rollups`)
- [x] **EVAL-03**: Evaluation data queryable via API: `GET /api/v1/models/evaluations?model=X&from=Y&to=Z`

## KB Promotion Persistence (KBPP)

- [x] **KBPP-01**: KB promotion state (Shadow/Canary/Quorum/Hardened) persists in SQLite across rc-agent restarts
- [x] **KBPP-02**: Shadow mode tracks candidate fix executions for 1 week or 25 applications (whichever first), logging only
- [x] **KBPP-03**: Canary stage restricts candidate fix to Pod 8 only and verifies success before fleet promotion
- [x] **KBPP-04**: Quorum requires 3+ successes across 2+ distinct pods before promoting to Hardened (Tier 1 rule)
- [x] **KBPP-05**: Hardened rules stored as typed `Rule` structs with matchers, actions, verifier, and TTL — applied at $0 cost
- [x] **KBPP-06**: 6-hour cron evaluator checks all candidate promotions and advances eligible entries

## Model Reputation Persistence (MREP)

- [x] **MREP-01**: Per-model accuracy and run count tracked persistently in SQLite (survives rc-agent restart)
- [x] **MREP-02**: Models with 7-day accuracy below 30% across 5+ runs are automatically removed from MMA roster
- [x] **MREP-03**: Models with 7-day accuracy above 90% across 10+ runs are promoted to higher priority in roster
- [x] **MREP-04**: Model reputation dashboard visible at `/api/v1/models/reputation` (per-model trends, cost efficiency)

## Retrain Data Export (TRAIN)

- [x] **TRAIN-01**: Weekly cron exports diagnosis evaluations + KB solutions as JSONL training data
- [x] **TRAIN-02**: Export format compatible with Ollama fine-tuning and Unsloth (conversation pairs with system/user/assistant)
- [x] **TRAIN-03**: Export includes model name, prompt, response, correct/incorrect, and fix outcome for each entry

## Intelligence Report v2 (RPTV2)

- [x] **RPTV2-01**: Weekly report includes per-model accuracy rankings (not just aggregate MTTR)
- [x] **RPTV2-02**: Weekly report includes KB promotion count (how many rules advanced this week)
- [x] **RPTV2-03**: Weekly report includes cost savings from Tier 1 hardened rules ($0 vs estimated model cost)
- [x] **RPTV2-04**: Weekly report includes prediction accuracy trends (improving/declining/stable per model)

## Traceability

| REQ | Phase | Status |
|-----|-------|--------|
| EVAL-01 | Phase 290 | Complete |
| EVAL-02 | Phase 290 | Complete |
| EVAL-03 | Phase 290 | Complete |
| KBPP-01 | Phase 291 | Complete |
| KBPP-02 | Phase 291 | Complete |
| KBPP-03 | Phase 291 | Complete |
| KBPP-04 | Phase 291 | Complete |
| KBPP-05 | Phase 291 | Complete |
| KBPP-06 | Phase 291 | Complete |
| MREP-01 | Phase 292 | Complete |
| MREP-02 | Phase 292 | Complete |
| MREP-03 | Phase 292 | Complete |
| MREP-04 | Phase 292 | Complete |
| TRAIN-01 | Phase 293 | Complete |
| TRAIN-02 | Phase 293 | Complete |
| TRAIN-03 | Phase 293 | Complete |
| RPTV2-01 | Phase 294 | Complete |
| RPTV2-02 | Phase 294 | Complete |
| RPTV2-03 | Phase 294 | Complete |
| RPTV2-04 | Phase 294 | Complete |

## Future Requirements (deferred)

- Chart image attached to weekly WhatsApp report (requires image generation library)
- Real-time model accuracy dashboard in admin UI (wait for v34 metrics TSDB)
- Automated Ollama fine-tuning trigger (wait for Ollama to support fine-tuning API)

## Out of Scope

- MLflow/Kubeflow integration — SQLite evaluation store is sufficient at venue scale
- Feature store — fleet_solutions + model_evaluations table is the feature store
- Multi-venue model training — single-venue only for now
- GPU-based training on venue hardware — export JSONL for external training only
