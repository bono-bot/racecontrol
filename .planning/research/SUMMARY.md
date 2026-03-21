# Project Research Summary

**Project:** v11.1 Pre-Flight Session Checks
**Domain:** rc-agent Windows service — pre-session health gate for sim racing kiosk pods
**Researched:** 2026-03-21
**Confidence:** HIGH

## Executive Summary

The v11.1 pre-flight gate is a well-scoped addition to an existing, mature codebase. Every capability required — Win32 window checks, process scanning, HID enumeration, disk/memory probes, WebSocket messaging, and async concurrency — is already compiled into rc-agent via existing dependencies. Zero new Rust crates are needed. The primary research conclusion is that this feature is an integration exercise, not a greenfield build. The correct implementation is a single new module (`pre_flight.rs`) containing all check logic, called from the existing `BillingStarted` arm in `ws_handler.rs`, with reused probe helpers extracted from `self_test.rs`.

The recommended approach is a concurrent check gate (`tokio::join!` over 8-10 targeted checks) with a hard 5-second total timeout, auto-fix attempted once per failure before alerting, and a new `MaintenanceRequired` lock screen state that blocks the pod and notifies staff via WebSocket. The gate fires on `BillingStarted` only — not on a cron or timer — making it a true per-session health check. The MVP covers the failure modes that actually bite operations: dead wheelbases, orphaned games, stuck billing sessions, and dead WebSocket connections. The build order is dictated by compile-time dependencies: `rc-common` protocol changes first, then lock screen state, then probe helpers, then the new module, then the handler integration.

The critical risks are all concurrency and state machine related, not algorithmic. Blocking the WebSocket receive loop during checks would stall billing ticks. A `MaintenanceRequired` state without exit transitions would permanently lock pods. Auto-fix killing games by process name instead of PID could disrupt active sessions. A notification flood from repeated check failures would train staff to ignore alerts. All four risks have clear preventions documented at the code-pattern level in the research.

---

## Key Findings

### Recommended Stack

The entire pre-flight feature is built on the existing dependency graph. No changes to `rc-agent/Cargo.toml` are required. The only dependency change is a source-level one: add new `AgentMessage` variants to `rc-common/src/protocol.rs`.

**Core technologies:**
- `tokio::join!` — concurrent check execution — already the runtime, no new dep
- `winapi 0.3` (`winuser` feature) — `FindWindowW` + `GetWindowRect` for display validation — already in Cargo.toml line 62
- `sysinfo 0.33` — process scan (ConspitLink, orphaned game) and disk/memory probes — already used in `kiosk.rs`, `self_test.rs`, `game_process.rs`
- `hidapi 2` — HID enumeration for wheelbase presence check — already used in `ffb_controller.rs`
- `tokio-tungstenite 0.26` — WebSocket message to racecontrol on pre-flight failure — already the WS client
- `reqwest 0.12` — HTTP orphan-end auto-fix call — already present for `billing_guard.rs`

See STACK.md for full rationale and confirmed Cargo.toml line references.

### Expected Features

The feature set is grounded in codebase audit of existing state, not aspirational requirements. All MVP checks read from state that already exists in `AppState` or from patterns already in `self_test.rs`.

**Must have (table stakes — v11.1 core):**
- BillingStarted hook intercepts session lifecycle before `show_active_session()` — the single correct gate point
- WebSocket connected check (hard block) — `HeartbeatStatus.ws_connected` atomic read
- HID wheelbase present check with one HID rescan auto-fix attempt (hard block if rescan fails)
- No orphaned game process check with PID-targeted kill auto-fix (hard block if kill fails)
- Billing clear check — no stuck session from previous customer — local atomic, not HTTP query
- Disk space >1GB free on C: (hard block) — reuse `self_test.rs probe_disk()` logic
- Memory >2GB free (soft warn, do not block) — reuse `self_test.rs probe_memory()` logic
- ConspitLink process running with spawn auto-fix (hard block if spawn fails)
- Auto-fix attempted before any staff alert
- `MaintenanceRequired` lock screen state on unresolvable failure
- `PreFlightFailed` AgentMessage to racecontrol server on hard block

**Should have (v11.1 polish, add after core proven stable):**
- UDP heartbeat soft-warn (low signal before any session has run)
- Structured `PreFlightResult` with per-check status for dashboard visibility
- Overlay TCP port check with restart auto-fix

**Defer to v11.2+:**
- Configurable hard/soft policy per check via `[preflight]` toml config section
- Fleet dashboard pre-flight field (requires Next.js frontend work)
- Kiosk dashboard badge on pre-flight failure (requires server pod state + Next.js changes)
- AC content directory existence check
- CLOSE_WAIT pre-session cleanup

