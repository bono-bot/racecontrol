---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: completed
stopped_at: Completed 76-03-PLAN.md
last_updated: "2026-03-20T12:55:56Z"
last_activity: "2026-03-20 -- 76-03 complete: service key middleware on rc-agent protected routes"
progress:
  total_phases: 45
  completed_phases: 19
  total_plans: 50
  completed_plans: 48
  percent: 94
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-03-20)

**Core value:** Customers see their lap times, compete on leaderboards, and compare telemetry
**Current focus:** v12.0 Security Audit & Hardening -- Phase 76 in progress (3/6 plans complete)

## Current Position

Phase: 76 of 80 (API Authentication & Admin Protection)
Plan: 03 of 6 complete
Status: In progress
Last activity: 2026-03-20 -- 76-03 complete: service key middleware on rc-agent protected routes

Progress: [█████████░] 94% (48/50 plans complete)

## Phase Map -- v11.0 Agent & Sentry Hardening

| Phase | Name | Requirements | Status |
|-------|------|--------------|--------|
| 71 | rc-common Foundation + rc-sentry Core Hardening | SHARED-01..03, SHARD-01..05 | Complete (2/2 plans done) |
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
- 71-01: wait-timeout = 0.2 for stdlib-only child process timeout; tokio optional dep with feature gate prevents rc-sentry contamination (SHARED-01..03 complete)
- 71-01: Truncation on Vec<u8> before String::from_utf8_lossy to prevent char boundary panics in exec.rs
- 75-01: 269 racecontrol routes classified into 5 tiers; 172 staff/admin routes have zero auth (CRITICAL); OTP plaintext in logs elevated to CRITICAL
- 75-01: rc-agent /exec and rc-sentry TCP flagged CRITICAL -- arbitrary command execution with zero auth on LAN
- 71-02: SlotGuard Drop impl ensures EXEC_SLOTS decremented even on panic -- prevents 429 lockout (SHARD-01..05 complete)
- 71-02: THREAD_COUNTER separate from EXEC_SLOTS -- EXEC_SLOTS=live connections, THREAD_COUNTER=monotonic spawn IDs
- 75-02: rand 0.8 thread_rng().r#gen() for JWT key gen (gen is Rust 2024 reserved keyword); RACECONTROL_* env var naming for all secrets
- 75-02: default_jwt_secret() kept for serde backward compat; resolve_jwt_secret() catches dangerous default at runtime
- 76-03: subtle crate for constant-time service key comparison on rc-agent; permissive mode when RCAGENT_SERVICE_KEY unset; /ping and /health remain public
- 66-05: INFRA-01 complete via static IP alone -- TP-Link EX220 firmware bug (Error 5024) persists ARP entries in NVRAM across reboots, permanently blocking DHCP reservation for server .23; reservation is "won't fix" for this router model, add if factory-reset or replaced
- 66-05: Bono deployment (exec round-trip) deferred async via INBOX.md; INFRA-03 code complete on both sides, live verification pending Bono pm2 restart

### Blockers/Concerns

- Phase 73 billing_guard: `attempt_orphan_end` calls reqwest directly -- need to decide on trait-wrap vs callback param before writing tests. Callback param (option b) is simpler, avoid trait boilerplate.
- Phase 74 select! decomposition: enumerate all 14 mutable shared variables before first extraction step -- assign each to ConnectionState (inner loop) or ReconnectState (outer loop)
- v6.0 (Phases 36-40) still blocked on BIOS AMD-V -- does not affect v11.0
- 66-05: exec round-trip (INFRA-03) pending Bono deployment -- Bono notified via INBOX.md commits 3e4091a + 35cea4f, will self-verify once Bono pulls + restarts pm2

### Pending Todos

- Pod 3 still not verified running after fix-pod.bat -- needs physical reboot + verification
- Version string inconsistency: USB-installed pods report v0.1.0, HTTP-deployed report v0.5.2
- Clean up .planning/update_roadmap_v9.py after v9.0 (can delete)
- Clean up .planning/update_roadmap_v11.py after v11.0 roadmap creation (can delete)

## Session Continuity

Last session: 2026-03-20T12:55:56Z
Stopped at: Completed 76-03-PLAN.md
Resume file: None
Next action: Continue Phase 76 -- plan 04 next
