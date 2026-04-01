---
phase: 294-intelligence-report-v2
plan: 01
subsystem: reporting
tags: [weekly-report, whatsapp, model-accuracy, kb-promotion, cost-savings, model-trends]

requires:
  - phase: 290-model-evaluation-store
    provides: ModelEvalStore::query_all() + compute_rollup() for per-model accuracy data
  - phase: 291-kb-promotion-persistence
    provides: KbPromotionStore::all_candidates() for promotion counts and hardened rule list
  - phase: 292-model-reputation-persistence
    provides: ModelReputationStore::load_all_outcomes() for trend labels
provides:
  - Enhanced weekly_report.rs with 4 new WhatsApp report sections (RPTV2-01..04)
  - Per-model accuracy rankings sorted by accuracy descending (top 5)
  - KB promotion count (rules that advanced stage this week)
  - Cost savings metric from Tier 1 hardened rules ($0 vs $0.001/call avoided)
  - Model trend labels: improving/declining/stable per model
affects: [weekly-report, rptv2, model-lifecycle, whatsapp-escalation]

tech-stack:
  added: []
  patterns:
    - Optional store Arc pattern for graceful degradation (None = "No data this week")
    - Lock-free collect helpers — Mutex acquired and dropped in tight block, no .await while held
    - Short-name truncation for WhatsApp: "deepseek/deepseek-r1-0528" → "deepseek-r1-0528"

key-files:
  created:
    - .planning/phases/294-intelligence-report-v2/294-01-PLAN.md
    - .planning/phases/294-intelligence-report-v2/294-01-SUMMARY.md
  modified:
    - crates/rc-agent/src/weekly_report.rs (enhanced: +479 lines, 4 new sections, 11 new tests)
    - crates/rc-agent/src/main.rs (updated spawn() call, cloned kb_promo_store before move)

key-decisions:
  - "Pass store Arcs as Option<Arc<Mutex<Store>>> — None degrades gracefully without panic"
  - "Clone kb_promo_store before kb_hardening::spawn() consumes it"
  - "Trend logic: promoted/>=70% = improving, demoted/<30% = declining, else stable"
  - "Cost savings = hardened_count * $0.001 per avoided model call (not per-week applications)"
  - "Model name truncation: last segment after / for WhatsApp readability"

patterns-established:
  - "Optional store inject: weekly report helpers accept Option<&Arc<Mutex<Store>>> and return empty/zero on None"
  - "Lock-then-drop pattern: acquire Mutex in tight block, process data synchronously, never hold across .await"

requirements-completed: [RPTV2-01, RPTV2-02, RPTV2-03, RPTV2-04]

duration: 17min
completed: 2026-04-01
---

# Phase 294 Plan 01: Intelligence Report v2 Summary

**Weekly WhatsApp report enhanced with per-model accuracy rankings, KB promotion count, Tier 1 rule cost savings, and improving/declining/stable trend labels — all sourced from Phases 290-292 SQLite stores.**

## Performance

- **Duration:** 17 min
- **Started:** 2026-04-01T14:46:00Z
- **Completed:** 2026-04-01T15:03:00Z
- **Tasks:** 4/4
- **Files modified:** 2

## Accomplishments

### Task 1: WeeklyReport struct + collect_report() with store data

Extended `WeeklyReport` struct with 4 new fields:
- `model_accuracy_rankings: Vec<(String, f64, u32)>` — top 5 by accuracy descending
- `kb_promotions_this_week: u32` — candidates advanced in past 7 days
- `hardened_rule_savings_usd: f64` — hardened_count * $0.001/call
- `model_trends: Vec<(String, String)>` — per-model improving/declining/stable

`collect_report()` now accepts 3 optional store Arcs. Implemented 3 pure sync helpers:
- `collect_model_rankings()` — queries eval store, runs compute_rollup(), sorts by accuracy
- `collect_kb_promotion_stats()` — counts stage advances and hardened candidates
- `collect_model_trends()` — maps reputation rows to trend labels

### Task 2: format_whatsapp_message() with 4 new sections

Added after the existing Knowledge Base section:
```
*Model Performance*
- deepseek-r1-0528: 87% (12 runs)
- qwen3/235b: 71% (8 runs)

*AI Learning*
- KB rules promoted this week: 3
- Cost saved (Tier 1 rules): $0.012

*Model Trends*
- deepseek-r1-0528: improving
- qwen3/235b: stable
```

Empty-state fallback messages when stores return no data.

### Task 3: spawn() signature update + main.rs wiring

- `weekly_report::spawn()` now accepts 3 optional store Arcs
- `main.rs`: clones `kb_promo_store` before `kb_hardening::spawn()` consumes it
- Passes `Some(eval_store.clone())`, `Some(kb_promo_store_for_report)`, `Some(rep_store.clone())`

### Task 4: Unit tests (11 new tests)

- `test_format_with_model_rankings` — RPTV2-01 format
- `test_format_with_kb_promotions` — RPTV2-02+03 format
- `test_format_with_trends` — RPTV2-04 format
- `test_format_empty_stores` — all empty-state placeholders
- `test_collect_model_rankings_none_store` — graceful None
- `test_collect_kb_promotion_stats_none_store` — graceful None
- `test_collect_model_trends_none_store` — graceful None
- `test_collect_model_rankings_with_data` — live in-memory store
- `test_collect_kb_promotion_stats_with_data` — live in-memory store
- `test_collect_model_trends_with_data` — live in-memory store (verifies 90%→improving, 10%→declining, 50%→stable)

## Deviations from Plan

None — plan executed exactly as written.

## Commits

| Task | Commit | Files |
|------|--------|-------|
| All 4 tasks | df478b0b | weekly_report.rs, main.rs |

## Known Stubs

None. All 4 sections wire live store data. Empty-state messages are intentional UX fallbacks, not stubs — the stores return real data once evaluation records and promotion events accumulate.

## Self-Check: PASSED

- [x] `/root/racecontrol/crates/rc-agent/src/weekly_report.rs` — exists, 37 RPTV2 references
- [x] `/root/racecontrol/crates/rc-agent/src/main.rs` — `kb_promo_store_for_report` clone present, spawn call updated
- [x] Commit `df478b0b` exists in git log
- [x] Build errors: same 6 pre-existing Windows-specific errors (unchanged from baseline)
- [x] No new compilation errors introduced
