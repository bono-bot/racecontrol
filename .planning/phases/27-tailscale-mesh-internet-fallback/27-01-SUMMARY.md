---
phase: 27-tailscale-mesh-internet-fallback
plan: 01
subsystem: infra
tags: [tailscale, mesh, relay, serde, config, rust, bono]

# Dependency graph
requires: []
provides:
  - BonoConfig struct in config.rs with relay_port=8099 default and all Option fields
  - bono_relay.rs module with BonoEvent enum (6 variants), RelayCommand enum (3 variants), spawn() stub
  - 5 new unit tests (bono_config_defaults, bono_config_explicit, spawn_disabled, spawn_no_url, event_serialization)
affects:
  - 27-02 (Wave 1 implementation — wires tokio task into spawn(), uses BonoConfig)
  - crates/racecontrol/src/state.rs (AppState used by spawn())

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "BonoConfig follows CloudConfig pattern: manual Default impl to preserve non-zero defaults when struct is constructed without TOML"
    - "BonoEvent/RelayCommand use #[serde(tag=type, content=data, rename_all=snake_case)] — same tagged enum shape as AgentMessage in rc-common"
    - "spawn() guard pattern: check enabled first, then check webhook_url — matches cloud_sync.rs spawn()"

key-files:
  created:
    - crates/racecontrol/src/bono_relay.rs
  modified:
    - crates/racecontrol/src/config.rs
    - crates/racecontrol/src/lib.rs

key-decisions:
  - "relay_port default is 8099 (not 8081) — PortAllocator reserves 8081-8096 for AC dedicated server sessions"
  - "BonoConfig uses manual Default impl (not #[derive(Default)]) — derived Default gives relay_port=0, not 8099"
  - "spawn_disabled and spawn_no_url test guard logic in isolation without AppState — white-box tests of branch conditions"

patterns-established:
  - "Config struct field with #[serde(default)] + manual Default impl: use this pattern for any config struct with non-zero/non-false defaults"

requirements-completed:
  - TS-01
  - TS-02
  - TS-03
  - TS-04

# Metrics
duration: 15min
completed: 2026-03-16
---

# Phase 27 Plan 01: Tailscale Relay Foundation Summary

**BonoConfig struct in config.rs (relay_port=8099) + bono_relay.rs skeleton with BonoEvent/RelayCommand enums and 5 passing unit tests**

## Performance

- **Duration:** ~15 min
- **Started:** 2026-03-16T11:30:00Z
- **Completed:** 2026-03-16T11:45:00Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments

- Added BonoConfig to config.rs with correct relay_port=8099 default (avoids AC server port range 8081-8096)
- Created bono_relay.rs with BonoEvent enum (6 variants: SessionStart, SessionEnd, LapRecorded, PodOffline, PodOnline, BillingEnd) and RelayCommand enum (3 variants)
- spawn() stub mirrors cloud_sync.rs pattern: early return when disabled or no webhook_url, log appropriate messages
- 5 new tests pass, full suite at 247 tests with 0 regressions

## Task Commits

Each task was committed atomically:

1. **Task 1: Add BonoConfig to config.rs with defaults test** - `f3b95ea` (feat)
2. **Task 2: Create bono_relay.rs skeleton with tests** - `07c0501` (feat)

## Files Created/Modified

- `crates/racecontrol/src/config.rs` - Added BonoConfig struct, bono field on Config, manual Default impl, 2 config tests
- `crates/racecontrol/src/bono_relay.rs` - New file: BonoEvent enum, RelayCommand enum, spawn() stub, 3 unit tests
- `crates/racecontrol/src/lib.rs` - Added `pub mod bono_relay;`

## Decisions Made

- **relay_port=8099 not 8081:** PortAllocator in state.rs uses `PortAllocator::new(9600, 8081, 16)` which reserves 8081-8096 for AC dedicated server HTTP. Using 8081 would conflict with session 1's HTTP port.
- **Manual Default instead of #[derive(Default)]:** Rust's derived Default gives `u16::default()` = 0 for relay_port. The serde `default = "default_relay_port"` annotation only fires during TOML deserialization, not when Default::default() is called directly (e.g. in default_config()). Manual impl fixes both paths.
- **White-box guard tests:** spawn_disabled and spawn_no_url test the boolean guard logic directly without constructing AppState — consistent with the existing test strategy in this codebase.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Manual Default impl for BonoConfig instead of derived**
- **Found during:** Task 1 (bono_config_defaults test ran and relay_port was 0 not 8099)
- **Issue:** #[derive(Default)] produces relay_port=0. The serde default fn only fires during deserialization; Default::default() called in default_config() bypasses it.
- **Fix:** Replaced `#[derive(Debug, Default, Deserialize)]` with `#[derive(Debug, Deserialize)]` + explicit `impl Default for BonoConfig` that calls `default_relay_port()` — same fix pattern as WatchdogConfig.
- **Files modified:** crates/racecontrol/src/config.rs
- **Verification:** bono_config_defaults test passes (relay_port=8099 confirmed)
- **Committed in:** f3b95ea (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (Rule 1 - Bug)
**Impact on plan:** Necessary correctness fix — without it the defaults test would fail and relay_port would silently be 0 in production when [bono] section is absent from TOML. No scope creep.

## Issues Encountered

None beyond the auto-fixed relay_port default bug above.

## User Setup Required

None - no external service configuration required. Tailscale setup and webhook_url configuration will be documented in a later plan when the full relay is wired up.

## Next Phase Readiness

- Wave 1 (Plan 27-02) can now implement the tokio event push loop and relay endpoint — BonoConfig and BonoEvent types are established
- spawn() stub has a clear TODO marker for Wave 1 to fill in
- 247 tests green, no regressions

## Self-Check: PASSED

- FOUND: crates/racecontrol/src/bono_relay.rs
- FOUND: crates/racecontrol/src/config.rs (BonoConfig added)
- FOUND: .planning/phases/27-tailscale-mesh-internet-fallback/27-01-SUMMARY.md
- FOUND: commit f3b95ea (BonoConfig)
- FOUND: commit 07c0501 (bono_relay.rs)
- FOUND: commit 0990a05 (metadata)
- All 247 tests green confirmed

---
*Phase: 27-tailscale-mesh-internet-fallback*
*Completed: 2026-03-16*
