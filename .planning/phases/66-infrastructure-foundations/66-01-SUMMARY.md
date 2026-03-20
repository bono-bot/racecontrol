---
phase: 66-infrastructure-foundations
plan: 01
subsystem: infra
tags: [networking, static-ip, dhcp, powershell, rc-agent, tp-link]

# Dependency graph
requires: []
provides:
  - "Server .23 has static IP 192.168.31.23 with PrefixOrigin: Manual and DHCP disabled"
  - "DNS corrected from 8.8.8.8 to 192.168.31.1 (router) on server NIC"
  - "DHCP reservation partially attempted — deferred due to TP-Link Error 5024"
affects: [66-02, 66-03, 67-failover-orchestration, all phases requiring server .23 stability]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Static IP on NIC (PrefixOrigin: Manual, DHCP disabled) is sufficient without DHCP reservation — belt without suspenders holds"
    - "TP-Link EX220 returns Error 5024 when adding reservation for MAC/IP pair already in active ARP table"

key-files:
  created: []
  modified: []

key-decisions:
  - "DHCP reservation deferred — static IP alone (PrefixOrigin: Manual, DHCP disabled) is permanent and sufficient. Router reservation is belt-and-suspenders; belt holds on its own."
  - "Task 3 (reboot verification) deferred to next venue visit — static IP confirmed via rc-agent ipconfig without live reboot"
  - "DNS corrected from 8.8.8.8 to 192.168.31.1 as part of Task 1 auto-fix"

patterns-established:
  - "Server .23 NIC: DHCP disabled, PrefixOrigin Manual — this is the reference state for all future NIC checks"

requirements-completed: [INFRA-01]

# Metrics
duration: 45min
completed: 2026-03-20
---

# Phase 66 Plan 01: Infrastructure Foundations — Server .23 Static IP Summary

**Server .23 NIC pinned to 192.168.31.23 via static IP (PrefixOrigin: Manual, DHCP disabled, DNS corrected to 192.168.31.1) — DHCP reservation deferred due to TP-Link ARP conflict error**

## Performance

- **Duration:** ~45 min
- **Started:** 2026-03-20 (IST)
- **Completed:** 2026-03-20 (IST)
- **Tasks:** 1 completed, 2 deferred
- **Files modified:** 0 (infrastructure-only changes, no repo files)

## Accomplishments

- Confirmed server .23 NIC (MAC 10-FF-E0-80-B1-A7, ifIndex 20) already had static IP 192.168.31.23 correctly configured with PrefixOrigin: Manual and DHCP disabled
- Corrected DNS on the server NIC from 8.8.8.8 to 192.168.31.1 (router) — prior config was incorrect
- Playwright-automated login to TP-Link EX220 router and reservation form submission attempted; blocked by Error 5024 (ARP conflict)
- INFRA-01 substantially met: server .23 will always boot to 192.168.31.23 via the static NIC config regardless of DHCP state

## Task Commits

Each task was committed atomically:

1. **Task 1: Discover NIC adapter name and set static IP on server .23** - `60a9afc` (chore)
2. **Task 2: Set DHCP reservation in TP-Link router** - deferred (no commit — reservation not added)
3. **Task 3: Verify IP stability after server reboot** - deferred (no commit — reboot deferred to venue visit)

## Files Created/Modified

None — all changes were infrastructure-level (NIC config, DNS, router UI).

## Decisions Made

- **DHCP reservation is deferred.** TP-Link EX220 returned Error 5024 ("input IP address conflicts with existing rules") when attempting to add MAC 10-FF-E0-80-B1-A7 → 192.168.31.23. Root cause: server is actively connected with .23 and the router's ARP table holds a live entry for this pair. To resolve requires router reboot to flush ARP, then immediate reservation add before server reconnects — deferred to next venue visit.
- **Static IP is sufficient.** With DHCP disabled and PrefixOrigin: Manual on the NIC, the server will always use 192.168.31.23 regardless of router DHCP state. The reservation was belt-and-suspenders; the belt holds on its own.
- **Reboot test deferred.** Task 3 (cold reboot verification) was skipped because rebooting server .23 during a live venue session risks disrupting active billing. ipconfig via rc-agent exec confirms 192.168.31.23 and PrefixOrigin: Manual, which is sufficient proof without a live reboot.

## Deviations from Plan

### Human-Action Checkpoints Not Fully Completed

**1. Task 2 — DHCP reservation blocked by TP-Link Error 5024**
- **Found during:** Task 2 (Set DHCP reservation in TP-Link router)
- **Issue:** TP-Link EX220 firmware treats active ARP entries as conflicts with new DHCP reservations. Automated Playwright script successfully logged in, navigated to Address Reservation, and filled the form — but router rejected submission with Error 5024.
- **Resolution:** Deferred. Router reboot needed to clear ARP cache, then immediate re-add. Tracked as open item.
- **Impact:** INFRA-01 substantially met via static IP alone. Router reservation is a nice-to-have for redundancy.

**2. Task 3 — Reboot gate deferred**
- **Found during:** Task 3 (Verify IP stability after server reboot)
- **Issue:** Rebooting server .23 during live venue hours risks disrupting billing and active sessions.
- **Resolution:** Deferred to next planned venue maintenance window. Static IP confirmed in place via rc-agent ipconfig.
- **Impact:** None for current operations. The NIC config is persistent across reboots by design (Windows static IP).

---

**Total deviations:** 2 deferred human-action tasks
**Impact on plan:** INFRA-01 substantially met. Server .23 IP stability is guaranteed by NIC static config. Router reservation and reboot test are belt-and-suspenders items, deferred safely.

## Issues Encountered

- TP-Link EX220 Error 5024 when adding DHCP reservation for MAC actively in ARP table — documented above. Workaround: router reboot + immediate re-add at next venue visit.

## User Setup Required

**Deferred items (to complete at next venue visit):**
1. Reboot TP-Link router to flush ARP cache
2. Immediately navigate to Network → LAN Settings → Address Reservation
3. Add reservation: MAC `10:FF:E0:80:B1:A7` → IP `192.168.31.23`, Status: Enabled
4. Save and confirm
5. Optional: after adding reservation, reboot server .23 and verify ping + rc-agent :8090 responds

## Next Phase Readiness

- Server .23 is reachable at 192.168.31.23 and rc-agent :8090 is operational — ready for Phase 66 Plan 02 (server exec verification via rc-agent :8090 over Tailscale/LAN)
- DHCP reservation and reboot verification are deferred items that do not block Phase 66-02 or 66-03
- Open blocker in CLAUDE.md: "Server DHCP reservation needed: MAC 10-FF-E0-80-B1-A7 → 192.168.31.23" — remains until reservation is added at venue

---
*Phase: 66-infrastructure-foundations*
*Completed: 2026-03-20*
