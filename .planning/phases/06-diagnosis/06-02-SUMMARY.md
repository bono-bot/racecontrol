---
phase: 06-diagnosis
plan: 02
subsystem: infra
tags: [server, network, port-audit, dhcp, mac-address, pod-agent]

# Dependency graph
requires: []
provides:
  - "DIAG-02: Server port audit — ports 3300 and 8080 NOT listening (kiosk not deployed, racecontrol not running)"
  - "DIAG-04: Server MAC BC-FC-E7-2C-F2-CE, DHCP enabled, IP drifted .51 → .23 → .4"
  - "Correction: Server HAS pod-agent on 8090 — MEMORY was wrong (said no pod-agent, requires RDP)"
  - "Discovery: .23 is NOT the server — it's an unknown device (phone/tablet)"
affects:
  - phase 7 (DHCP reservation MAC, server IP pinning, kiosk deployment via pod-agent)
  - phase 8 (pod configs need updated server IP)
  - all phases (server manageable via pod-agent, no RDP needed)

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Server pod-agent accessible at .4:8090 — same /exec API as pod agents"
    - "mDNS hostname resolution: ping Racing-Point-Server resolves to current DHCP IP"
    - "ARP table + NetBIOS lookup for IP identity verification"

key-files:
  created:
    - ".planning/phases/06-diagnosis/06-02-SUMMARY.md"
  modified:
    - ".planning/phases/06-diagnosis/06-FINDINGS.md"

key-decisions:
  - "DIAG-02: Port 3300 (kiosk) and 8080 (racecontrol) both NOT listening — confirms both services need Phase 7 deployment"
  - "DIAG-02: java.exe on port 45021 is unknown — investigate later, not blocking"
  - "DIAG-04: Server MAC = BC-FC-E7-2C-F2-CE (Marvell AQtion 10Gbit) — use for DHCP reservation in Phase 7"
  - "DIAG-04: DHCP lease expires nightly (~01:05) — IP drift is a daily risk until reservation is set"
  - "DIAG-04: IP drift history .51 → .23 → .4 — all pod configs pointing to .23 are broken"
  - "Correction: Server has pod-agent on 8090 — Plan 06-02 checkpoint (RDP) was unnecessary"
  - "Discovery: .23 is an unknown device (MAC 16-55-fe-10-e0-6e, no NetBIOS, 46ms latency) — pods are connecting to wrong host"

patterns-established:
  - "Server is remotely manageable via pod-agent /exec — treat like pods for deployment"
  - "DHCP lease on server expires nightly — Phase 7 MUST pin IP before any other server work"

requirements-completed:
  - DIAG-02
  - DIAG-04

# Metrics
duration: 10min
completed: 2026-03-13
---

# Phase 6 Plan 02: Server Diagnostics Summary

**Server port audit + IP/MAC identification reveals server IP drifted to .4, both racecontrol and kiosk not running, and server has pod-agent (no RDP needed)**

## Performance

- **Duration:** ~10 min
- **Started:** 2026-03-13
- **Completed:** 2026-03-13
- **Tasks:** 2 of 2
- **Files modified:** 1 (06-FINDINGS.md updated with DIAG-02 + DIAG-04 sections)

## Accomplishments

- Established DIAG-02: Server port audit shows ports 3300 (kiosk) and 8080 (racecontrol) NOT listening — both services need Phase 7 deployment. Pod-agent running on 8090.
- Established DIAG-04: Server MAC BC-FC-E7-2C-F2-CE, DHCP enabled with nightly lease expiry, IP drifted .51 → .23 → .4. Old IP .23 now belongs to unknown device.
- Discovered server HAS pod-agent on port 8090 — eliminates RDP dependency for all future phases
- Identified .23 is NOT the server — all 8 pods hardcoded to .23 are connecting to a random device

## Task Commits

1. **Task 1 + Task 2: DIAG-02 port audit + DIAG-04 IP/MAC** - (this commit)

## Files Created/Modified

- `.planning/phases/06-diagnosis/06-FINDINGS.md` - Added DIAG-02 (server port table, missing ports) and DIAG-04 (MAC, DHCP status, IP drift history, .23 identity)

## Decisions Made

- Server pod-agent on 8090 makes RDP unnecessary — all server management can be remote
- Phase 7 MUST set DHCP reservation first (before deploying kiosk/racecontrol) since IP drifts nightly
- Pod configs need server IP update: .23 → whatever the DHCP reservation pins to
- java.exe on 45021 is unknown but not blocking — investigate if it causes port conflicts later

## Deviations from Plan

- **Major deviation:** Plan 06-02 was designed as human-gated (RDP checkpoint). Instead, discovered server has pod-agent and collected all data remotely via curl. No RDP was needed.
- Diagnostics run on James's PC first for command verification, then on server via pod-agent.

## Issues Encountered

- None blocking. MEMORY incorrectly stated server has no pod-agent — corrected in findings.

## Next Phase Readiness

- **Phase 7:** FULLY UNBLOCKED — all 4 DIAG requirements complete. MAC for DHCP reservation: BC-FC-E7-2C-F2-CE. Ports 3300 and 8080 free. Pod-agent available for remote deploy.
- **Phase 8:** Informed by DIAG findings — pods need updated server IP in configs
- **Phase 9:** Already ready from Plan 06-01

---
*Phase: 06-diagnosis*
*Completed: 2026-03-13*
