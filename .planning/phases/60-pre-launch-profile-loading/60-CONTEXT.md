# Phase 60: Pre-Launch Profile Loading - Context

**Gathered:** 2026-03-24
**Status:** Ready for planning

<domain>
## Phase Boundary

rc-agent pre-loads the correct ConspitLink FFB preset BEFORE spawning the game process, so the wheelbase has the right force feedback parameters from first customer input. Provides a safe fallback (reduced force, centered spring) for unrecognized games. Does NOT create custom presets (Phase 61) or push configs fleet-wide (Phase 62).

</domain>

<decisions>
## Implementation Decisions

### Pre-Launch Hook Location
- Insert in `ws_handler.rs` LaunchGame handler, after safe mode entry but before game process spawn
- New function `pre_load_game_preset(sim_type: SimType)` in `ffb_controller.rs` — called from LaunchGame handler
- Lookup table maps `SimType` → ConspitLink game key string (reuses VENUE_GAME_KEYS constants from Phase 59)
- Brief block (2-3s max) before game spawn to ensure preset is loaded — FFB must be correct from first input

### Preset Loading Mechanism
- Wait 3s for ConspitLink auto-detect (Phase 59) to switch preset first
- If CL auto-detect doesn't switch within 3s timeout, escalate: force preset via Global.json `LastUsedPreset` field write + CL restart
- Only restart CL if auto-detect failed — avoid unnecessary restarts
- If CL is not running, use `ensure_conspit_link_running()` from existing watchdog before attempting preset load

### Safe Fallback for Unrecognized Games
- "Unrecognized" = SimType variant not in the SimType→key lookup table (e.g., Forza, ForzaHorizon5)
- Safe default: 50% power cap via HID `axis.power` command + centered spring via `axis.idlespring` — reuses Phase 57 HID commands
- Log `tracing::warn` with game name — visible in pod logs, no WhatsApp (not critical)
- Restore normal power cap (80%) on session end — `safe_session_end()` already handles this

### Claude's Discretion
- Exact 3s timeout implementation (tokio::time::timeout vs polling loop)
- Whether to check ConspitLink's current preset before waiting (skip wait if already correct)
- Error handling for HID command failures during fallback
- Test structure and naming

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Game Launch Path
- `crates/rc-agent/src/ws_handler.rs` lines 283-312 — LaunchGame handler (insertion point for pre-load hook)
- `crates/rc-common/src/types.rs` lines 8-20 — SimType enum (all game variants)

### FFB Controller (Phase 57-59 code)
- `crates/rc-agent/src/ffb_controller.rs` — VENUE_GAME_KEYS (line 592), ensure_auto_switch_config(), restart_conspit_link_hardened(), HID commands (idlespring, axis.power)
- `.planning/phases/59-auto-switch-configuration/59-CONTEXT.md` — Auto-switch decisions (prerequisite)
- `.planning/phases/57-session-end-safety/57-CONTEXT.md` — HID commands, power cap, session-end sequence

### Architecture
- `.planning/research/conspit-link/ARCHITECTURE.md` — ConspitLink auto-detection flow, config file roles, anti-patterns

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `VENUE_GAME_KEYS` in ffb_controller.rs — game key strings for ConspitLink lookup
- `restart_conspit_link_hardened(false)` — graceful CL restart with config backup/verify
- `send_vendor_cmd_to_class()` — HID command sender for power cap and idlespring
- `ensure_conspit_link_running()` — watchdog that delegates to hardened restart
- `is_process_running("ConspitLink2.0.exe")` — process check

### Established Patterns
- Testable `_impl(Option<&Path>)` pattern for filesystem-dependent functions (Phase 58)
- `spawn_blocking` for synchronous operations in async context (Phase 59 startup wiring)
- Compare-before-write to avoid unnecessary CL restarts (Phase 59)

### Integration Points
- `ws_handler.rs` LaunchGame handler — where pre-load hook inserts (between safe_mode entry and game spawn)
- `safe_session_end()` — already restores power cap on session end, handles fallback recovery
- `conn.current_sim_type` — tracks which game is active, available in the handler

</code_context>

<specifics>
## Specific Ideas

- The 3s auto-detect wait is a "trust but verify" approach — Phase 59's auto-switch should work, Phase 60 is the safety net
- Power cap for unrecognized games (50%) is intentionally conservative — better to feel weak than to risk 8Nm torque on an untested game
- This pairs with Phase 59: auto-switch handles the common path, pre-launch handles edge cases and provides guaranteed correctness

</specifics>

<deferred>
## Deferred Ideas

- Custom venue-tuned .Base presets per game — Phase 61 (FFB Preset Tuning)
- Fleet-wide config push — Phase 62 (superseded by v22.0)
- Reading ConspitLink's current active preset programmatically — no API exists, would need log parsing

</deferred>

---

*Phase: 60-pre-launch-profile-loading*
*Context gathered: 2026-03-24*
