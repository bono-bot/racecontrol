---
gsd_state_version: 1.0
milestone: v2.0
milestone_name: Kiosk URL Reliability
status: active
stopped_at: Completed 08-pod-lock-screen-hardening Plan 03 — release binary built and staged, awaiting human verify checkpoint
last_updated: "2026-03-14T00:14:44.033Z"
last_activity: 2026-03-14 — rc-core reverse proxy + CORS fix committed, 21MB binary staged
progress:
  total_phases: 6
  completed_phases: 3
  total_plans: 7
  completed_plans: 7
  percent: 96
---

---
gsd_state_version: 1.0
milestone: v2.0
milestone_name: Kiosk URL Reliability
status: active
stopped_at: Phase 7 Plan 02 COMPLETE — rc-core proxy committed, deployment pending physical server access
last_updated: "2026-03-14"
last_activity: 2026-03-14 — rc-core reverse proxy committed, binary staged, server deploy blocked by SAC
progress:
  [██████████] 96%
  completed_phases: 2
  total_plans: 4
  completed_plans: 4
  percent: 50
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-03-13)

**Core value:** Every URL in the venue always works — staff kiosk, customer PIN grid, pod lock screens are permanently accessible with zero manual intervention.
**Current focus:** Phase 7 COMPLETE (code) — server deployment pending physical access, Phase 8 next after deploy

## Current Position

Phase: 7 of 11 (Server-Side Pinning) — Code complete, deployment pending
Plan: 2 of 2 in current phase — BOTH DONE
Status: rc-core proxy code committed (ea9a728, 3db7403), binary staged at deploy-staging/racecontrol.exe. Server deployment blocked by Windows SAC — requires physical access to .23.
Last activity: 2026-03-14 — rc-core reverse proxy + CORS fix committed, 21MB binary staged

Progress: [███░░░░░░░] 35%

## Performance Metrics

**Velocity:**
- Total plans completed: 4 (v2.0)
- Average duration: ~20min
- Total execution time: ~80min

**By Phase:**

| Phase | Plans | Total | Avg/Plan |
|-------|-------|-------|----------|
| 06-diagnosis P01 | 1 | 2 tasks, 1 file | 15min |
| 06-diagnosis P02 | 1 | 2 tasks, 1 file | 10min |
| 07-server-pinning P01 | 1 | 2 tasks, 0 files | 15min |
| 07-server-pinning P02 | 1 | 2 tasks, 1 file | 30min |
| Phase 08-pod-lock-screen-hardening P02 | 5min | 1 tasks | 1 files |
| Phase 08-pod-lock-screen-hardening P01 | 25 | 2 tasks | 3 files |
| Phase 08-pod-lock-screen-hardening P03 | 15 | 1 tasks | 1 files |

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
- [Phase 07-server-pinning P02]: Windows SAC blocks node.exe from accepting network connections — route kiosk traffic through rc-core Axum reverse proxy instead of direct port 3300 access
- [Phase 07-server-pinning P02]: SAC also blocks pod-agent/WinRM on server — physical access required for server deployment; code committed and binary staged at deploy-staging
- [Phase 07-server-pinning P02]: Kiosk proxy paths: /kiosk* and /_next/* forwarded to localhost:3300; access point is kiosk.rp:8080/kiosk not :3300
- [Phase 08-pod-lock-screen-hardening]: Use scheduled-task watchdog (not loop) for rc-agent — one-shot script invoked by schtasks /SC MINUTE, calls start-rcagent.bat on crash
- [Phase 08-pod-lock-screen-hardening]: StartupConnecting classified as is_idle_or_blanked()=true — pod not ready for customers during startup, consistent with Disconnected and Hidden
- [Phase 08-pod-lock-screen-hardening]: wait_for_self_ready() never panics — 5s deadline with graceful log warning, ensuring agent always starts even if HTTP server is slow
- [Phase 08-pod-lock-screen-hardening]: Browser opened once by show_startup_connecting() at boot; state changes picked up by 3s JS reload, no re-launch needed on transitions
- [Phase 08-pod-lock-screen-hardening]: rc-agent binary size is 6.7MB (not 15-25MB) — plan estimate was based on rc-core; size is correct and consistent with prior builds

### Pending Todos

None.

### Blockers/Concerns

- ~~Server MAC address needed~~ → RESOLVED: BC-FC-E7-2C-F2-CE
- ~~rc-core CORS may need `kiosk.rp` origin guard~~ → RESOLVED: added in ea9a728
- ~~Kiosk port confirmation~~ → RESOLVED: 3300 (not listening on server, free to use)
- ~~Server DHCP lease expires nightly~~ → RESOLVED: DHCP reservation pinned to .23
- ~~Node.js must be installed on server~~ → RESOLVED: kiosk runs through rc-core proxy, no direct node.exe network access needed
- **OPEN: Server deployment requires physical access** — SAC blocks WinRM/pod-agent remote exec. Steps documented in 07-02-SUMMARY.md "User Setup Required". Binary staged at deploy-staging/racecontrol.exe (21MB).

## Session Continuity

Last session: 2026-03-14T00:14:44.030Z
Stopped at: Completed 08-pod-lock-screen-hardening Plan 03 — release binary built and staged, awaiting human verify checkpoint
Resume file: None
