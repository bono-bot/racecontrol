# Phase 26: Lap Filter, PIN Security, Telemetry + Multiplayer - Context

**Gathered:** 2026-03-16
**Status:** Ready for planning

<domain>
## Phase Boundary

Seven requirements across four domains:
- **LAP-01/02/03**: Wire AC is_valid from shared memory into persist_lap; add per-track min lap time configurable from admin; classify laps as hotlap vs practice from AC session type
- **PIN-01/02**: Add separate failed-attempt counters for customer and staff PINs; customer counter cannot affect staff lockout
- **TELEM-01**: Alert staff (email + dashboard flag) when UDP silent >60s during active billing + DrivingState::Active
- **MULTI-01**: Detect AC multiplayer server disconnect via shared memory status field; fully auto-teardown all pods in the session

Lap hard-delete and multiplayer auto-rejoin remain out of scope (documented anti-features).

</domain>

<decisions>
## Implementation Decisions

### Lap validity (LAP-01)
- **AC only** — F1 25 adapter deferred until F1 sees real use at the venue
- **Verify + fix shared memory offset** — current offset 180 is marked "approximate, may need correction" in assetto_corsa.rs; researcher must confirm correct offset from AC SDK/community tables before planning touches that constant
- `persist_lap` pipeline already has the gate (`if !lap.valid { return }`) and `valid` column in the INSERT — only the AC adapter population is missing

### Per-track min lap time (LAP-02)
- **`min_lap_time_ms` column added to `kiosk_experiences` table** — not a separate table; same row as the experience config
- Initial seed values for Monza, Silverstone, Spa (as specified in requirement); NULL for other tracks means no floor
- Admin panel CRUD for this field (edit experience screen)
- `persist_lap` checks `min_lap_time_ms` from the matching experience row; laps below the floor get `valid = false` (not deleted)

### Session type classification (LAP-03)
- **Map AC session type enum from shared memory** to `SessionType::Hotlap` vs `SessionType::Practice`
- AC sends: QUALIFY / HOTLAP → `SessionType::Hotlap`; PRACTICE / RACE → `SessionType::Practice`
- `LapData.session_type` field already exists — researcher must confirm it's populated in assetto_corsa.rs or whether wiring is needed like LAP-01

