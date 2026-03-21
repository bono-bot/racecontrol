# Phase 110: Telemetry Gating - Context

**Gathered:** 2026-03-21
**Status:** Ready for planning

<domain>
## Phase Boundary

This phase gates the shared memory telemetry readers (iRacing, LMU, AC EVO) and UDP telemetry sockets (F1 25, EA WRC) so they respect anti-cheat initialization windows. Shared memory connects are deferred 5 seconds after game process is stable. UDP sockets are created only when the corresponding game is active and destroyed on game exit. AC EVO telemetry is feature-flagged off by default.

</domain>

<decisions>
## Implementation Decisions

### Shared Memory Gating (HARD-03)
- iRacing and LMU shared memory adapters defer MapViewOfFile/OpenFileMapping connect until 5 seconds after game process state reaches Running
- Use existing game_process.rs GameState::Running detection — add a 5-second delay before telemetry adapter initialization
- This avoids accessing shared memory during anti-cheat driver initialization

### UDP Socket Lifecycle (HARD-04)
- F1 25 and EA WRC UDP telemetry sockets created only when the corresponding game is in Running state
- Sockets destroyed (dropped) within 5 seconds of game exit
- Existing UDP ports: 20777 (F1), 5555 (LMU — actually UDP too), 6789 (iRacing — UDP heartbeat), 9996 (AC)

### AC EVO Feature Flag (HARD-05)
- AC EVO shared memory telemetry is feature-flagged off by default
- TOML config: `ac_evo_telemetry_enabled = false` in rc-agent.toml
- Until Kunos confirms anti-cheat status at v1.0, the adapter exists but is not activated

### Claude's Discretion
- Exact implementation of the 5-second delay (tokio::time::sleep vs timer in event loop)
- Where to add the feature flag check (adapter creation vs event loop)
- Error handling for shared memory connect failures after delay

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `crates/rc-agent/src/sims/iracing.rs` — iRacing shared memory adapter
- `crates/rc-agent/src/sims/lmu.rs` — LMU shared memory adapter (rF2 format)
- `crates/rc-agent/src/sims/assetto_corsa_evo.rs` — AC EVO shared memory adapter
- `crates/rc-agent/src/sims/f1_25.rs` — F1 25 UDP telemetry adapter
- `crates/rc-agent/src/sims/mod.rs` — SimAdapter trait and adapter creation
- `crates/rc-agent/src/game_process.rs` — GameState enum, game lifecycle
- `crates/rc-agent/src/safe_mode.rs` — Safe mode (Phase 109)

### Established Patterns
- SimAdapter trait with `connect()`, `poll()`, `disconnect()` methods
- UDP sockets bound in adapter `connect()` method
- Shared memory mapped in adapter `connect()` method
- Config in rc-agent.toml

### Integration Points
- ws_handler.rs LaunchGame handler — game state transitions trigger adapter lifecycle
- event_loop.rs — adapter polling happens in game_check_interval tick
- safe_mode.rs — telemetry gating complements safe mode (different concern: timing vs subsystem)

</code_context>

<specifics>
## Specific Ideas

- The 5-second deferred connect is specifically for anti-cheat init — EAC/Javelin/EOS kernel drivers initialize during game startup and scan for memory access patterns
- iRacing SDK's named shared memory (IRSDKMemMapFileName) is explicitly safe per iRacing staff — the delay is precautionary
- LMU's rF2 shared memory is less documented — defer as a safety measure
- UDP telemetry (F1 25 port 20777) is read-only and officially supported — but socket lifecycle should still be clean

</specifics>

<deferred>
## Deferred Ideas

- Per-game telemetry delay configuration (different delays for different anti-cheat systems)
- Telemetry adapter hot-reload when game restarts mid-session

</deferred>
