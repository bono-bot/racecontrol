# Phase 109: Safe Mode State Machine - Context

**Gathered:** 2026-03-21
**Status:** Ready for planning

<domain>
## Phase Boundary

This phase adds a safe mode state machine to rc-agent that detects protected game launches (both rc-agent-initiated and external/Steam launches), automatically disables risky subsystems, maintains a 30-second exit cooldown after game exit, and ensures billing/lock screen/WS exec continue uninterrupted. The safe mode state lives in AppState and survives WebSocket reconnections.

</domain>

<decisions>
## Implementation Decisions

### Safe Mode State
- New `SafeMode` struct in AppState with `active: bool`, `game: Option<SimType>`, `cooldown_until: Option<Instant>`
- State survives WebSocket reconnections (lives in AppState, not WS session)
- New module: `crates/rc-agent/src/safe_mode.rs`

### Game Detection — Dual Path
- **Primary:** Existing `LaunchGame` handler in ws_handler.rs — enters safe mode before spawning game process (zero delay)
- **Secondary:** WMI `Win32_ProcessStartTrace` event subscription for detecting games launched OUTSIDE rc-agent (manual Steam launches, staff testing). This covers the SAFE-01 requirement for sub-1-second detection.
- WMI subscription watches for known game executables: `F1_25.exe`, `iRacingSim64DX11.exe`, `Le Mans Ultimate.exe`, `acs_x64.exe` (AC EVO), `WRC.exe`

### Protected Games
- F1 25 (EA Javelin) — REQUIRED
- iRacing (EOS) — REQUIRED
- LMU / Le Mans Ultimate (EAC) — REQUIRED
- EA WRC (EA AntiCheat) — REQUIRED
- AC EVO (unknown) — PROTECTED as precaution (Early Access, may add anti-cheat)
- AC original — NOT protected (no anti-cheat)

### Safe Mode Exit
- Game process exits (detected by existing GameProcess monitoring OR WMI process stop event)
- Start 30-second cooldown timer (EA Javelin stays active post-game)
- Safe mode fully deactivates after cooldown expires
- If another protected game launches during cooldown, safe mode stays active (no gap)

### Claude's Discretion — Subsystem Gating
- Process guard (allowlist enforcement + auto-kill) — SUSPEND during safe mode
- Ollama LLM queries — SUPPRESS during safe mode (GPU/VRAM contention + detection risk)
- Registry write operations — DEFER until safe mode exits
- Claude decides: implementation approach for gating (Arc<AtomicBool>, channel signals, or direct state checks)
- Claude decides: whether to add safe_mode field to self_test.rs probes
- Claude decides: logging strategy for safe mode transitions

### Unaffected Subsystems (SAFE-07)
- Billing lifecycle (start, ticks, end) — MUST continue
- Lock screen management — MUST continue
- Overlay — MUST continue
- WebSocket keepalive + heartbeat — MUST continue
- WebSocket exec (remote commands) — MUST continue
- UDP heartbeat — MUST continue

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `crates/rc-agent/src/ws_handler.rs:279` — `LaunchGame` handler, primary safe mode entry point
- `crates/rc-agent/src/app_state.rs:24` — AppState struct, safe mode state goes here
- `crates/rc-agent/src/game_process.rs` — GameProcess monitoring, game exit detection
- `crates/rc-agent/src/process_guard.rs` — needs safe mode gate
- `crates/rc-agent/src/ai_debugger.rs` — Ollama queries, needs safe mode suppression
- `crates/rc-agent/src/event_loop.rs` — main loop, WMI subscription could live here
- `crates/rc-common/src/protocol.rs` — SimType enum with all game types

### Established Patterns
- AppState fields are `pub(crate)` with direct access in ws_handler and event_loop
- Async tasks use `tokio::spawn` with `Arc<RwLock<>>` for shared state
- Process monitoring uses `CreateToolhelp32Snapshot` (one-time snapshots, not continuous)
- Config in `racecontrol.toml` / `rc-agent.toml`

### Integration Points
- `ws_handler.rs:279` LaunchGame — add safe mode entry before game spawn
- `event_loop.rs` — add WMI subscription loop and cooldown timer check
- `process_guard.rs` — add `if safe_mode.active { return; }` guard
- `ai_debugger.rs` — add safe mode check before Ollama query
- `app_state.rs` — add `safe_mode: SafeMode` field

</code_context>

<specifics>
## Specific Ideas

- WMI in Rust: use `windows` crate WMI COM interfaces, or shell out to `wmic` / PowerShell for simplicity
- Phase 107 compatibility matrix already defines SAFE/UNSAFE/SUSPEND/GATE per subsystem per game
- Safe mode should default to ACTIVE on startup if a protected game is already running (check process list once at boot)
- Consider adding safe mode status to fleet health reporting (server knows which pods are in safe mode)

</specifics>

<deferred>
## Deferred Ideas

- Server-side safe mode dashboard visibility — deferred to future milestone
- Safe mode override from admin panel — not needed for v15.0
- Per-game subsystem gating granularity (different gates for different games) — all games get same safe mode for v15.0

</deferred>
