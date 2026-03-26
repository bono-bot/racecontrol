# Phase 208: Chain Verification Integration - Context

**Gathered:** 2026-03-26
**Status:** Ready for planning

<domain>
## Phase Boundary

Wrap the 4 critical parse/transform chains with VerificationChain so that each logs intermediate step values. A failing chain produces a log line naming the exact step and raw value that failed, not just a downstream symptom. Covers: pod healer curl parse, config TOML load, allowlist enforcement, and rc-sentry spawn verification.

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices are at Claude's discretion — pure infrastructure phase. Key constraints from requirements:
- Pod healer curl→stdout→u32 chain wrapped with ColdVerificationChain (COV-02) — logs raw value including quotes on parse failure
- Config→URL load chain wrapped with ColdVerificationChain (COV-03) — logs first 3 lines of file on TOML parse failure, VerificationError::TransformError on fallback-to-default
- Allowlist→enforcement chain wrapped with ColdVerificationChain (COV-04) — VerificationError::InputParseError on empty allowlist with guard enabled, auto-switch to report_only
- spawn()→child verification chain (COV-05) — 500ms PID liveness check + 10s health endpoint poll after spawn().is_ok(), VerificationError::ActionError on failure, retry spawn
- rc-sentry is sync-only (no tokio) — spawn verification must use std::thread::sleep, not async
- ColdVerificationChain and VerificationError from rc-common/src/verification.rs (Phase 205)

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `rc-common/src/verification.rs` — ColdVerificationChain, VerifyStep trait, VerificationError enum (Phase 205)
- `crates/racecontrol/src/pod_healer.rs` — pod health check curl exec chain
- `crates/racecontrol/src/config.rs` — load_or_default() with TOML parsing
- `crates/rc-agent/src/process_guard.rs` — allowlist fetch + enforcement loop
- `crates/rc-agent/src/main.rs` — config loading with fallback sites
- `crates/rc-sentry/src/watchdog.rs` — spawn + FSM transitions
- `crates/rc-sentry/src/session1_spawn.rs` — process spawning

### Established Patterns
- tracing::warn! with structured fields for state transitions
- eprintln! for pre-tracing errors
- Process guard already has OBS-03 empty allowlist auto-switch (Phase 206)

### Integration Points
- pod_healer.rs — wrap curl exec result parsing
- config.rs (racecontrol) — wrap TOML load chain
- process_guard.rs — wrap allowlist fetch+validate chain
- watchdog.rs + session1_spawn.rs — wrap spawn→verify chain

</code_context>

<specifics>
## Specific Ideas

- The verification chain wrapping should be non-invasive — add chain logging at existing parse/transform call sites without changing the control flow
- For COV-05, rc-sentry has no HTTP client — PID liveness check via `tasklist /FI "PID eq {pid}"` is sufficient, health endpoint poll can use std::net::TcpStream or a simple HTTP GET via std::io

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>