**Anti-features (do not build):**
- GPU temperature check — varies legitimately 30-80C, not a session gate
- Full 22-probe self_test run on every BillingStarted — 10+ second latency, wrong scope
- Customer-visible technical error messages — show only "Maintenance Required — Staff Notified"
- Per-check retry loops — auto-fix attempts once, then alerts immediately
- Screenshot-based display validation — Session 0 cannot capture Session 1 display; use HTTP probe to `:18923` instead

See FEATURES.md for full prioritization matrix and feature dependency graph.

### Architecture Approach

The pre-flight gate slots into the existing `ws_handler.rs` `BillingStarted` match arm — the only correct insertion point. The new module (`pre_flight.rs`) is self-contained with a single public function `run(&state, &config) -> PreFlightResult`. All checks run concurrently with `tokio::join!`; auto-fixes run sequentially after checks complete to prevent interference. The `MaintenanceRequired` lock screen state follows the exact same enum-variant pattern as the existing 13 states. Protocol changes (two new `AgentMessage` variants, one new `CoreToAgentMessage`) live in `rc-common`.

**Major components:**
1. `pre_flight.rs` (new) — owns all check logic, `PreFlightResult` enum, `CheckResult` struct, concurrent execution via `tokio::join!`, sequential auto-fix dispatch
2. `lock_screen.rs` (modify) — add `MaintenanceRequired { reasons: Vec<String> }` variant and `show_maintenance_required()` method
3. `ws_handler.rs` (modify) — insert `pre_flight::run().await` at top of `BillingStarted` arm; handle new `ClearMaintenance` server message
4. `rc-common/protocol.rs` (modify) — add `PreFlightFailed`, `PreFlightPassed`, `ClearMaintenance` message variants
5. `self_test.rs` (modify, preparatory) — extract `pub(crate)` helper functions: `check_hid_device()`, `available_disk_gb()`, `available_memory_gb()`

**Suggested build order (compiler-dependency-driven):**
1. `rc-common/protocol.rs` — shared lib, must exist before rc-agent code that references it compiles
2. `lock_screen.rs` — `MaintenanceRequired` state must exist before `pre_flight.rs` calls `show_maintenance_required()`
3. `self_test.rs` — extract `pub(crate)` helpers (preparatory, no behavior change)
4. `pre_flight.rs` — new module, all check logic
5. `ws_handler.rs` — gate call + `ClearMaintenance` handler
6. `main.rs` — add `mod pre_flight;`
7. racecontrol server — handle `PreFlightFailed`, kiosk dashboard pod state

See ARCHITECTURE.md for data flow diagrams, pass/fail paths, and anti-patterns.

### Critical Pitfalls

1. **Blocking the WS receive loop with inline `await`** — serial awaits inside `BillingStarted` arm stall BillingTick processing. Prevention: use `tokio::join!` inside `pre_flight::run()` for concurrent reads; wrap the entire gate in `tokio::time::timeout(Duration::from_secs(5), ...)`.

2. **Auto-fix killing game processes by name not PID** — `taskkill /F /IM acs.exe` kills any matching process without verifying it belongs to the current pod's orphan state. Prevention: cross-check `state.game_process` before any kill; only kill PIDs not tracked by the agent; use `taskkill /F /PID <pid>`.

3. **`MaintenanceRequired` state with no exit path** — pods stay blocked until manual restart. Prevention: two explicit exit transitions required at design time: (a) `ClearMaintenance` server command handler, (b) 30-second auto-retry background task that self-clears if all checks pass.

4. **Self-test probe semantics are wrong for warm-system context** — `self_test.rs` probes assume cold boot. UDP port bound at startup means no game running (pass). UDP port bound between sessions means orphan socket (fail). Prevention: `pre_flight.rs` must be a separate module with inverted probe semantics; never call `self_test::run_all_probes()` from pre-flight.

5. **Staff notification flood on repeated check failures** — 8 `BillingStarted` calls during pre-opening pod testing = 8 identical WhatsApp alerts in 10 minutes. Prevention: `MaintenanceRequired` state is the deduplication gate — only alert on transition into `MaintenanceRequired`, not on every re-check failure.

6. **`MaintenanceRequired` breaks server pod reservation** — racecontrol continues booking the pod, sends another `BillingStarted`, illegal state transition occurs. Prevention: dual guard required — server marks pod unavailable on `PreFlightFailed`; agent checks lock screen state at top of `BillingStarted` arm and rejects new sessions while in `MaintenanceRequired`.

See PITFALLS.md for all 11 pitfalls with code-level prevention guidance, integration gotchas, and a "Looks Done But Isn't" verification checklist.

---

## Implications for Roadmap

Based on research, suggested phase structure:

