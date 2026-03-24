# Phase 176: Protocol Foundation + Cargo Gates - Context

**Gathered:** 2026-03-24
**Status:** Ready for planning

<domain>
## Phase Boundary

Lay the rc-common protocol types (7 new WS message variants + serde forward-compat), Cargo feature gate structure for rc-agent (ai-debugger, process-guard) and rc-sentry (watchdog, tier1-fixes, ai-diagnosis), and single-binary-tier policy documentation. Foundation phase — no runtime behavior changes.

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices are at Claude's discretion — pure infrastructure phase.

Key constraints from prior investigation:
- Telemetry excluded from feature gates (too entangled with billing/game state)
- #[serde(other)] catch-all must be added to AgentMessage + CoreToAgentMessage BEFORE new variants
- keyboard-hook feature gate pattern exists in kiosk.rs (function-level #[cfg]) but is dead code
- rc-sentry is pure std::net (no tokio) — feature gates work the same but gated modules are sync
- process-guard is cleanest boundary (single spawn() call, all state injected)
- ai-debugger is moderate (6 import points, but all guarded by .enabled checks)

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- Existing `keyboard-hook` feature gate pattern in `crates/rc-agent/src/kiosk.rs` (function-level + conditional re-exports)
- `crates/rc-common/src/protocol.rs` — 5 enums, all using `#[serde(tag = "type/event/command", content = "data")]`
- `crates/rc-common/src/types.rs` — shared data structs referenced by protocol enums

### Established Patterns
- Serde internally-tagged enums with content field for all protocol messages
- Module declarations in main.rs with conditional compilation via `#[cfg]`
- Config structs in config.rs with TOML deserialization
- Background tasks spawned via `tokio::spawn()` with channel-based communication

### Integration Points
- `crates/rc-common/src/protocol.rs` — add new enum variants + Unknown catch-all
- `crates/rc-agent/Cargo.toml` — add [features] section entries
- `crates/rc-agent/src/main.rs` — conditional `mod` declarations for ai_debugger, process_guard
- `crates/rc-agent/src/config.rs` — conditional config fields
- `crates/rc-agent/src/event_loop.rs` — conditional ai_debugger usage (11 import points)
- `crates/rc-agent/src/ws_handler.rs` — conditional ai_debugger usage (4 import points)
- `crates/rc-agent/src/failure_monitor.rs` — conditional ai_debugger usage (2 import points)
- `crates/rc-agent/src/self_monitor.rs` — conditional ai_debugger usage (1 import point)
- `crates/rc-sentry/Cargo.toml` — add [features] section entries
- `crates/rc-sentry/src/main.rs` — conditional `mod` declarations for watchdog, tier1_fixes, ollama, debug_memory

</code_context>

<specifics>
## Specific Ideas

No specific requirements — infrastructure phase.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>
