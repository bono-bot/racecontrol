# Project Research Summary

**Project:** RC Bot Expansion v5.0 — Deterministic Auto-Fix for Sim Racing Pod Management
**Domain:** Embedded venue operations — Windows 11 process and hardware watchdog with billing integration
**Researched:** 2026-03-16
**Confidence:** HIGH

## Executive Summary

RaceControl v5.0 expands the existing `ai_debugger.rs` auto-fix system from 4 fix patterns to 9 failure classes: pod crash/hang, billing edge cases, network repair, USB hardware failure, game launch failures, telemetry gaps, multiplayer session guards, kiosk PIN lockouts, and lap filtering. The research is grounded in direct codebase inspection of every relevant source file — no training-data assumptions. The key finding from STACK.md is that no new crates are required. All 9 patterns are implementable with the existing dependency set (`sysinfo 0.33`, `winapi 0.3`, `hidapi 2`, `tokio 1`, `reqwest 0.12`). The architecture introduces three new rc-agent modules (`failure_monitor.rs`, `billing_guard.rs`, `lap_filter.rs`) and one new racecontrol coordinator (`bot_coordinator.rs`), linked by 5 new `AgentMessage` variants and a shared `PodFailureReason` enum in rc-common.

The central architectural constraint is that all detection logic must flow through `try_auto_fix()` keyword dispatch to preserve `DebugMemory` pattern learning. Detection belongs in `failure_monitor.rs`; fix implementations stay in `ai_debugger.rs`. This indirection is not overhead — it is the mechanism that makes fixes smarter over time via pattern replay. The build order is non-negotiable: rc-common protocol changes compile first (both consuming crates break until variants are handled), then rc-agent detection infrastructure, then racecontrol coordinator.

The highest-severity risks are billing-related. Any fix that kills a game process or resets hardware must: (1) gate on `billing_active` state inside the fix function itself (not just the call site — pattern memory replay bypasses call-site guards), (2) call `end_billing_session()` before the kill rather than relying on `AcStatus::Off` propagation which may not arrive in crash scenarios, and (3) encode billing context in the pattern memory key. The second major risk is the cloud sync wallet race: a bot-triggered early session end within a 30-second sync window can result in a cloud pull overwriting the local deduction (CRDT merge uses `MAX(updated_at)`, documented as P1 in CONCERNS.md). This race must be fenced before billing edge case fixes ship.

## Key Findings

### Recommended Stack

No new crates are required for v5.0. The STACK.md finding is definitive. `winapi 0.3`'s `winuser` feature (already present in `Cargo.toml`) covers `IsHungAppWindow` and `EnumWindows` for hang detection. `hidapi 2`'s `device_list()` covers USB reconnect polling (blocking — wrap in `spawn_blocking`). `sysinfo 0.33` covers game process CPU hang detection with a mandatory two-refresh pattern. `tokio::time::timeout` handles launch timeouts. Versions must not be upgraded: `sysinfo 0.33 -> 0.38` changes `ProcessesToUpdate` and `Process::status()` return types, breaking all 47 existing tests with zero bot benefit.

**Core technologies (existing, used for new patterns):**
- `sysinfo 0.33`: Process CPU and alive checks — stay pinned, API changed in 0.34+
- `winapi 0.3` (winuser feature): `IsHungAppWindow` + `EnumWindows` for game freeze detection — already present, zero Cargo.toml change
- `hidapi 2`: USB wheelbase reconnect polling via `device_list()` — wrap blocking scan in `spawn_blocking`, existing pattern in `driving_detector.rs`
- `tokio` (workspace): `time::timeout` for 90-second launch hang detection, `time::interval` for detection polling loops
- `reqwest 0.12`: HTTP retry pattern for cloud sync failures — already implemented in `cloud_sync.rs`, apply same pattern to rc-agent HTTP calls

**What to avoid adding:** `windows-rs`/`windows-sys` (duplicate Windows bindings, linker conflicts with existing `winapi 0.3`), `notify` (inotify complexity with no benefit over `hidapi` polling), WMI (100ms+ COM startup cost), `lettre` (prohibited by PROJECT.md), `WM_DEVICECHANGE` (requires GUI message pump), any per-bot UDP socket (read HeartbeatStatus atomics instead).

### Expected Features

FEATURES.md defines 9 failure classes with P1 (table stakes), P2 (differentiators), and P3 (defer) patterns. P1 patterns are what staff currently handle manually — eliminating them is the entire value proposition of v5.0.

