---
gsd_state_version: 1.0
milestone: v2.0
milestone_name: Kiosk URL Reliability
status: active
stopped_at: Phase 6 COMPLETE — all 4 DIAG requirements fulfilled
last_updated: "2026-03-13"
last_activity: 2026-03-13 — Phase 6 Diagnosis complete, Phase 7 unblocked
progress:
  total_phases: 6
  completed_phases: 1
  total_plans: 2
  completed_plans: 2
  percent: 17
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-03-13)

**Core value:** Every URL in the venue always works — staff kiosk, customer PIN grid, pod lock screens are permanently accessible with zero manual intervention.
**Current focus:** Phase 6 COMPLETE — ready for Phase 7

## Current Position

Phase: 6 of 11 (Diagnosis) — COMPLETE
Plan: 2 of 2 in current phase — ALL DONE
Status: Phase 6 complete, Phase 7 ready to plan
Last activity: 2026-03-13 — DIAG-02/04 collected via pod-agent on server

Progress: [█░░░░░░░░░] 17%

## Performance Metrics

**Velocity:**
- Total plans completed: 2 (v2.0)
- Average duration: ~12min
- Total execution time: ~25min

**By Phase:**

| Phase | Plans | Total | Avg/Plan |
|-------|-------|-------|----------|
| 06-diagnosis P01 | 1 | 2 tasks, 1 file | 15min |
| 06-diagnosis P02 | 1 | 2 tasks, 1 file | 10min |

## Accumulated Context

### Decisions

- v2.0 kickoff: NSSM banned — use HKLM Run key for Session 1 GUI processes, sc.exe for headless services
- v2.0 kickoff: Phase 10 (dashboard) depends on Phase 7, not Phase 9 — parallel track possible
- v2.0 kickoff: Phase 11 (branding) depends on Phase 8 — lock screen infrastructure must be in place first
- v2.0 kickoff: Two-layer IP pinning — DHCP reservation at router AND static NIC backup
- [Phase 06-diagnosis]: DIAG-01: Only Pod 8 has persistent log file — Pods 1-7 start script doesn't redirect stdout
- [Phase 06-diagnosis]: DIAG-01: rc-core unreachable on .23:8080 is universal root cause — all pods show disconnected lock screen
- [Phase 06-diagnosis]: DIAG-03: All 8 pods on Edge 145.0.3800.97, StartupBoost+BackgroundMode default-enabled, EdgeUpdate STOPPED but not DISABLED — all 8 need Phase 9 remediation
- [Phase 06-diagnosis]: DIAG-02: Ports 3300 (kiosk) and 8080 (rc-core) NOT listening on server — both need Phase 7 deployment
- [Phase 06-diagnosis]: DIAG-04: Server MAC = BC-FC-E7-2C-F2-CE, IP drifted .51→.23→.4, DHCP lease expires nightly
- [Phase 06-diagnosis]: Server HAS pod-agent on 8090 — MEMORY was wrong (no RDP needed for server management)
- [Phase 06-diagnosis]: .23 is NOT the server — unknown device (phone/tablet) with locally administered MAC
- [Phase 06-diagnosis]: java.exe on server port 45021 — unknown, not blocking, investigate if conflicts arise

### Pending Todos

None.

### Blockers/Concerns

- ~~Server MAC address needed~~ → RESOLVED: BC-FC-E7-2C-F2-CE
- rc-core CORS may need `kiosk.rp` origin guard — verify during Phase 7 before going live
- ~~Kiosk port confirmation~~ → RESOLVED: 3300 (not listening on server, free to use)
- Server DHCP lease expires nightly (~01:05) — Phase 7 must pin IP ASAP

## Session Continuity

Last session: 2026-03-13
Stopped at: Phase 6 COMPLETE — all 4 DIAG requirements fulfilled
Resume file: None