### Phase 1: Pre-Flight Framework + Hardware Checks
**Rationale:** The concurrency model, timeout budget, `PreFlightResult` type, and safe-kill rules are foundational — every subsequent check inherits these decisions. Getting the concurrency model wrong means every check is wrong. Hardware checks (HID, ConspitLink) are the highest-value checks and must be implemented alongside the framework to validate the approach on the most complex cases first.
**Delivers:** `pre_flight.rs` module with `run()` function, `PreFlightResult` enum, `CheckResult` struct, concurrent `tokio::join!` gate, 5-second timeout, HID check, ConspitLink process+HID liveness (two-stage), orphan game check with PID-targeted kill, safe-kill rules verified against `state.game_process`. Also: `disable_preflight` boolean in `AgentConfig` as day-one rollback option.
**Addresses:** TS-4, TS-5, TS-9, TS-10 from FEATURES.md.
**Avoids:** Pitfall 1 (WS loop blocking), Pitfall 2 (wrong game kill), Pitfall 4 (wrong probe semantics), Pitfall 6 (ConspitLink hung check), Pitfall 7 (5s budget), Pitfall 11 (PID-targeted kill).

### Phase 2: Lock Screen Integration + Protocol
**Rationale:** Lock screen state and protocol messages are the output channel of the gate. They must compile before the gate is wired into `ws_handler.rs`. Both exit paths from `MaintenanceRequired` (staff clear command and auto-retry) must be designed alongside the state addition — not as a follow-up. Server-side pod reservation changes must be in this phase, not deferred, or Pitfall 9 (booking a blocked pod) will be live in production.
**Delivers:** `MaintenanceRequired` lock screen state with `show_maintenance_required()` method and HTML template. `ClearMaintenance` server command handler in `ws_handler.rs`. `PreFlightFailed` / `PreFlightPassed` / `ClearMaintenance` variants in `rc-common/protocol.rs`. 30-second auto-retry background task. racecontrol marks pod unavailable on `PreFlightFailed`.
**Addresses:** TS-11, TS-12 from FEATURES.md.
**Avoids:** Pitfall 3 (no exit path), Pitfall 9 (server pod state breakage), Pitfall 10 (screenshot-based display check).

### Phase 3: Billing + System Checks + Handler Integration
**Rationale:** Billing and system checks (disk, memory, WS, billing clear) are simpler read-only checks with no auto-fix complexity. They belong after the framework is proven on the harder checks. Handler integration (`ws_handler.rs` wiring) is the final step so that a partially-working pre-flight never reaches pods — integration happens only when all checks are ready.
**Delivers:** Billing stuck-session check (local atomic, not HTTP query), disk space check, memory check, WS connected check. `pre_flight::run()` wired into `ws_handler.rs` `BillingStarted` arm. `self_test.rs` `pub(crate)` helpers extracted. `main.rs` `mod pre_flight;` declaration. WhatsApp alerter deduplication (one alert per `MaintenanceRequired` entry, not per failure).
**Addresses:** TS-1, TS-2, TS-6, TS-7, TS-8 from FEATURES.md.
**Avoids:** Pitfall 5 (notification flood), Pitfall 8 (billing check racing session cleanup via local atomic).

### Phase 4: Server-Side Staff UX
**Rationale:** racecontrol changes are decoupled from the agent. Unknown `AgentMessage` variants are gracefully ignored by the existing server during the deployment window, so the agent can be deployed first without breakage. This phase completes the operational visibility story.
**Delivers:** Kiosk dashboard maintenance badge (Racing Red `#E10600`) per pod. "Clear Maintenance" button in kiosk dashboard (staff PIN-gated). Maintenance failure details hidden from customer-visible kiosk displays. `preflight_alert_cooldown_secs` config field in racecontrol.
**Addresses:** D-4 (kiosk badge), D-3 (fleet dashboard pre-flight field — MVP version).
**Avoids:** UX pitfall (maintenance details visible to customers on TV-visible dashboard).

### Phase Ordering Rationale

- Phase 1 first because the concurrency model and safe-kill rules are foundational — wrong here means wrong everywhere.
- Phase 2 before Phase 3 because `lock_screen.rs` and `rc-common/protocol.rs` changes must compile before `ws_handler.rs` integration builds. This is a hard compiler dependency, not a preference.
- Phase 3 integrates the handler last so pods are never running a partially-wired gate. The gate goes live as a complete system.
- Phase 4 last and decoupled because racecontrol can gracefully ignore `PreFlightFailed` messages until the server upgrade. Pod-side changes are independently deployable.

### Research Flags

All implementation patterns are known from direct codebase inspection. No additional research phases are needed before implementation. The following verification steps should be confirmed during each phase:

