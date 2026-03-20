---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: planning
stopped_at: Completed 66-02-PLAN.md
last_updated: "2026-03-20T11:32:42.739Z"
last_activity: 2026-03-20 -- v11.0 roadmap created, 4 phases (71-74), 21 requirements mapped
progress:
  total_phases: 39
  completed_phases: 17
  total_plans: 44
  completed_plans: 41
  percent: 91
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-03-20)

**Core value:** Customers see their lap times, compete on leaderboards, and compare telemetry
**Current focus:** v11.0 Agent & Sentry Hardening -- Phase 71 ready to plan

## Current Position

Phase: 71 of 74 (rc-common Foundation + rc-sentry Core Hardening)
Plan: -- (not yet planned)
Status: Ready to plan
Last activity: 2026-03-20 -- v11.0 roadmap created, 4 phases (71-74), 21 requirements mapped

Progress: [█████████░] 91% (40/44 plans complete)

## Phase Map -- v11.0 Agent & Sentry Hardening

| Phase | Name | Requirements | Status |
|-------|------|--------------|--------|
| 71 | rc-common Foundation + rc-sentry Core Hardening | SHARED-01..03, SHARD-01..05 | Not started |
| 72 | rc-sentry Endpoint Expansion + Integration Tests | SEXP-01..04, SHARD-06, TEST-04 | Not started |
| 73 | Critical Business Tests | TEST-01, TEST-02, TEST-03 | Not started |
| 74 | rc-agent Decomposition | DECOMP-01..04 | Not started |

**Phase 71:** rc-common exec.rs with feature gate (SHARED) + rc-sentry timeout, truncation, concurrency cap, partial read fix, structured logging (SHARD). No rc-agent changes. Verify `cargo tree -p rc-sentry` shows no tokio after every rc-common change.
**Phase 72:** rc-sentry endpoint expansion (/health, /version, /files, /processes, graceful shutdown) + TcpStream-based integration tests on ephemeral port.
**Phase 73:** billing_guard + failure_monitor unit tests using watch channel injection / mockall; FfbBackend trait seam. MUST complete before Phase 74 (Refactor Second rule).
**Phase 74:** config.rs -> app_state.rs -> ws_handler.rs -> event_loop.rs extraction in strict dependency order. select! dispatch body (event_loop) deferred to v12.0 if risk is too high.

## Key Constraints for This Milestone

- rc-sentry MUST stay stdlib-only -- never add tokio. Run `cargo build --bin rc-sentry` after every rc-common change
- rc-common feature gate: default = sync (rc-sentry uses this), "async-exec" = tokio (rc-agent uses this)
- wait-timeout 0.2 is the only correct stdlib-compatible child process timeout on Windows
- mockall 0.13 goes in rc-agent dev-dependencies only (MSRV 1.77, project at 1.93.1)
- event_loop.rs extraction is the highest regression risk -- protect with Phase 73 tests first
- select! variable extraction uses ConnectionState struct pattern, never Arc<Mutex<T>> for local variables

## Performance Metrics

**Velocity (recent):**
- Phase 56 P01: 494 min | Phase 56 P02: 3 min | Phase 57 P01-03: ~35 min total
- Average recent plan: ~15 min

**Updated after each plan completion**

## Accumulated Context

### Decisions (v11.0)

- Build order: rc-common extraction -> rc-sentry hardening -> rc-agent decomposition (rc-common unblocks both sentry and agent)
- Phase 71 combines rc-common extraction + rc-sentry core hardening (they must be co-developed to validate the feature gate)
- Phase 73 (tests) precedes Phase 74 (decomposition) -- Refactor Second is a non-negotiable standing rule
- FfbBackend trait is the correct approach over #[cfg(test)] stubs -- cleaner seam, decided before writing any FFB tests
- SHARD-04 (partial TCP read fix) added to Phase 71 -- correctness issue distinct from timeout/truncation, flagged by research

### Blockers/Concerns

- Phase 73 billing_guard: `attempt_orphan_end` calls reqwest directly -- need to decide on trait-wrap vs callback param before writing tests. Callback param (option b) is simpler, avoid trait boilerplate.
- Phase 74 select! decomposition: enumerate all 14 mutable shared variables before first extraction step -- assign each to ConnectionState (inner loop) or ReconnectState (outer loop)
- v6.0 (Phases 36-40) still blocked on BIOS AMD-V -- does not affect v11.0

### Pending Todos

- Pod 3 still not verified running after fix-pod.bat -- needs physical reboot + verification
- Version string inconsistency: USB-installed pods report v0.1.0, HTTP-deployed report v0.5.2
- Clean up .planning/update_roadmap_v9.py after v9.0 (can delete)
- Clean up .planning/update_roadmap_v11.py after v11.0 roadmap creation (can delete)

## Session Continuity

Last session: 2026-03-20T11:32:42.735Z
Stopped at: Completed 66-02-PLAN.md
Resume file: None
Next action: Phase 71 -- run `/gsd:plan-phase 71`
