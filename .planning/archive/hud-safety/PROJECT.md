# Racing HUD & Wheelbase Safety Overhaul

## What This Is

A focused improvement to rc-agent's Racing HUD overlay and wheelbase session-end handling. The HUD needs to be redesigned to follow Assetto Corsa's Essentials app layout — large centered gear, prominent RPM bar, visible sector times and lap times with proper game timer sync. Additionally, the wheelbase must send a zero-force command when the game closes to prevent dangerous uncontrolled rotation.

## Core Value

**Customers must never be at risk of wrist injury from the wheelbase, and drivers must see their lap/sector data clearly during a session.**

## Requirements

### Validated

- ✓ Win32 GDI overlay renders on top of game — existing (`overlay.rs`)
- ✓ Speed, gear display working — existing
- ✓ RPM bar (top 4px color bar) functional — existing
- ✓ HID input reading from Conspit Ares wheelbase (VID:0x1209 PID:0xFFB0) — existing
- ✓ AC shared memory telemetry adapter populates sector times — existing (`assetto_corsa.rs`)
- ✓ Session cleanup kills game processes on billing end — existing (`ac_launcher.rs`)
- ✓ Lap completion tracking with previous/best comparison — existing

### Active

- [ ] **HUD-01**: Redesign overlay to AC Essentials layout — large centered gear indicator, RPM arc/bar prominently visible
- [ ] **HUD-02**: Display sector times (S1, S2, S3) live as each sector completes, with F1-style color coding (purple=best, green=faster, yellow=slower)
- [ ] **HUD-03**: Display lap times — current lap timer, previous lap, personal best
- [ ] **HUD-04**: Increase RPM font/indicator size significantly (currently 16pt, needs ~24-32pt or visual arc)
- [ ] **HUD-05**: Sync HUD timer with AC game session time (from shared memory `iSessionTime`), not billing countdown
- [ ] **HUD-06**: Show lap count (current lap number)
- [ ] **FFB-01**: Send zero-force HID output report to wheelbase before killing game process on session end
- [ ] **FFB-02**: Send zero-force on any game crash/unexpected exit detected by watchdog

### Out of Scope

- F1 25 HUD redesign — AC only for now, F1 comes later
- FFB strength adjustment UI — just need safe shutdown for now
- Full telemetry dashboard (that's the kiosk's job) — HUD is glanceable only
- Billing countdown on HUD — user wants game time, not billing time

## Context

**Current HUD state:** The overlay bar renders but sector times and lap times are showing as dashes (`--.-`). Speed and gear work. RPM bar is a thin 4px strip across the top. The overall layout is a horizontal 6-section bar — functional but not optimized for glanceability at speed.

**AC Essentials reference:** The Essentials app for Assetto Corsa is the gold standard for in-game HUDs. Key design elements:
- Centered gear number (very large, 60-80pt equivalent)
- RPM shown as arc/bar with color zones (green→yellow→red)
- Sector splits below gear with color coding
- Clean dark semi-transparent background
- Minimal chrome, maximum readability

**Wheelbase safety:** When billing ends, `ac_launcher.rs` calls `taskkill /IM acs.exe`. The game dies instantly, and the wheelbase (Conspit Ares 8Nm via OpenFFBoard firmware) retains its last FFB state — which can be full force to one side. No zero-force HID output report is sent. This is a **real injury risk** — 8Nm of sudden uncontrolled torque can hurt wrists and damage the wheelbase mount.

**HID protocol:** rc-agent already opens the device for reading (`hidapi`, `read_timeout`). Writing requires `write()` or `send_feature_report()`. OpenFFBoard uses USB HID PID reports for FFB — a zero-force report needs to be determined from the OpenFFBoard protocol spec.

**Codebase map:** See `.planning/codebase/` for full architecture. Key files:
- `crates/rc-agent/src/overlay.rs` (848 lines) — HUD rendering
- `crates/rc-agent/src/driving_detector.rs` (282 lines) — HID input
- `crates/rc-agent/src/main.rs` (1416 lines) — HID monitor, session lifecycle
- `crates/rc-agent/src/ac_launcher.rs` (987 lines) — session cleanup
- `crates/rc-agent/src/sims/assetto_corsa.rs` (417 lines) — AC telemetry

## Constraints

- **Platform**: Windows 11, Win32 GDI only (no web overlay, no browser dependency)
- **Performance**: Paint must complete in <5ms at 200ms intervals (60fps game underneath)
- **Hardware**: Conspit Ares 8Nm (OpenFFBoard VID:0x1209 PID:0xFFB0) — must not brick device
- **Safety**: FFB zero-force MUST execute before game process kill. Failure = default to safe (center/stop)
- **Scope**: AC only for this milestone. F1 25 adapter untouched.

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Essentials-style centered layout over horizontal bar | User request + Essentials is proven UX for racing | — Pending |
| Game session time over billing countdown | Driver cares about track time, not money timer | — Pending |
| Win32 GDI (keep existing) over switching to DirectX | Works reliably, no dependency changes, fast enough | — Pending |
| Zero-force via HID write before taskkill | Only reliable way to disarm FFB before game death | — Pending |
| AC only, F1 later | Focus and ship quality for most-used sim first | — Pending |

---
*Last updated: 2026-03-11 after initialization*
