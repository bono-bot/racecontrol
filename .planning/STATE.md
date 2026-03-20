---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: executing
stopped_at: Completed 73-01-PLAN.md
last_updated: "2026-03-20T13:29:55.443Z"
last_activity: "2026-03-20 -- 73-01 complete: FfbBackend trait seam + mockall tests (TEST-03)"
progress:
  total_phases: 45
  completed_phases: 22
  total_plans: 62
  completed_plans: 57
  percent: 92
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-03-20)

**Core value:** Customers see their lap times, compete on leaderboards, and compare telemetry
**Current focus:** v12.0 Security Audit & Hardening -- Phase 76 in progress (4/6 plans complete)

## Current Position

Phase: 73 of 80 (Critical Business Tests)
Plan: 01 complete; 02 next
Status: In progress
Last activity: 2026-03-20 -- 73-01 complete: FfbBackend trait seam + mockall tests (TEST-03)

Progress: [█████████░] 92% (57/62 plans complete)

## Phase Map -- v11.0 Agent & Sentry Hardening

| Phase | Name | Requirements | Status |
|-------|------|--------------|--------|
| 71 | rc-common Foundation + rc-sentry Core Hardening | SHARED-01..03, SHARD-01..05 | Complete (2/2 plans done) |
| 72 | rc-sentry Endpoint Expansion + Integration Tests | SEXP-01..04, SHARD-06, TEST-04 | Complete (2/2 plans done) |
| 73 | Critical Business Tests | TEST-01, TEST-02, TEST-03 | In progress (1/? plans done) |
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
- 72-01: build.rs copied from rc-agent for GIT_HASH embedding; winapi 0.3 consoleapi-only for SetConsoleCtrlHandler; non-blocking accept loop polls SHUTDOWN_REQUESTED every 10ms for graceful drain (SEXP-01..04, SHARD-06 complete)
- 72-02: inline #[cfg(test)] module with incoming().take(N) for clean thread exit; ephemeral ports via 127.0.0.1:0; all 7 tests pass with zero tokio contamination (TEST-04 complete)
- 66-05: Bono deployment (exec round-trip) deferred async via INBOX.md; INFRA-03 code complete on both sides, live verification pending Bono pm2 restart
- 67-01: Allowlist approach for sanitizer (only venue/pods/branding) -- never denylist; httpPost used for relay/sync POST for consistency; RACECONTROL_TOML_PATH env var for configurable path (SYNC-01, SYNC-02 complete)
- 67-02: parse_config_snapshot extracted as pub(crate) fn for testability -- sync_push calls it rather than inlining (SYNC-03 complete)
- 67-02: config_snapshot uses total += 1 (single record semantics, not per-field) -- consistent with other upserts
- 67-02: Structured tracing on config_snapshot receipt: venue name, pod count, hash prefix (first 8 chars)
- 76-02: argon2 0.5 with Argon2id default params for admin PIN hashing; spawn_blocking for CPU-heavy verification; 503 when no hash configured; 12h JWT expiry (shift-length limit)
- 76-04: tower_governor 0.8 with PeerIpKeyExtractor for per-IP rate limiting; into_make_service_with_connect_info for ConnectInfo; SQLx transaction wraps validate_pin token lifecycle
- 76-04: Bot wallet check (AUTH-05) already existed; billing is deferred (in-memory), not DB -- TOCTOU mitigated by optimistic locking
- 73-01: FfbBackend trait uses FfbController::method(self) fully-qualified delegation to avoid infinite recursion when trait and inherent method names match; mockall mock tests added inside existing test module; tokio test-util added to dev-deps to fix pre-existing billing_guard compilation (TEST-03 complete)

### Blockers/Concerns

- Phase 73 billing_guard: `attempt_orphan_end` calls reqwest directly -- need to decide on trait-wrap vs callback param before writing tests. Callback param (option b) is simpler, avoid trait boilerplate.
- Phase 74 select! decomposition: enumerate all 14 mutable shared variables before first extraction step -- assign each to ConnectionState (inner loop) or ReconnectState (outer loop)
- v6.0 (Phases 36-40) still blocked on BIOS AMD-V -- does not affect v11.0
- 66-05: exec round-trip (INFRA-03) pending Bono deployment -- Bono notified via INBOX.md commits 3e4091a + 35cea4f, will self-verify once Bono pulls + restarts pm2
- 76-01: Permissive mode for initial staff JWT deploy -- logs unauthenticated requests without rejecting (expand-migrate-contract pattern)
- 76-01: StaffClaims uses role="staff" field -- customer JWTs lacking role field are auto-rejected by deserialization
- 76-01: api_routes() split into 4 tiers (public/customer/staff/service) with state parameter for middleware

### Pending Todos

- Pod 3 still not verified running after fix-pod.bat -- needs physical reboot + verification
- Version string inconsistency: USB-installed pods report v0.1.0, HTTP-deployed report v0.5.2
- Clean up .planning/update_roadmap_v9.py after v9.0 (can delete)
- Clean up .planning/update_roadmap_v11.py after v11.0 roadmap creation (can delete)

## Session Continuity

Last session: 2026-03-20T13:26:57.055Z
Stopped at: Completed 73-01-PLAN.md
Resume file: None
Next action: Continue Phase 76 -- API Authentication & Admin Protection (plan 04 next)