**Must have (P1 — eliminates manual staff intervention):**
- UDP silence timeout -> crash detection: staff do not know a game froze until a customer complains
- WerFault dismiss on deterministic crash path: currently wired only through AI path; must fire on every crash
- FFB zero force on session end AND game crash: safety requirement; high torque engaged with no game feedback is a physical hazard
- Stuck session cleanup: billing stays active after game exit, charging customers for time they did not drive
- Launch timeout (90s) + Content Manager hang kill: staff currently walk to the pod when launch hangs; bot eliminates most cases
- PIN fail count tracking + lockout message: customers stranded at lock screen with no guidance
- Invalid lap flag wiring for F1 and AC sim adapters: without this, track cuts and crashes enter the leaderboard
- Game-state-aware telemetry silence alerting: must gate on `AcStatus::Live` to suppress in-menu silence noise

**Should have (P2 — operational quality):**
- USB disconnect mid-session alert path
- Cloud sync consecutive failure counter and structured recovery
- IP drift fallback config (multi-IP list in rc-agent.toml)
- Pre-launch dialog clearance before each launch attempt
- Crash pattern classification by exit code (0xC0000005 = access violation etc.)
- Freeze vs crash discrimination (`is_process_alive()` + UDP silence conjunction)
- Per-track minimum lap time validation (requires `min_lap_ms` in track catalog)
- AC server reachability pre-check before multiplayer launch
- Single pod desync detection

**Defer (P3 or after field validation):**
- USB re-enumeration auto-reconnect: USB re-enum behavior varies by Windows build; needs field testing on pods before shipping
- Multiplayer auto-rejoin: requires AC server session token accessible to rc-agent; path does not currently exist
- WS reconnect storm prevention with jitter: build when reconnect storms are observed in practice
- Statistical outlier lap detection: needs sufficient lap history volume per track before it produces reliable signals
- Billing tick gap local pause: edge case with unclear real-world impact

### Architecture Approach

The architecture follows strict separation of concerns: `failure_monitor.rs` owns all detection policy, `ai_debugger.rs` owns all fix implementations, and `bot_coordinator.rs` on the server owns all fleet-level decisions (billing recovery, session teardown, multiplayer coordination). Detection flows through synthetic suggestion strings to `try_auto_fix()` — preserving the `DebugMemory` learning loop that enables sub-100ms instant fix replay on recurrence. Server-side billing end never fires until the agent confirms `SessionUpdate::Finished` — preserving the existing invariant: lock screen before game kill, game kill before billing end.

**Major components:**
1. `failure_monitor.rs` (NEW, rc-agent) — single detection loop for 7 agent-side failure classes; reads HeartbeatStatus atomics; constructs canonical synthetic suggestions; calls `try_auto_fix()`
2. `billing_guard.rs` (NEW, rc-agent) — 30-second poll for 3 billing anomaly classes; emits `BillingAnomaly` over WebSocket; never directly ends sessions
3. `lap_filter.rs` (NEW, rc-agent) — validates laps at UDP capture time using game-reported `isValidLap` as primary signal; bot analysis is review flag only, never auto-reject
4. `bot_coordinator.rs` (NEW, racecontrol) — receives 5 new `AgentMessage` variants; routes billing anomalies to `recover_stuck_session()`, telemetry gaps to dashboard alerts, hardware failures to email alerts, lap flags to `lap_tracker.rs`
5. `PodFailureReason` enum (NEW, rc-common) — 21-variant shared taxonomy; must be defined before any detection or fix code is written; forces deliberate naming as a compile-time gate
6. `ai_debugger.rs` (MODIFY, rc-agent) — 6 new `try_auto_fix()` arms + 6 new fix handler functions + 7 extended `PodStateSnapshot` fields for richer AI context

**Key agent-side fix flow:** `failure_monitor` detects -> builds `PodStateSnapshot` -> constructs canonical synthetic suggestion -> `try_auto_fix()` -> fix handler -> `DebugMemory.record_fix()` -> `AgentMessage` to server.

**Key server-side recovery flow:** `BillingAnomaly` received -> `bot_coordinator` -> `CoreToAgentMessage::StopSession` to agent -> agent kills game, shows lock screen -> `AgentMessage::SessionUpdate { Finished }` -> `billing::end_session()` fires.

### Critical Pitfalls

