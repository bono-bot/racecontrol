---
gsd_state_version: 1.0
milestone: v2.0
milestone_name: Kiosk URL Reliability
status: active
stopped_at: Phase 7 Plan 01 COMPLETE — server pinned to .23, inventory done
last_updated: "2026-03-14"
last_activity: 2026-03-14 — Plan 07-01 executed, server at .23, Plan 07-02 next
progress:
  total_phases: 6
  completed_phases: 1
  total_plans: 4
  completed_plans: 3
  percent: 25
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-03-13)

**Core value:** Every URL in the venue always works — staff kiosk, customer PIN grid, pod lock screens are permanently accessible with zero manual intervention.
**Current focus:** Phase 7 Plan 01 COMPLETE — server pinned to .23, Plan 02 next

## Current Position

Phase: 7 of 11 (Server-Side Pinning) — IN PROGRESS
Plan: 1 of 2 in current phase — Plan 01 done, Plan 02 next
Status: Server at .23, inventory complete, ready for deployment (Plan 02)
Last activity: 2026-03-14 — DHCP reservation + server inventory via pod-agent

Progress: [██░░░░░░░░] 25%

## Performance Metrics

**Velocity:**
- Total plans completed: 3 (v2.0)
- Average duration: ~13min
- Total execution time: ~40min

**By Phase:**

| Phase | Plans | Total | Avg/Plan |
|-------|-------|-------|----------|
| 06-diagnosis P01 | 1 | 2 tasks, 1 file | 15min |
| 06-diagnosis P02 | 1 | 2 tasks, 1 file | 10min |
| 07-server-pinning P01 | 1 | 2 tasks, 0 files | 15min |

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
- [Phase 07-server-pinning]: Router is TP-Link EX220 (not Xiaomi as previously documented)
- [Phase 07-server-pinning]: DHCP reservation created: MAC BC-FC-E7-2C-F2-CE → IP .23
- [Phase 07-server-pinning]: Server inventory: No Node.js, no racecontrol.toml, no rc-core — all must be deployed in Plan 02
- [Phase 07-server-pinning]: Server auto-login: ADMIN user, Session 2, Active — HKLM Run keys will fire at boot
- [Phase 07-server-pinning]: C:\RacingPoint contains nginx + pod-agent only — clean target for deployment

### Pending Todos

None.

### Blockers/Concerns

- ~~Server MAC address needed~~ → RESOLVED: BC-FC-E7-2C-F2-CE
- rc-core CORS may need `kiosk.rp` origin guard — verify during Phase 7 before going live
- ~~Kiosk port confirmation~~ → RESOLVED: 3300 (not listening on server, free to use)
- ~~Server DHCP lease expires nightly~~ → RESOLVED: DHCP reservation pinned to .23
- Node.js must be installed on server before Plan 02 can deploy kiosk

## Session Continuity

Last session: 2026-03-14
Stopped at: Phase 7 Plan 01 COMPLETE — server at .23, Plan 02 next
Resume file: None
