# Research Summary — Racing HUD & Wheelbase FFB Safety

## Key Findings

### Stack
- **Stay on Win32 GDI** — existing overlay architecture is solid, no need for Direct2D/DirectX migration
- **Use monospace fonts** (Consolas) for all numeric values to prevent layout jitter
- **OpenFFBoard vendor report interface** (report ID `0xA1`, usage page `0xFF00`) is the correct way to send FFB commands — independent of DirectInput game FFB
- **hidapi `write()`** with `windows-native` feature — buffer must start with report ID byte
- **AC shared memory** provides `iCurrentTime`, `iLastTime`, `iBestTime`, `sessionTimeLeft`, `currentSectorIndex`, `lastSectorTime` — all needed for HUD

### Features — Table Stakes for Venue HUD
1. Speed + Gear (large, centered)
2. RPM bar with color zones
3. Current lap time (from AC shared memory)
4. Previous lap + Personal best
5. Sector times S1/S2/S3 with F1 color coding (purple/green/yellow)
6. Session timer (from AC `sessionTimeLeft`)
7. Lap counter
8. Invalid lap indicator

### Features — Anti-Features (keep OFF screen)
- Engine temps, brake bias, diff settings, TC/ABS counters
- Telemetry graphs, input traces
- Billing details, PC system metrics
- Widget drag handles, track mini-map

### Architecture Recommendations
1. **Component-based paint** — `HudComponent` trait, `GdiResources` cache (fonts created once), `SectionRect` layout system
2. **FFB controller** — new `ffb_controller.rs` module, separate from `driving_detector.rs` (read-only HID). Use OpenFFBoard `estop` command (CmdID `0x0A`, Data=1)
3. **Ordering guarantee** — `ffb.zero_force()` THEN `taskkill` in same `spawn_blocking` closure
4. **Timer sync** — Use AC's `sessionTimeLeft` (offset 152) for session countdown, `iCurrentTime` for lap timer
5. **FFB zero on startup** — unconditionally, to recover from any prior unclean exit

### Critical Pitfalls to Address
| ID | Pitfall | Prevention | Phase |
|----|---------|-----------|-------|
| P-01 | GDI font leak (8 fonts/frame) | Cache in GdiResources, create once | HUD refactor |
| P-07 | Wrong FFB report format | Use `estop` vendor cmd, verify firmware version first | FFB |
| P-08 | Blocking HID write on async thread | `spawn_blocking` + 100ms timeout | FFB |
| P-12 | lastSectorTime only valid at transition | Poll at 50ms (20Hz) minimum | HUD data |
| P-13 | iCurrentTime resets to 0 at lap boundary | Hold previous value for 2 poll cycles | HUD data |
| P-14 | First lap blocked by `last_lap_count > 0` guard | Remove guard, use time threshold | HUD data |
| P-15 | Best sector != best lap's sector | Track best sectors independently | HUD data |
| P-18 | FFB cleanup skipped if rc-agent killed | Send FFB zero on startup | FFB safety |
| P-20 | ConspitLink may overwrite zero-force | Use constant-force magnitude 0 OR briefly close ConspitLink | FFB safety |
| P-22 | Stale shared memory after game exit | Check STATUS field, disconnect adapter | HUD data |

### Build Order (from Architecture research)
1. **FFB Safety first** — `ffb_controller.rs` + wire into session lifecycle (safety critical)
2. **GDI resources cache** — eliminate font leak, prep for HUD refactor
3. **HUD component system** — trait + layout calculator
4. **Essentials layout** — centered gear, RPM arc, sector display
5. **Data accuracy fixes** — sector timing, lap boundary, first lap
6. **Timer sync** — sessionTimeLeft integration
7. **Pod 8 end-to-end validation**

---
*Research completed: 2026-03-11 | 4 parallel researchers | 2,307 lines of research*