1. **Fix fires during active billing session without a guard** — Every new fix function that terminates a process, resets a device, or closes a socket MUST gate on `!snapshot.billing_active` inside the fix function itself. Pattern memory replay (`DebugMemory::instant_fix()`) calls `try_auto_fix()` directly, bypassing any call-site guards. Required: a test for every new fix with `billing_active: true` confirming it returns `None` (no-op). This is acceptance criteria for every phase that adds a fix pattern.

2. **Pattern memory replays destructive fix in wrong billing context** — `DebugMemory` keys on `"{SimType}:{exit_code}"` only. A fix recorded while billing was inactive replays instantly during an active session. Extend `pattern_key` to `"{SimType}:{exit_code}:billing={true/false}"` before adding any destructive fix type. Also add `billing_active_when_recorded: bool` to `DebugIncident` schema.

3. **Billing timer orphans after bot-triggered game kill** — `BillingTimer::tick()` runs independently of game state. If the bot kills the game, `AcStatus::Off` may never arrive (WebSocket degraded was why the fix fired). The billing timer keeps charging. Any game kill fix must call `end_billing_session()` before executing the process kill — not after.

4. **Cloud sync wallet race on bot-triggered EndedEarly** — CONCERNS.md P1: CRDT merge uses `MAX(updated_at)`. If the cloud record has a 1-second clock skew advantage, it overwrites the local deduction. Bot-triggered session ends happen asynchronously within the 30-second sync window. Requires a wallet write fence (tombstone timestamp or sync hold-off) before billing recovery ships.

5. **CRLF in remote command strings silently does nothing** — Rust string literals use Unix `\n`. `cmd.exe` splits on `\r\n`. Multi-line bot commands posted to pod-agent `/exec` are treated as one long invalid command while returning HTTP 200. This caused the March 15 outage. All multi-line remote commands must use `\r\n` with a unit test asserting their presence. Cross-cutting — applies to every phase.

6. **Multiple bot tasks racing on the same pod simultaneously** — `pod_healer`, `pod_monitor`, and new bot tasks can simultaneously detect and attempt to fix the same degraded pod, leaving the pod in a worse state. Establish `is_pod_in_recovery(state, pod_id) -> bool` as a shared utility before any new tasks are added. Every new bot task must call it before acting.

7. **Telemetry gap false alerts when no session is active** — A telemetry monitor without billing awareness fires between sessions, causing staff alert fatigue. All telemetry and hardware alerts must gate on `billing_active: true`.

## Implications for Roadmap

The build order is dictated by cross-crate compile dependencies and safety ordering. Four phases are suggested, with Phase 1 being non-negotiable as the protocol foundation.

### Phase 1: Protocol Contract and Concurrency Safety

**Rationale:** rc-common compiles before both rc-agent and racecontrol. Any new `types.rs` or `protocol.rs` entry breaks both consuming crates until they handle new variants. This must land first, alone, in a clean commit. Additionally, the `is_pod_in_recovery()` concurrency guard must exist before any new bot tasks are spawned — establishing it in Phase 1 prevents racing in all subsequent phases.

**Delivers:** `PodFailureReason` enum (21 variants) in rc-common types.rs. Five new `AgentMessage` variants (`HardwareFailure`, `TelemetryGap`, `BillingAnomaly`, `LapFlagged`, `MultiplayerFailure`) in rc-common protocol.rs. `is_pod_in_recovery()` utility in racecontrol AppState. All rc-common existing tests green. Both consuming crates produce compile errors (expected — no handling code yet), confirming the protocol is wired.

**Avoids:** Pitfall 6 (concurrent task racing) — foundation laid before any new tasks exist. Pitfall 5 (string-typed reasons) — `PodFailureReason` enum defined in rc-common forces typed usage everywhere.

**Research flag: skip.** serde enum extension with `#[serde(tag = "type")]` is a well-established Rust pattern. Direct codebase inspection of `protocol.rs` confirms exact existing format. No unknowns.

### Phase 2: Crash, Hang, Launch, and USB Bot Patterns

**Rationale:** These four failure classes have the highest staff relief value and lowest implementation complexity. They are entirely agent-side fixes that do not require server coordination beyond the existing `AiDebugResult` path. Creating `failure_monitor.rs` here establishes the detection pattern (synthetic suggestion -> `try_auto_fix()`) that all subsequent agent-side expansion follows. FFB zero-force on crash is a physical safety requirement and must not be deferred.

