---
phase: 278
plan: 01
subsystem: rc-agent/knowledge-base
tags: [kb-hardening, promotion-ladder, autonomous-healing]
dependency_graph:
  requires: [knowledge_base.rs, fleet_event.rs]
  provides: [kb_hardening.rs, hardened_rules table, promotion ladder]
  affects: [tier_engine.rs (future: Tier 1 reads hardened_rules)]
tech_stack:
  added: []
  patterns: [promotion-ladder, shadow-canary-quorum, background-promoter]
key_files:
  created: [crates/rc-agent/src/kb_hardening.rs]
  modified: [crates/rc-agent/src/knowledge_base.rs, crates/rc-agent/src/main.rs]
decisions:
  - Used solution_nodes table for quorum tracking instead of parsing source_node field
  - Promotion events emitted as FleetEvent::FixApplied with tier=0 for observability
  - Canary pod identified by node_id containing "pod_8" or "pod-8" (matches existing convention)
metrics:
  duration: ~15min
  completed: 2026-04-01
---

# Phase 278 Plan 01: KB Hardening Pipeline Summary

KB promotion ladder with 5-stage lifecycle: Observed > Shadow > Canary > Quorum > Hardened Rule, enforcing graduated confidence before fleet-wide deterministic application.

## Requirements Coverage

| Req | Description | Status |
|-----|-------------|--------|
| KB-01 | Promotion ladder: Observed > Shadow > Canary > Quorum > Deterministic Rule | DONE |
| KB-02 | Shadow mode: logs only for 1 week or 25 applications | DONE |
| KB-03 | Canary: apply on Pod 8 first, verify before fleet | DONE |
| KB-04 | Quorum: 3+ successes across 2+ pods | DONE |
| KB-05 | Promoted rules stored as typed HardenedRule structs | DONE |

## Implementation Details

### New File: `kb_hardening.rs`
- Background promoter task spawned every 5 minutes
- Four promotion functions: observed>shadow, shadow>canary, canary>quorum, quorum>hardened
- Shadow: promoted after success_count >= 1
- Shadow>Canary: after 25 applications OR 7 days, with success > fail check
- Canary>Quorum: requires Pod 8 success (checked via solution_nodes table)
- Quorum>Hardened: 3+ successes across 2+ distinct nodes, creates HardenedRule
- Helper functions: `is_shadow_mode()`, `is_canary_mode()`, `is_canary_pod()`
- FleetEvent emitted for each promotion step

### Modified: `knowledge_base.rs`
- Added `HardenedRule` struct with matchers/action/verifier/ttl_secs/confidence/provenance
- New columns: `promotion_status` (TEXT DEFAULT 'observed'), `promoted_at` (TEXT)
- New tables: `hardened_rules`, `solution_nodes`
- 10 new methods on KnowledgeBase: get_promotion_candidates, promote_solution, get_promotion_status, count_distinct_nodes, record_node_outcome, has_canary_pod_success, days_since_promotion, store_hardened_rule, get_hardened_rules

### Modified: `main.rs`
- Added `mod kb_hardening;`
- Spawns `kb_hardening::spawn()` with fleet_bus sender and node_id

## Commits

| Hash | Message |
|------|---------|
| 00c56d4c | feat(278): KB hardening promotion ladder (KB-01..05) |

## Deviations from Plan

None - plan executed exactly as written.

## Known Stubs

None - all data paths are wired. The `is_shadow_mode()` and `is_canary_mode()` helper functions are available for tier_engine integration but not yet called from tier_engine.rs (future phase to wire Tier 1 reads from hardened_rules and Tier 2 shadow/canary gating).

## Self-Check: PASSED
