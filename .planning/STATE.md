---
gsd_state_version: 1.0
milestone: v35.0
milestone_name: Structured Retraining & Model Lifecycle
status: roadmap_complete
stopped_at: Phase 280 (not started)
last_updated: "2026-04-01T19:30:00.000Z"
last_activity: 2026-04-01 — Roadmap created, ready for Phase 280 planning
progress:
  total_phases: 5
  completed_phases: 0
  total_plans: 0
  completed_plans: 0
  percent: 0
---

## Current Position

Phase: 280 — Model Evaluation Store (not started)
Plan: —
Status: Roadmap complete, ready for planning
Last activity: 2026-04-01 — v35.0 roadmap created (5 phases, 20 requirements)

Progress: [░░░░░░░░░░] 0%  (0/5 phases)

```
280 ──┬──> 281 (KB Promotion) ────┐
      ├──> 282 (Model Reputation) ┤──> 284 (Report v2)
      └──> 283 (Retrain Export) ──┘
```

## Project Reference

**Milestone:** v35.0 Structured Retraining & Model Lifecycle
**Core value:** Close the continuous learning loop — system gets measurably smarter each week
**Roadmap:** .planning/ROADMAP.md
**Requirements:** .planning/REQUIREMENTS.md

See: .planning/PROJECT.md (updated 2026-04-01)

## Performance Metrics

| Metric | Value |
|--------|-------|
| Phases total | 5 |
| Phases complete | 0 |
| Requirements total | 20 |
| Requirements mapped | 20 |
| Coverage | 100% |
| Phases unblocked | 1 (Phase 280) |
| Phases blocked | 3 (281, 282, 283 — await 280) |
| Phases deeply blocked | 1 (284 — awaits 280, 281, 282) |

## Accumulated Context

### Key Decisions

- **Phase numbering starts at 280**: Continuing from v32.0 which ended at Phase 279.
- **Phase 280 is strict foundation**: All other phases depend on it for evaluation data. Cannot parallelize before it completes.
- **Phases 281, 282, 283 are parallel**: Once 280 is complete all three can be planned and executed in parallel.
- **Phase 284 is integration gate**: Depends on 280 + 281 + 282. Can overlap with 283 (retrain export is independent of 284 inputs).
- **No new infrastructure**: All new tables land in existing SQLite databases (rc-agent's DB or racecontrol's DB — confirm in Phase 280 plan).
- **v32.0 in-memory reputation reuses**: `model_reputation.rs` and `kb_hardening.rs` exist; v35.0 adds SQLite persistence layers, does not rewrite from scratch.

### From v32.0 (carried forward)

- **model_reputation.rs** exists with in-memory accuracy tracking — Phase 282 adds persistence
- **kb_hardening.rs** exists with basic ladder — Phase 281 adds SQLite persistence + Shadow/Canary/Quorum stages
- **mma_engine::get_all_model_stats()** returns per-model accuracy data — Phase 280 adds structured write path
- **weekly_report.rs** exists — Phase 284 enhances with model accuracy rankings, KB promotion count, cost savings
- **EscalationPayload** used for WhatsApp delivery — reuse for enhanced reports in Phase 284
- **FleetEvent bus** carries all events — evaluation events can ride the bus
- **Budget tracker** in mma_engine resets daily — weekly accumulation not tracked yet; Phase 280 rollups handle this

### Todos

- [ ] Run `gsd-codebase-mapper` before Phase 280 planning (mandatory for new milestone per standing rules)
- [ ] Confirm which SQLite database file gets new tables (rc-agent local DB vs racecontrol server DB)
- [ ] Identify existing `mma_engine` write path to hook EVAL-01 into

### Blockers

None currently.

## Session Continuity

Last session: 2026-04-01T19:30:00.000Z
Stopped at: Roadmap complete — start with `/gsd:plan-phase 280`
Resume file: None