**Delivers:** `failure_monitor.rs` with game freeze detection (`IsHungAppWindow` conjunction check: low CPU + no UDP + `IsHungAppWindow` = true, all four required), Content Manager hang kill, 90-second launch timeout via `tokio::time::timeout`, USB wheelbase reconnect polling every 5s via `hidapi::device_list()`. FFB zero-force wiring on crash and session end. Six new `try_auto_fix()` arms with canonical keyword contracts. Extended `PodStateSnapshot` with 7 new fields. Minor changes to `game_process.rs` (expose `launch_elapsed_secs`) and `driving_detector.rs` (expose `last_hid_error: Option<String>`). All 47+ existing tests pass plus new tests per fix handler.

**Implements:** `failure_monitor.rs` (NEW), extended `ai_debugger.rs`.

**Avoids:** Pitfall 1 (billing guard) — every new fix has `billing_active: true` test confirming no-op. Pitfall 3 (billing timer orphan) — game kill fix emits billing end before the kill. Pitfall 4 (USB re-enumeration) — post-reset VID/PID confirmation before marking success. Pitfall 5 (CRLF) — all remote command strings tested for `\r\n`. The IsHungAppWindow conjunction check (all four conditions required) avoids the false positive case of a game at the AC main menu (low CPU + no UDP but window is responsive).

**Research flag: skip.** All patterns derivable directly from existing codebase. `IsHungAppWindow` + `EnumWindows` APIs are in the already-present `winuser` winapi feature. USB polling pattern already exists in `driving_detector.rs`. No external research needed.

### Phase 3: Billing Guard, Telemetry Alerting, and Server Coordinator

**Rationale:** Billing edge cases require both agent-side detection (`billing_guard.rs`) and server-side recovery (`bot_coordinator.rs`). The wallet sync race (CONCERNS.md P1) must be fenced before any bot can trigger `EndedEarly`. The server coordinator must exist before Phase 4 can route multiplayer and PIN messages. Telemetry gap detection belongs here because it shares the billing-active gate requirement and routes through the same coordinator.

**Delivers:** `billing_guard.rs` with stuck session detection (billing_active + game not running >60s), idle drift guard gated on `DrivingState::Idle` with 60-second minimum threshold, cloud sync failure counter. `bot_coordinator.rs` routing `BillingAnomaly`, `TelemetryGap`, `HardwareFailure`. `billing::recover_stuck_session()` helper using the StopSession -> SessionUpdate::Finished -> end_session() flow. Wallet write fence for bot-triggered EndedEarly. Pattern memory key extended to `"{SimType}:{exit_code}:billing={true/false}"`. `DebugIncident` schema extended with `billing_active_when_recorded`. `ws/mod.rs` routing for 5 new AgentMessage variants.

**Implements:** `billing_guard.rs` (NEW rc-agent), `bot_coordinator.rs` (NEW racecontrol), minor modifications to `billing.rs` and `ws/mod.rs`.

**Avoids:** Pitfall 2 (pattern memory context) — DebugIncident gets billing_active field, key extended before any destructive fix type uses it. Pitfall 3 (billing timer orphan) — `recover_stuck_session()` uses the correct sequenced flow, never calls `end_session()` directly. Pitfall 7 (cloud sync wallet race) — fence added after bot EndedEarly, before next sync pull window. Pitfall 9 (idle detection fires during menu navigation) — idle threshold is 60s minimum and gated on `DrivingState::Idle` (confirmed idle in-car), not UDP silence alone.

**Research flag: needs attention before coding.** The wallet sync fence mechanism requires a decision before `recover_stuck_session()` is implemented. Options: (a) write `updated_at = cloud_ts + 1s`, (b) add `venue_authoritative` flag to wallet upsert, (c) migrate to additive transaction log. Review actual `cloud_sync.rs` CRDT merge logic to confirm which option is feasible without a DB migration. Additionally, `billing.rs` has zero test coverage (CONCERNS.md P0) — write characterization tests for the existing session lifecycle before adding new code. This is the "Refactor Second" standing rule.

### Phase 4: Lap Filter, PIN Bot, and Multiplayer Guard

**Rationale:** All three classes depend on the server coordinator from Phase 3 being in place. Lap filter emits `LapFlagged` which routes through `bot_coordinator`. PIN bot extends existing lock_screen state without touching billing. Multiplayer guard requires `bot_coordinator` for cross-pod desync decisions. All three are lower risk than billing edge cases.

