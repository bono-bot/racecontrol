# Phase 97: rc-common Protocol + pre_flight.rs Framework + Hardware Checks - Context

**Gathered:** 2026-03-21
**Status:** Ready for planning

<domain>
## Phase Boundary

Create the pre-flight check framework in rc-agent (new `pre_flight.rs` module) with concurrent check execution, add protocol variants to rc-common, implement the 3 hardware checks (HID wheelbase, ConspitLink two-stage, orphan game kill) with one auto-fix attempt each, and add the `disable_preflight` config flag.

</domain>

<decisions>
## Implementation Decisions

### Pre-flight runner design
- New module: `crates/rc-agent/src/pre_flight.rs`
- `pub async fn run(state: &AppState, ffb: &dyn FfbBackend) -> PreFlightResult`
- All checks run via `tokio::join!` with individual 2-second timeouts per check, 5-second hard timeout on the whole batch
- Each check returns `CheckResult { name: &str, status: CheckStatus, detail: String }`
- `CheckStatus`: Pass, Warn (non-blocking), Fail (blocking)
- On first Fail: attempt one auto-fix, re-run that specific check, if still Fail → PreFlightResult::MaintenanceRequired
- `PreFlightResult`: Pass (all checks pass/warn), MaintenanceRequired { failures: Vec<CheckResult> }

### Auto-fix strategy
- One fix attempt per failed check, no retry loop
- Safe fixes only: ConspitLink restart (spawn process), orphan game kill (PID-targeted)
- HID disconnected: no auto-fix possible (hardware), just report
- Auto-fix timeout: 3 seconds max per fix attempt
- After fix: re-run only the failed check, not all checks

### Protocol additions (rc-common)
- `AgentMessage::PreFlightFailed { pod_id: u32, failures: Vec<String>, timestamp: String }` — sent to racecontrol
- `AgentMessage::PreFlightPassed { pod_id: u32 }` — optional, for fleet health tracking
- `CoreToAgentMessage::ClearMaintenance` — server tells pod to exit MaintenanceRequired (Phase 98)

### Hardware checks
- **HW-01 (Wheelbase HID)**: Call `ffb.zero_force()` — returns `Ok(true)` = connected, `Ok(false)` = not found. No auto-fix (hardware).
- **HW-02 (ConspitLink)**: Two-stage: (1) `sysinfo::System::processes()` check for "ConspitLink.exe", (2) if running, verify `C:\ConspitLink\config.json` exists and is valid JSON. Status: both pass = Pass, process missing = Fail, config invalid = Warn.
- **HW-03 (ConspitLink auto-fix)**: If process missing, spawn `C:\ConspitLink\ConspitLink.exe` via `Command::new()`, wait 2s, re-check process list. If now running = Pass, still missing = Fail.
- **SYS-01 (Orphan game)**: Check `state.game_process` — if Some AND `state.billing_active` is false, `taskkill /F /PID {pid}`. Never name-based kill. Reset `state.game_process = None`.

### Config flag
- `[preflight]` section in rc-agent.toml: `enabled = true` (default), `disable_preflight = false`
- When disabled: BillingStarted proceeds directly, no pre_flight::run() call
- Serde default: enabled if section missing (backward compat with existing pods)

### Concurrency model
- Pre-flight runs via `tokio::spawn` from BillingStarted handler in ws_handler.rs
- Result communicated via oneshot channel back to the event loop
- WS receive loop is never blocked — billing ticks for other pods continue

### Claude's Discretion
- Exact tracing log format for pre-flight results
- Whether to include check durations in PreFlightResult
- Internal naming of check functions (check_hid, check_conspit, check_orphan_game, etc.)

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### rc-agent source (modify)
- `crates/rc-agent/src/ws_handler.rs` — BillingStarted handler where pre-flight gate is inserted
- `crates/rc-agent/src/event_loop.rs` — ConnectionState may need pre-flight result field
- `crates/rc-agent/src/config.rs` — Add PreflightConfig struct
- `crates/rc-agent/src/ffb_controller.rs` — FfbBackend trait used for HID check
- `crates/rc-agent/src/self_test.rs` — Reference for probe patterns (do NOT reuse run_all_probes)

### rc-common source (extend)
- `crates/rc-common/src/protocol.rs` — Add PreFlightFailed, PreFlightPassed, ClearMaintenance variants

### Research
- `.planning/research/PITFALLS.md` — 11 pitfalls including BillingStarted blocking, warm/cold semantics, safe-kill rules
- `.planning/research/ARCHITECTURE.md` — Integration points, gate location, build order
- `.planning/research/FEATURES.md` — Table stakes vs differentiators, auto-fix patterns

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `FfbBackend::zero_force()` — HID connectivity check (returns Ok(true)/Ok(false)/Err)
- `sysinfo::System::processes()` — already used in kiosk.rs and self_test.rs for process scanning
- `failure_monitor.rs` watch channel pattern — pre_flight can follow same tokio::spawn + watch approach
- `billing_guard.rs` mpsc channel — for sending PreFlightResult back to event loop

### Established Patterns
- `spawn_blocking` for Win32/sysinfo calls (from failure_monitor.rs)
- `AgentMessage` enum variants with pod_id field (from billing_guard.rs BillingAnomaly)
- Config sections with serde defaults (from AgentConfig existing pattern)

### Integration Points
- `ws_handler.rs` BillingStarted arm: insert pre_flight::run() call before show_active_session()
- `config.rs`: add PreflightConfig to AgentConfig struct
- `rc-common/protocol.rs`: add 3 new enum variants

</code_context>

<specifics>
## Specific Ideas

No specific requirements — user delegated all decisions to Claude's discretion. Decisions above reflect research recommendations and existing codebase patterns.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>

---

*Phase: 97-rc-common-protocol-pre-flight-rs-framework-hardware-checks*
*Context gathered: 2026-03-21*