### PIN failure counters (PIN-01/02)
- **In-memory on racecontrol server** — `HashMap<pod_id, PinFailedAttempts>` in `AppState`, same pattern as `OtpFailedAttempts` (state.rs lines 65-68)
- **Two separate structs per pod**: `customer_pin_failures` and `staff_pin_failures` — different keys or a nested struct, never shared
- **Customer lockout**: 5 failures → 5-minute lockout. Pod lock screen shows "Try again in N minutes"
- **Staff lockout**: 10 failures → temporary lockout (duration: Claude's discretion, suggested 15 min). Staff counter is completely independent of customer counter
- **Reset**: counters reset on lockout expiry (time-based, no manual reset needed)
- Staff PIN is checked first in validate_pin (already the case via `todays_debug_pin()`) — counter increment must be gated so only staff attempts hit the staff counter

### Telemetry gap bot (TELEM-01)
- **Trigger**: `last_udp_secs_ago >= 60` AND `billing_active = true` AND `DrivingState::Active` (all three conditions must hold simultaneously)
- **Alert delivery**: email to james@racingpoint.in via existing Bono email relay + set `TelemetryLost` flag on pod's kiosk dashboard card
- **Fire once, reset on UDP resumption** — when UDP resumes (`last_udp_secs_ago < 60`), clear the alert flag. No repeat spamming
- **Implementation**: extends billing_guard.rs pattern; reads same `FailureMonitorState` watch receiver; constructs `AgentMessage::TelemetryGap` with the gap duration

### Multiplayer disconnect bot (MULTI-01)
- **Detection**: monitor AC shared memory `status` / session state field — when a multiplayer-joined session drops to offline/standalone, that's the signal
- **Action**: fully automatic teardown — same end-to-end recovery as CRASH-01, no staff approval gate
- **Teardown sequence per pod**: lock screen → zero FFB → end billing → log `MultiplayerServerDisconnect` event
- **Cascade**: bot coordinator on racecontrol server identifies all pods sharing the same `session_id` / multiplayer group and triggers teardown on all simultaneously (not just the detecting pod)
- **Message flow**: rc-agent sends `AgentMessage::MultiplayerFailure { reason: MultiplayerServerDisconnect }` → ws/mod.rs routes to `bot_coordinator::handle_multiplayer_failure()` (new handler, same pattern as handle_billing_anomaly)

### Claude's Discretion
- Exact staff PIN lockout duration (15 min suggested)
- AC shared memory field name/offset for session type (researcher to confirm)
- Admin UI layout for min_lap_time_ms field (inline edit on existing experience table row)
- `TelemetryLost` flag representation on dashboard (boolean in pod state or dedicated status enum variant)

</decisions>

<specifics>
## Specific Ideas

- **Billing guard pattern reuse**: TELEM-01 detection should live in billing_guard.rs or a sibling file using the same `FailureMonitorState` watch — avoid a fourth separate file if the pattern fits cleanly
- **LAP-02 null = no floor**: tracks without a configured minimum should silently pass — no behavior change for existing tracks
- **MULTI-01 teardown is like CRASH-01**: the teardown sequence is identical to the frozen game recovery (zero FFB, kill game, lock screen, end billing) — reuse the same `fix_frozen_game` teardown steps rather than duplicating logic

</specifics>

<code_context>
## Existing Code Insights

### Reusable Assets
- `billing_guard.rs` — full detection loop pattern with `FailureMonitorState` watch receiver; TELEM-01 detection is a natural third condition in the same 5s poll loop
- `OtpFailedAttempts` struct (state.rs:65-68) — exact pattern to clone for `PinFailedAttempts`
- `fix_frozen_game` teardown sequence — FFB zero + game kill + lock screen + end billing; reuse for MULTI-01
- `bot_coordinator::handle_billing_anomaly()` — routing pattern for MULTI-01's `handle_multiplayer_failure()`

### Established Patterns
- **AgentMessage routing**: rc-agent constructs variant → WS sends → ws/mod.rs routes to bot_coordinator handler → handler takes action on server side
- **billing_guard detection**: watch receiver poll every 5s, check multiple FailureMonitorState fields, construct AgentMessage on threshold breach
- **persist_lap gate**: `if !lap.valid { return; }` already present at line 28 of lap_tracker.rs — min_lap_time check adds a second gate before this

### Integration Points
- `crates/rc-agent/src/sims/assetto_corsa.rs` — LAP-01 offset fix + LAP-03 session_type wiring
- `crates/racecontrol/src/db/mod.rs` — LAP-02 migration: add `min_lap_time_ms INTEGER` to `kiosk_experiences`
- `crates/racecontrol/src/lap_tracker.rs` — LAP-02 floor check (query kiosk_experiences by experience_id)
- `crates/racecontrol/src/auth/mod.rs` — PIN-01/02 counter logic in `validate_pin` and `validate_pin_kiosk`
- `crates/racecontrol/src/state.rs` — add `PinFailedAttempts` struct + two HashMaps to AppState
- `crates/rc-agent/src/billing_guard.rs` — TELEM-01 third detection arm
- `crates/rc-agent/src/sims/assetto_corsa.rs` — MULTI-01 multiplayer status field monitoring
- `crates/racecontrol/src/bot_coordinator.rs` — new `handle_multiplayer_failure()` routing function
- `crates/racecontrol/src/ws/mod.rs` — wire `MultiplayerFailure` arm to `handle_multiplayer_failure()` (currently a stub)

</code_context>

<deferred>
## Deferred Ideas

- **F1 25 is_valid wiring** — deferred until F1 25 adapter sees real venue use
- **TELEM-01 repeat/escalation** — re-alert every 5 min was considered but rejected; could be added if single alert proves insufficient
- **MULTI-01 staff 30s grace window** — alert + wait before auto-teardown was considered; rejected in favor of fully automatic to maintain autonomous recovery promise
- **DBG-01/02** (bot action log, DebugMemory billing context) — v6.0 deferred requirements, not Phase 26

</deferred>

---

*Phase: 26-lap-filter-pin-security-telemetry-multiplayer*
*Context gathered: 2026-03-16*