**Delivers:** `lap_filter.rs` using game-reported `isValidLap` as primary validity signal; bot analysis (speed floor, sector sum) as secondary review flag only; laps soft-flagged with `review_required: true`, never hard-deleted. Invalid lap flag wiring in AC and F1 sim adapters. PIN fail counter with strict type separation: customer counters and staff/debug counters are independent, lockout action applies to customer type only. QR soft alert after 120s. AC server reachability pre-check before multiplayer launch. Single pod desync detection (one pod `AcStatus::Off` while others remain `Live`). `multiplayer.rs` desync state hook.

**Implements:** `lap_filter.rs` (NEW rc-agent), PIN state extensions in `lock_screen.rs`, `ac_server.rs` reachability check, minor `multiplayer.rs` modification.

**Avoids:** Pitfall 11 (lap filter rejects valid laps on LAN packet loss) — game-reported validity is authoritative; LAN packet loss looks identical to a track cut at the raw telemetry layer; bot analysis alone cannot distinguish them. Pitfall 12 (PIN bot locks out staff) — customer and staff PIN failure counters are strictly separated by pin_type field; lockout applies only to customer type.

**Research flag: skip for lap filter.** Game-reported `isValidLap` as the primary validity signal is the established correct approach for AC UDP protocol — no further research needed. **Needs scoping decision for multiplayer.** Auto-rejoin requires AC server session token access; current architecture has no such path. Phase 4 must be scoped to detection and staff alert only. Auto-rejoin is flagged for potential Phase 5 pending investigation of whether racecontrol can generate and forward the join URL to the agent.

### Phase Ordering Rationale

- Phase 1 first: cross-crate compile dependency makes this non-negotiable.
- Phase 2 second: can be validated on Pod 8 (canary) before the server coordinator exists. Safety and crash fixes deliver immediate staff relief and establish the `failure_monitor.rs` detection pattern.
- Phase 3 third: billing recovery is the highest-risk bot action in the codebase. Requires concurrency guard from Phase 1, detection foundation from Phase 2, and a wallet sync fence decision that must not be rushed. Cannot ship before `billing.rs` has characterization tests.
- Phase 4 last: lowest operational risk. Depends on coordinator from Phase 3. Lap filter and PIN bot can be developed in parallel with Phase 3 but must deploy after.

### Research Flags

Phases needing deeper investigation before coding begins:

- **Phase 3 — Wallet sync fence:** Review actual `cloud_sync.rs` CRDT merge implementation to confirm which fence mechanism is feasible. Do not code `recover_stuck_session()` until the billing end -> sync window race is addressed. Options are (a) timestamp manipulation, (b) authoritative flag, (c) transaction log migration. Option (c) is correct long-term but may be out of scope for v5.0.
- **Phase 4 — Multiplayer auto-rejoin scope:** Confirm whether racecontrol can generate and forward AC server join URLs to the agent. If not, scope Phase 4 multiplayer to detection plus staff alert only and defer auto-rejoin to Phase 5.

Phases with established patterns (skip research):

- **Phase 1:** serde enum extension with `#[serde(tag = "type")]` is well-documented; all existing protocol details confirmed from direct `protocol.rs` inspection.
- **Phase 2:** All patterns derivable from existing codebase. `IsHungAppWindow`, USB polling, `tokio::time::timeout` are all established uses of already-present dependencies.
- **Phase 4 (lap filter):** Game-reported `isValidLap` as the primary signal is the correct and established approach for AC UDP telemetry.

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | Dependencies read directly from `Cargo.toml`; version pinning rationale verified against sysinfo API changelog; winapi feature coverage confirmed in docs; "no new crates" finding is definitive |
| Features | HIGH | All 9 failure classes derived from direct codebase inspection of existing types, protocol messages, and partial implementations; feature table mapped to existing code symbols throughout |
| Architecture | HIGH | All claims derived from reading actual source files: 767-line `ai_debugger.rs`, 219-line `self_monitor.rs`, `protocol.rs`, `types.rs`, `pod_monitor.rs`, `billing.rs`, `pod_healer.rs`; no training-data assumptions |
| Pitfalls | HIGH | 12 pitfalls derived from direct code paths (`try_auto_fix`, `BillingTimer::tick`, `DebugMemory::instant_fix`), CONCERNS.md P0/P1 issues, and documented production incidents (CRLF March 15 outage, billing timer behavior) |

**Overall confidence: HIGH**

### Gaps to Address

- **Wallet sync CRDT fence implementation:** CONCERNS.md marks wallet sync as P1 with CRDT merge untested under concurrent writes. The exact fence mechanism must be decided before Phase 3 billing recovery ships. Re-read `cloud_sync.rs` implementation before planning Phase 3 sprint.