- **Phase 1:** Stopwatch test — BillingStarted to ActiveSession under 5s on a healthy pod. BillingTick messages arrive normally during gate execution (no dropped ticks in logs). Safe-kill test — manually set `game_process = None` in test, verify gate does NOT kill the running game.
- **Phase 2:** Manual test — trigger a failure, fix it, confirm pod auto-clears within 60 seconds without restart. Confirm racecontrol marks pod unavailable and kiosk blocks booking.
- **Phase 3:** False positive test — run pre-flight on healthy pod 20 consecutive times, zero failures. Confirm billing check uses local atomic, not HTTP query.
- **Phase 4:** Confirm maintenance badge requires staff PIN to view failure details. Confirm "Clear Maintenance" button transitions pod back to available.

Phases with well-established patterns (no additional research needed):
- **All phases** — confidence is HIGH across all research files. Implementation patterns are directly sourced from the existing codebase. No novel algorithms or external service integrations.

---

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | Every capability confirmed in Cargo.toml and existing source files. Zero new crates. All dep versions verified from actual Cargo.toml. |
| Features | HIGH | Sourced from direct codebase audit of ws_handler.rs, self_test.rs, billing_guard.rs, failure_monitor.rs. MVP feature set exactly matches existing AppState structure. |
| Architecture | HIGH | Build order derived from compiler dependency graph. Module boundaries sourced from inspection of all named files. Anti-patterns confirmed against existing codebase patterns. |
| Pitfalls | HIGH | All 11 pitfalls grounded in actual rc-agent source code patterns with exact code-level prevention guidance. |

**Overall confidence:** HIGH

### Gaps to Address

- **Display validation check composite definition** — STACK.md confirms `GetWindowRect` is the correct approach. PITFALLS.md confirms screenshot is wrong (Session 0/1 boundary). ARCHITECTURE.md recommends composite check: HTTP probe to `:18923` + Edge process alive check + window rect check. The exact combination to use is a detail-level implementation decision. Recommend HTTP probe as primary (confirms server serving), Edge process check as secondary, `GetWindowRect` as tertiary. Resolve during Phase 2 implementation.

- **ConspitLink liveness limitation** — two-stage check (process exists + HID enumeration returns the OpenFFBoard VID:0x1209 PID:0xFFB0 device) is the best achievable signal without a ConspitLink health API. A hung ConspitLink that holds the HID handle cannot be detected by enumeration alone. Accept this limitation and document it. If ConspitLink hangs become a recurring issue, add a ConspitLink health TCP port in a future milestone.

- **racecontrol pod reservation scope** — Phase 4 requires changes to racecontrol's pod reservation system. The research focused on rc-agent. Before Phase 4 implementation begins, read `crates/racecontrol/src/` pod reservation code to scope the server-side changes accurately.

- **`disable_preflight` config flag** — PITFALLS.md recovery strategies flag this as a day-one necessity. The flag is not in the current MVP feature list. Recommend adding to Phase 1 scope as a one-line boolean in `AgentConfig` plus an early-return guard in `pre_flight::run()`. Gives Uday a rollback without a redeploy.

---

## Sources

### Primary (HIGH confidence — direct source inspection)

- `crates/rc-agent/src/ws_handler.rs` — BillingStarted dispatch, handle_ws_message signature, critical path structure
- `crates/rc-agent/src/self_test.rs` — 22 probes, ProbeResult/SelfTestReport types, reusable probe functions, timeout patterns
- `crates/rc-agent/src/lock_screen.rs` — LockScreenState enum (13 states), show_* methods, get_virtual_screen_bounds()
- `crates/rc-agent/src/app_state.rs` — AppState 34 fields, hid_detected, game_process, failure_monitor_tx
- `crates/rc-agent/src/failure_monitor.rs` — FailureMonitorState, watch channel pattern, debounce patterns
- `crates/rc-agent/src/billing_guard.rs` — attempt_orphan_end(), suppression gate patterns (recovery_in_progress)
- `crates/rc-agent/src/game_process.rs` — orphan game process detection pattern
- `crates/rc-agent/src/kiosk.rs` — sysinfo process scan in spawn_blocking, enforce_process_whitelist_blocking
- `crates/rc-agent/src/ai_debugger.rs` — try_auto_fix() pattern, canonical keyword dispatch
- `crates/rc-agent/Cargo.toml` — confirmed winuser + wingdi features (line 62), all dependency versions

### Secondary (MEDIUM confidence)

- [winapi 0.3 docs — GetWindowRect](https://docs.rs/winapi/latest/winapi/um/winuser/fn.GetWindowRect.html) — confirmed in winuser module
- [Rust forum — GetWindowRect window position](https://users.rust-lang.org/t/how-to-get-window-position-and-size-of-a-different-process/79224) — consistent with official docs

---

*Research completed: 2026-03-21 IST*
*Ready for roadmap: yes*
