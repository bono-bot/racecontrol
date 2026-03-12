# Requirements — Racing HUD & Wheelbase Safety

## v1 Requirements

### HUD Layout (Essentials Style)
- [ ] **HUD-01**: Redesign overlay to AC Essentials centered layout — large gear indicator (60-80pt), speed below gear, all data organized around center
- [ ] **HUD-02**: Full-width RPM horizontal bar (8-12px height), fills left-to-right with color zones: green (0-50%), yellow (50-75%), amber (75-90%), red (90%+ / redline). Read `max_rpm` from AC shared memory instead of hardcoded 18000
- [ ] **HUD-03**: Display current lap time from AC shared memory (`iCurrentTime`), previous lap (`iLastTime`), personal best (`iBestTime`)
- [ ] **HUD-04**: Display live sector times S1/S2/S3 as each sector completes with F1 color coding — purple (personal best sector), green (faster than previous), yellow (slower)
- [ ] **HUD-05**: Show session timer from AC `sessionTimeLeft` field (game time, not billing countdown)
- [ ] **HUD-06**: Show lap counter (current lap number)
- [ ] **HUD-07**: Invalid lap indicator — visual marker when track limits violated
- [ ] **HUD-08**: Speed display in KM/H (keep existing, reposition for Essentials layout)
- [ ] **HUD-09**: Use monospace font (Consolas) for all numeric values to prevent layout jitter

