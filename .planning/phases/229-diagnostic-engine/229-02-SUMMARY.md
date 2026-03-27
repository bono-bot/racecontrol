---
phase: 229-diagnostic-engine
plan: "02"
status: complete
started: 2026-03-27T19:00:00+05:30
completed: 2026-03-27T19:30:00+05:30
---

## Summary

Created tier_engine.rs — the 5-tier decision tree that reads DiagnosticEvents from the diagnostic_engine channel and applies fixes or escalates.

### What was built

**Tier 1 (Deterministic) — fully implemented:**
- MAINTENANCE_MODE sentinel clear on every diagnostic event (proactive)
- WerFault/WerReport orphan process kill via sysinfo
- Stale sentinel file removal (FORCE_CLEAN, SAFE_MODE)
- Per-trigger actions (SentinelUnexpected → remove, ProcessCrash → log)

**Tiers 2-5 — stubs with descriptive logging:**
- Tier 2: KB lookup stub → Phase 230
- Tier 3: Qwen3 single-model → Phase 231
- Tier 4: 4-model parallel → Phase 231
- Tier 5: WhatsApp escalation → Phase 231

**Wiring:**
- `mod tier_engine` added to main.rs
- `_diagnostic_event_rx` renamed to `diagnostic_event_rx`
- `tier_engine::spawn(diagnostic_event_rx)` wired after diagnostic_engine::spawn

### Key files

| File | Action | Lines |
|------|--------|-------|
| crates/rc-agent/src/tier_engine.rs | Created | 281 |
| crates/rc-agent/src/main.rs | Modified | +7/-1 |

### Requirements covered

| ID | Description | Status |
|----|-------------|--------|
| DIAG-02 | Tier 1 deterministic fixes | Implemented |
| DIAG-03 | Tier 2 KB lookup | Stub |
| DIAG-04 | Tier 3 single-model | Stub |
| DIAG-05 | Tier 4 4-model parallel | Stub |
| DIAG-06 | Tier 5 human escalation | Stub |

### Commits

| Hash | Description |
|------|-------------|
| a9fa050a | feat(229-02): create tier_engine.rs |
| 9e3951a1 | feat(229-02): wire tier_engine::spawn() into main.rs |

### Self-Check

- [x] cargo check -p rc-agent-crate passes
- [x] No .unwrap() in tier_engine.rs
- [x] Lifecycle logs present (started, first_event_processed)
- [x] All 9 DiagnosticTrigger variants handled in match (no wildcard)
- [x] TierResult used 21 times across the module
- [x] Phase 230/231 references in all stubs (17 occurrences)
