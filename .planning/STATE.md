---
gsd_state_version: 1.0
milestone: v2.0
milestone_name: Kiosk URL Reliability
status: active
stopped_at: "Completed 11-02-PLAN.md — Wallpaper URL staff UI: Pod Display section added to kiosk settings page"
last_updated: "2026-03-14T03:54:26.580Z"
last_activity: 2026-03-14 — 10-01 backend lockdown routes + 10 new unit tests committed (564b8ee)
progress:
  total_phases: 6
  completed_phases: 6
  total_plans: 12
  completed_plans: 12
---

---
gsd_state_version: 1.0
milestone: v2.0
milestone_name: Kiosk URL Reliability
status: active
stopped_at: Phase 10-01 staff lockdown API routes complete
last_updated: "2026-03-14"
last_activity: 2026-03-14 — lockdown_pod + lockdown_all_pods routes added, 10 new tests passing
progress:
  total_phases: 6
  completed_phases: 4
  total_plans: 9
  completed_plans: 9
  percent: 70
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-03-13)

**Core value:** Every URL in the venue always works — staff kiosk, customer PIN grid, pod lock screens are permanently accessible with zero manual intervention.
**Current focus:** Phases 6-9 COMPLETE — Phase 10 (Staff Dashboard Controls) or Phase 11 (Customer Experience Polish) next

## Current Position

Phase: 10 of 11 (Staff Dashboard Controls) — IN PROGRESS
Plan: 1 of N — DONE
Status: Lockdown API routes complete (lockdown_pod + lockdown_all_pods). Frontend wiring + UI controls remain.
Last activity: 2026-03-14 — 10-01 backend lockdown routes + 10 new unit tests committed (564b8ee)

Progress: [███████░░░] 70%

## Performance Metrics

**Velocity:**
- Total plans completed: 9 (v2.0)
- Average duration: ~15min
- Total execution time: ~145min

**By Phase:**

| Phase | Plans | Total | Avg/Plan |
|-------|-------|-------|----------|
| 06-diagnosis P01 | 1 | 2 tasks, 1 file | 15min |
| 06-diagnosis P02 | 1 | 2 tasks, 1 file | 10min |
| 07-server-pinning P01 | 1 | 2 tasks, 0 files | 15min |
| 07-server-pinning P02 | 1 | 2 tasks, 1 file | 30min |
| 08-pod-lock-screen-hardening P01 | 1 | 2 tasks, 3 files | 25min |
| 08-pod-lock-screen-hardening P02 | 1 | 1 tasks, 1 file | 5min |
| 08-pod-lock-screen-hardening P03 | 1 | 1 tasks, 1 file | 15min |
| 09-edge-browser-hardening P01 | 1 | 2 tasks, 1 file | 5min |
| 10-staff-dashboard-controls P01 | 1 | 1 task, 3 files | 25min |
| Phase 10-staff-dashboard-controls P02 | 15 | 2 tasks | 2 files |
| Phase 11-customer-experience-polish P01 | 10 | 2 tasks | 2 files |
| Phase 11-customer-experience-polish P02 | 1 | 1 tasks | 1 files |

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
- [Phase 09-edge-browser-hardening]: reg query with quoted paths fails via pod-agent JSON exec; unquoted paths work. Use bat files for complex registry operations.
- [Phase 09-edge-browser-hardening]: MicrosoftEdgeElevationService exists on all 8 pods (error 1060 in Phase 6 was for MicrosoftEdgeUpdate). All 3 services disabled.
- [Phase 10-01]: Lockdown toggle is ephemeral (no DB write) — resets to rc-agent config default on restart; correct behavior since default is locked
- [Phase 10-01]: parse_mac changed to pub(crate) for unit testability; BillingTimer::dummy() added as cfg(test) helper
- [Phase 10-01]: /pods/lockdown-all registered with static bulk routes before {id} dynamic routes to prevent Axum routing conflict
- [Phase 10-02]: Optimistic UI for lockdown toggle — icon reflects last sent action without waiting for server roundtrip
- [Phase 10-02]: Unlock All is non-destructive — no confirmation dialog; Lock All requires confirmation
- [Phase 10-02]: Optimistic UI for lockdown toggle — icon reflects last sent action without waiting for server roundtrip
- [Phase 10-02]: Unlock All is non-destructive — no confirmation dialog; Lock All requires confirmation
- [Phase 11-01]: SVG raw string uses r##...## delimiter because fill='#E10600' contains # which would terminate r#...#
- [Phase 11-01]: session_race_position stays None — TelemetryFrame doesn't carry race position yet; placeholder for when AC shared memory position is plumbed through
- [Phase 11-01]: ScreenBlanked never gets wallpaper — render_blank_page() uses page_shell() directly (passes None)
- [Phase 11-02]: No API/type/backend changes needed — KioskSettings index signature + updateSettings already handle arbitrary keys including lock_screen_wallpaper_url
- [Phase 11-02]: Pod Display section positioned between Spectator Display and Experiences for logical grouping

### Pending Todos

None.

### Blockers/Concerns

- ~~Server MAC address needed~~ → RESOLVED: BC-FC-E7-2C-F2-CE
- ~~rc-core CORS may need `kiosk.rp` origin guard~~ → RESOLVED: added in ea9a728
- ~~Kiosk port confirmation~~ → RESOLVED: 3300 (not listening on server, free to use)
- ~~Server DHCP lease expires nightly~~ → RESOLVED: DHCP reservation pinned to .23
- ~~Node.js must be installed on server~~ → RESOLVED: kiosk runs through rc-core proxy, no direct node.exe network access needed
- ~~Edge auto-updates could break kiosk~~ → RESOLVED: EdgeUpdate services disabled on all 8 pods
- **OPEN: Server deployment requires physical access** — SAC blocks WinRM/pod-agent remote exec. Steps documented in 07-02-SUMMARY.md "User Setup Required". Binary staged at deploy-staging/racecontrol.exe (21MB).

## Session Continuity

Last session: 2026-03-14T03:48:46.256Z
Stopped at: Completed 11-02-PLAN.md — Wallpaper URL staff UI: Pod Display section added to kiosk settings page
Resume file: None
