# Phase 299: Policy Rules Engine - Context

**Gathered:** 2026-04-01
**Status:** Ready for planning
**Mode:** Auto-generated (autonomous mode)

<domain>
## Phase Boundary

Staff can define automated IF/THEN rules that respond to live metrics without manual intervention. Rules stored in SQLite, evaluated periodically against the metrics TSDB (Phase 285/v34.0), with actions like sending alerts, changing config, toggling flags, or adjusting budgets. Admin UI for CRUD operations and evaluation log visibility.

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices are at Claude's discretion.

Key observations:
- v34.0 Phase 289 already has alert_rules evaluated against TSDB — this extends that pattern
- Existing alert_engine.rs has check_condition() for threshold evaluation
- The difference: alert rules fire WhatsApp alerts. Policy rules fire arbitrary actions (config change, flag toggle, budget adjust)
- SQLite table: policy_rules with condition, action, enabled, last_fired, eval_count
- Evaluation runs on a periodic task (reuse the metric_alert_task pattern)
- Admin UI: /policy page in racingpoint-admin dashboard group

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `crates/racecontrol/src/alert_engine.rs` — check_condition(), metric evaluation (v34.0)
- `crates/racecontrol/src/metrics_tsdb.rs` — TSDB queries
- `crates/racecontrol/src/config_push.rs` — config push for "change config" action
- `crates/racecontrol/src/db/mod.rs` — SQLite table patterns

### Integration Points
- Reads from metrics TSDB (v34.0)
- Fires config push (Phase 296) for "change config" action
- Feature flags API for "toggle flag" action
- Alert engine for "send alert" action
- Admin page in racingpoint-admin

</code_context>

<specifics>
## Specific Ideas

No specific requirements — standard extension of existing patterns.

</specifics>

<deferred>
## Deferred Ideas

None.

</deferred>