- **Content Manager process name on all pods:** STACK.md notes CM registers as `Content Manager.exe` but advises confirmation with `tasklist /FI "IMAGENAME eq Content Manager.exe"` on an actual pod before the Phase 2 kill pattern is coded. Run this check on Pod 8 before the launch timeout fix is submitted.

- **`billing.rs` zero test coverage:** CONCERNS.md P0. Phase 3 adds `recover_stuck_session()` to this file. Characterization tests for the existing session lifecycle must be written first. Estimate: 1-2 days of characterization test work before any Phase 3 code is written.

- **AC server session token accessibility for multiplayer auto-rejoin:** Current architecture has no path from rc-agent to the AC server join URL. Determine whether racecontrol can generate and forward it before finalizing Phase 4 scope. This decision gates whether multiplayer auto-rejoin is achievable in v5.0 at all.

- **sysinfo two-refresh CPU pattern in async context:** The game freeze detection requires two `refresh_processes()` calls 500ms apart for accurate CPU readings. The hang detector should maintain a persistent `System` instance in a dedicated async task, pre-warmed every 30s, so CPU data is fresh when the conjunction check fires. Confirm this pattern in the Phase 2 implementation plan before coding.

## Sources

### Primary (HIGH confidence — direct source reads, 2026-03-16)

- `crates/rc-agent/Cargo.toml` — authoritative dependency list; "no new crates" finding confirmed here
- `crates/rc-agent/src/ai_debugger.rs` — `try_auto_fix()`, `DebugMemory`, `PodStateSnapshot`, `PROTECTED_PROCESSES` (767 lines)
- `crates/rc-agent/src/self_monitor.rs` — existing bot loop, CLOSE_WAIT detection, `relaunch_self()` (219 lines)
- `crates/rc-agent/src/game_process.rs` — `is_process_alive()`, `GetExitCodeProcess`, PID lifecycle, sysinfo usage
- `crates/rc-agent/src/driving_detector.rs` — `DetectorSignal::HidDisconnected`, `last_udp_packet`, DrivingState FSM
- `crates/rc-agent/src/ffb_controller.rs` — `CMD_ESTOP`, `zero_force()`, hidapi HID write pattern
- `crates/rc-agent/src/udp_heartbeat.rs` — `HeartbeatStatus` atomics, `HeartbeatEvent::CoreDead`, sequence tracking
- `crates/rc-common/src/protocol.rs` — all `AgentMessage` variants, `CoreToAgentMessage`, serde tag format
- `crates/rc-common/src/types.rs` — `BillingSessionStatus`, `GameState`, `DrivingState`, `AcStatus`, `LapData`
- `crates/racecontrol/src/pod_monitor.rs` — `WatchdogState` FSM, watchdog coordination pattern
- `crates/racecontrol/src/billing.rs` — `BillingTimer::tick()`, `end_billing_session()`, `WaitingForGameEntry`
- `crates/racecontrol/src/pod_healer.rs` — `heal_pod()` billing active check (lines 223-230), watchdog state skip (lines 151-176)
- `.planning/PROJECT.md` — v5.0 requirements, "no new dependencies" constraint, known past bugs
- `.planning/codebase/CONCERNS.md` — P0/P1 issues: billing.rs zero test coverage, cloud sync CRDT untested, pod state races, 154 `.ok()` error silences

### Secondary (MEDIUM confidence — community and docs)

- crates.io/crates/sysinfo — v0.38.3 confirmed latest; v0.33 pinned intentionally, API changed in 0.34+
- docs.rs/windows-sys `IsHungAppWindow` — confirmed present in `winuser` feature (already in rc-agent Cargo.toml)
- Rust community: sysinfo CPU two-refresh requirement — documented pattern, matches sysinfo internal behavior
- kennykerr.ca: winapi vs windows-sys comparison — winapi 0.3 functional for current needs; windows-rs would add duplicate Windows bindings

### Tertiary (context — production incident records)

- MEMORY.md — CRLF bug root cause (March 15 outage), billing_active 10-second idle threshold, Conspit Ares VID/PID, 8-pod network map, Session 0/Session 1 fix history
- `.planning/codebase/ARCHITECTURE.md` — codebase map (7 docs, 3,960 lines, read 2026-03-16)

---
*Research completed: 2026-03-16*
*Ready for roadmap: yes*
