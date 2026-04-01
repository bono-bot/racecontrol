---
phase: 291-kb-promotion-persistence
plan: 02
subsystem: tier-engine
tags: [tier-engine, hardened-rules, zero-cost, kbpp-05]

# Dependency graph
requires:
  - "knowledge_base.rs: HardenedRule struct + get_hardened_rules()"
  - "tier_engine.rs: TierResult, DiagnosticEvent, run_tiers(), staff diagnostic flow"
provides:
  - "tier0_hardened_rule() — zero-cost hardened rule lookup"
  - "Tier 0 wired into run_tiers() autonomous flow (before mma_decision)"
  - "Tier 0 wired into staff diagnostic flow (after tier2_kb_lookup)"
  - "fix_type='hardened_rule' in staff result on Tier 0 match"
  - "tier0_tests module with 4 tests"

# Tech tracking
tech-stack:
  patterns:
    - "tier0_hardened_rule() uses KnowledgeBase::open(KB_PATH) per-call (same as tier2_kb_lookup)"
    - "Test helper isolates matching logic from DB for unit tests"
    - "No .unwrap() in production code"

key-files:
  created: []
  modified:
    - "crates/rc-agent/src/tier_engine.rs"

key-decisions:
  - "Tier 0 opens KnowledgeBase per-call — consistent with existing tier2_kb_lookup pattern"
  - "fix_type='hardened_rule' distinguishes from Tier 2 'kb_lookup'"
  - "Staff flow: Tier 0 after tier2_kb_lookup (not before Tier 1 — deterministic always first)"

metrics:
  duration_minutes: 5
  completed_date: "2026-04-01"
  tasks_completed: 1
  files_created: 0
  files_modified: 1
---

# Phase 291 Plan 02: Tier 0 Hardened Rule Lookup Summary

**One-liner:** tier0_hardened_rule() added to tier_engine.rs — zero-cost hardened rule lookup before any model API calls in both autonomous and staff diagnostic flows (KBPP-05).

## Commits

- `e767d69f`: feat(291-02): tier0_hardened_rule() — zero-cost hardened rule lookup before model tiers

## Deviations from Plan

None — plan executed exactly as written.

## Self-Check: PASSED

- `tier0_hardened_rule` — 8 occurrences in tier_engine.rs (definition + call sites + tests)
- `"Tier 0 hardened rule match — $0 cost"` log line present
- `fix_type: "hardened_rule"` in staff flow
- `get_hardened_rules` called
- Commit `e767d69f` — EXISTS
