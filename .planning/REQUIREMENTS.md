# Requirements: Racing Point Operations — v11.1 Pre-Flight Session Checks

**Defined:** 2026-03-21
**Core Value:** Customers see their lap times, compete on leaderboards, and compare telemetry

## v11.1 Requirements

Pre-flight checks that run before every customer session, auto-fix failures, and alert staff only when auto-fix fails.

### Pre-Flight Framework

- [x] **PF-01**: Pre-flight checks run on every BillingStarted before PIN entry is shown
- [x] **PF-02**: All checks run concurrently via tokio::join! with 5-second hard timeout
- [x] **PF-03**: Failed checks attempt one auto-fix before reporting failure
- [x] **PF-04**: Lock screen shows "Maintenance Required" state when pre-flight fails after auto-fix
- [x] **PF-05**: PreFlightFailed AgentMessage sent to racecontrol with failed check details
- [ ] **PF-06**: Pod auto-retries pre-flight every 30s while in MaintenanceRequired state
- [x] **PF-07**: Pre-flight can be disabled per-pod via rc-agent.toml config flag

### Hardware Checks

- [x] **HW-01**: Wheelbase HID connected (FfbController::zero_force returns Ok(true))
- [x] **HW-02**: ConspitLink process running (two-stage: process alive + config files valid)
- [x] **HW-03**: Auto-fix: restart ConspitLink process if not running

### System & Network Checks

- [x] **SYS-01**: No orphaned game process from previous session (kill if found)
- [ ] **SYS-02**: No stuck billing session (billing_active should be false before new session)
- [ ] **SYS-03**: Disk space > 1GB free
- [ ] **SYS-04**: Memory > 2GB free
- [ ] **NET-01**: WebSocket connected and stable (connected for >10s, not flapping)

### Display Checks

- [ ] **DISP-01**: Lock screen HTTP server responding on port 18923
- [ ] **DISP-02**: Lock screen window position validated via GetWindowRect (centered on primary monitor)

### Staff Visibility

- [ ] **STAFF-01**: Kiosk dashboard shows pre-flight status badge per pod (pass/fail/maintenance)
- [ ] **STAFF-02**: Staff can manually clear MaintenanceRequired state from kiosk dashboard
- [ ] **STAFF-03**: Pod marked unavailable in fleet health while in MaintenanceRequired state
- [ ] **STAFF-04**: Pre-flight failure alerts rate-limited (no flood on repeated failures)

## v12.0+ Requirements

Deferred to future release.

### Extended Checks

- **PF-08**: AC content path accessible (Steam path + race.ini writable)
- **PF-09**: GPU temperature within safe range before session
- **PF-10**: Pre-flight history log (last N results per pod for debugging)

## Out of Scope

| Feature | Reason |
|---------|--------|
| Screenshot-based display validation | rc-agent runs Session 0, cannot capture Session 1 display; HTTP probe is sufficient |
| Full self_test.rs re-run per session | 22 probes take 10+ seconds; warm/cold semantics differ; 8-10 targeted checks are better |
| Customer-visible pre-flight status | Customers see "Connecting..." then PIN entry; maintenance is staff-only concern |
| Pre-flight on startup | self_test.rs already handles startup health; pre-flight is session-scoped only |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| PF-01 | Phase 97 | Complete |
| PF-02 | Phase 97 | Complete |
| PF-03 | Phase 97 | Complete |
| PF-07 | Phase 97 | Complete |
| HW-01 | Phase 97 | Complete |
| HW-02 | Phase 97 | Complete |
| HW-03 | Phase 97 | Complete |
| SYS-01 | Phase 97 | Complete |
| PF-04 | Phase 98 | Complete |
| PF-05 | Phase 98 | Complete |
| PF-06 | Phase 98 | Pending |
| DISP-01 | Phase 98 | Pending |
| DISP-02 | Phase 98 | Pending |
| SYS-02 | Phase 99 | Pending |
| SYS-03 | Phase 99 | Pending |
| SYS-04 | Phase 99 | Pending |
| NET-01 | Phase 99 | Pending |
| STAFF-04 | Phase 99 | Pending |
| STAFF-01 | Phase 100 | Pending |
| STAFF-02 | Phase 100 | Pending |
| STAFF-03 | Phase 100 | Pending |

**Coverage:**
- v11.1 requirements: 21 total
- Mapped to phases: 21
- Unmapped: 0

---
*Requirements defined: 2026-03-21*
*Last updated: 2026-03-21 after roadmap creation — all 21 requirements mapped to phases 97-100*