### HUD Data Accuracy
- [ ] **DATA-01**: Fix first lap not recorded (remove `last_lap_count > 0` guard in `assetto_corsa.rs`)
- [ ] **DATA-02**: Track best sector times independently (not just best lap's sectors) for accurate purple coloring
- [ ] **DATA-03**: Handle `iCurrentTime` reset to 0 at lap boundary — hold previous value for 2 poll cycles
- [ ] **DATA-04**: Check AC shared memory STATUS field — skip render when not LIVE (status != 2), prevent stale data display after game exit

### HUD Infrastructure
- [ ] **INFRA-01**: Cache GDI font handles in struct (create once at window init, destroy at window close) — fix 8 fonts/frame leak
- [ ] **INFRA-02**: Component-based paint system — `GdiResources` cache struct, section layout calculator, dispatcher pattern in `paint_hud()`

### FFB Safety
- [ ] **FFB-01**: Create `ffb_controller.rs` — open wheelbase on vendor HID interface (usage page `0xFF00`), send OpenFFBoard `estop` command (report ID `0xA1`, CmdID `0x0A`, Data=1)
- [ ] **FFB-02**: Wire `zero_force()` into session end — execute BEFORE `taskkill` in same `spawn_blocking` closure, with 100ms timeout (proceed to kill even if write fails)
- [ ] **FFB-03**: Send `zero_force()` unconditionally on rc-agent startup — recover from any prior unclean exit
- [ ] **FFB-04**: Graceful fallback — if wheelbase device not found or write fails, log warning and continue (no panic, no block)

### Watchdog Hardening
- [x] **WD-01**: Escalating restart cooldowns: 30s -> 2m -> 10m -> 30m per pod, resets on successful recovery
- [ ] **WD-02**: Post-restart self-test: verify rc-agent process running, WebSocket reconnected, and lock screen responsive within 60s of restart
- [x] **WD-03**: Email notification to Uday (usingh@racingpoint.in) when a pod hits max escalation or post-restart verification fails
- [x] **WD-04**: Email rate limiting: max 1 email per pod per 30 minutes, max 1 venue-wide email per 5 minutes (aggregate multiple pod failures)
- [ ] **WD-05**: Shared backoff state between pod_monitor and pod_healer to prevent duplicate restart attempts
- [x] **WD-06**: Configurable alert settings in racecontrol.toml: email recipient, enable/disable, script path, cooldown durations

## v2 Requirements (Deferred)
- [ ] F1 25 HUD support (separate adapter, different telemetry source)
- [ ] RPM arc/semicircle gauge (requires Direct2D migration)
- [ ] Live delta bar vs personal best
- [ ] Theoretical best lap calculation
- [ ] Proximity radar / relative bar
- [ ] DPI-aware scaling for different monitor configurations
- [ ] FFB strength adjustment per customer tier
- [ ] `ffb-guard.exe` standalone watchdog for crash-time FFB protection

## Out of Scope
- Engine/water temps on HUD — anti-feature for venue (confuses casual customers)
- Brake bias / diff settings display — pro-level, not for venue
- Telemetry graphs on overlay — that's the kiosk dashboard's job
- Billing details on HUD — user explicitly excluded
- Track mini-map — clutters overlay
- Drag-to-arrange widgets — must be locked for kiosk use
- Widget chrome/handles visible to customers — venue, not home setup

## Traceability

| REQ | Phase | Plan | Status |
|-----|-------|------|--------|
| FFB-01 | Phase 1: FFB Safety | P1.1 — FFB Controller and Session Lifecycle Integration | Pending |
| FFB-02 | Phase 1: FFB Safety | P1.1 — FFB Controller and Session Lifecycle Integration | Pending |
| FFB-03 | Phase 1: FFB Safety | P1.1 — FFB Controller and Session Lifecycle Integration | Pending |
| FFB-04 | Phase 1: FFB Safety | P1.1 — FFB Controller and Session Lifecycle Integration | Pending |
| INFRA-01 | Phase 2: HUD Infrastructure | P2.1 — GDI Resources Cache and Component Paint System | Pending |
| INFRA-02 | Phase 2: HUD Infrastructure | P2.1 — GDI Resources Cache and Component Paint System | Pending |
| HUD-01 | Phase 3: HUD Layout | P3.1 — Core Layout (Gear, Speed, RPM Bar) | Pending |
| HUD-02 | Phase 3: HUD Layout | P3.1 — Core Layout (Gear, Speed, RPM Bar) | Pending |
| HUD-03 | Phase 3: HUD Layout | P3.2 — Timing Display (Lap Times, Sectors, Session Timer, Lap Counter, Invalid Indicator) | Pending |
| HUD-04 | Phase 3: HUD Layout | P3.2 — Timing Display (Lap Times, Sectors, Session Timer, Lap Counter, Invalid Indicator) | Pending |
| HUD-05 | Phase 3: HUD Layout | P3.2 — Timing Display (Lap Times, Sectors, Session Timer, Lap Counter, Invalid Indicator) | Pending |
| HUD-06 | Phase 3: HUD Layout | P3.2 — Timing Display (Lap Times, Sectors, Session Timer, Lap Counter, Invalid Indicator) | Pending |
| HUD-07 | Phase 3: HUD Layout | P3.2 — Timing Display (Lap Times, Sectors, Session Timer, Lap Counter, Invalid Indicator) | Pending |
| HUD-08 | Phase 3: HUD Layout | P3.1 — Core Layout (Gear, Speed, RPM Bar) | Pending |
| HUD-09 | Phase 3: HUD Layout | P3.1 — Core Layout (Gear, Speed, RPM Bar) | Pending |
| DATA-01 | Phase 4: Data Accuracy | P4.1 — Timing Data Fixes and State Validation | Pending |
| DATA-02 | Phase 4: Data Accuracy | P4.1 — Timing Data Fixes and State Validation | Pending |
| DATA-03 | Phase 4: Data Accuracy | P4.1 — Timing Data Fixes and State Validation | Pending |
| DATA-04 | Phase 4: Data Accuracy | P4.1 — Timing Data Fixes and State Validation | Pending |
| WD-01 | Phase 5: Watchdog Hardening | 05-01 (Foundation) + 05-02 (Integration) | Pending |
| WD-02 | Phase 5: Watchdog Hardening | 05-02 (Integration) | Pending |
| WD-03 | Phase 5: Watchdog Hardening | 05-01 (Foundation) + 05-02 (Integration) | Pending |
| WD-04 | Phase 5: Watchdog Hardening | 05-01 (Foundation) | Pending |
| WD-05 | Phase 5: Watchdog Hardening | 05-02 (Integration) | Pending |
| WD-06 | Phase 5: Watchdog Hardening | 05-01 (Foundation) | Pending |

---
*Last updated: 2026-03-12 — 25 requirements across 5 phases*
