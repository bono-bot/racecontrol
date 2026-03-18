---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: ready_to_plan
stopped_at: Completed 260318-wkg-01 (web dashboard watchdog)
last_updated: "2026-03-18T18:05:37.885Z"
last_activity: 2026-03-17 — v6.0 roadmap created, Phases 36–40
progress:
  total_phases: 5
  completed_phases: 0
  total_plans: 2
  completed_plans: 1
---

---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: ready_to_plan
stopped_at: "Roadmap created — 5 phases, 20 requirements mapped, ready to plan Phase 36"
last_updated: "2026-03-17T00:00:00.000Z"
last_activity: 2026-03-17 — v6.0 roadmap created (Phases 36–40)
progress:
  total_phases: 5
  completed_phases: 0
  total_plans: 12
  completed_plans: 0
  percent: 0
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-03-17)

**Core value:** Customers see their lap times, compete on leaderboards, and compare telemetry — driving repeat visits and social sharing from a publicly accessible cloud PWA.
**Current focus:** v6.0 Salt Fleet Management — Phase 36 (WSL2 Infrastructure) is next.

## Current Position

Phase: 36 of 40 (WSL2 Infrastructure)
Plan: 0 of 2 in current phase
Status: Ready to plan
Last activity: 2026-03-18 - Completed quick task 260318-xbn: Kiosk watchdog

Progress: [░░░░░░░░░░] 0%

## Performance Metrics

**Velocity:**
- Total plans completed: 0 (v6.0 milestone)
- Average duration: — (no plans yet)
- Total execution time: 0 hours

**Recent Trend:** —

*Updated after each plan completion*

## Accumulated Context

### Decisions

(v6.0 Salt Fleet Management — standing constraints)
- WSL2 mirrored networking is non-negotiable — NAT mode (172.x.x.x) makes pods unreachable from 192.168.31.x; mirrored must be enabled before any minion install
- Hyper-V firewall is a separate layer from Windows Defender — both must be opened explicitly for ports 4505/4506
- Salt minion service cannot restart itself on Windows (confirmed bug #65577) — sc failure recovery must be configured during install
- Defender exclusions for C:\Program Files\Salt Project\Salt must be applied BEFORE running the installer — asynchronous quarantine happens 5–15s after install
- remote_ops.rs deletion is last — characterization tests required first (Refactor Second standing rule)
- cp.get_file vs curl-for-binaries decision: verify on Pod 8 in Phase 37 before committing to cp.get_file in deploy.rs (Phase 38)
- Minion IDs must be explicit (id: pod1–pod8, server) — never auto-generated from OEM hostname
- Salt cannot launch GUI applications (Session 0 isolation) — WebSocket channel unchanged for game launch, billing, lock screen
- salt_exec.rs uses existing reqwest client — no new Cargo dependencies
- [Phase 36-wsl2-infrastructure]: BIOS AMD-V (SVM Mode) must be enabled before WSL2 can install on Ryzen 7 5800X — VirtualizationFirmwareEnabled=False confirmed
- [Phase 260318-wkg]: Web dashboard watchdog uses HKLM Run key + PowerShell loop (not scheduled task) for auto-start reliability

### Pending Todos

- Pod 3 still not verified running after fix-pod.bat — needs physical reboot + verification
- Version string inconsistency: USB-installed pods report v0.1.0, HTTP-deployed report v0.5.2
- TELEM-01 and MULTI-01 live verification pending (needs real pod session)

### Blockers/Concerns

- Phase 36 gate: if mirrored networking + Hyper-V firewall cannot be verified from Pod 8 (Test-NetConnection port 4505), all downstream phases are blocked — no Rust code should be written until this gate passes
- Phase 39 gate: remote_ops.rs AppState field inventory must be done before characterization test scope is known — read remote_ops.rs in full and grep AppState mutations before starting 39-01-PLAN.md
- BIOS AMD-V disabled on Ryzen 7 5800X — WSL2 cannot install. Enable SVM Mode in BIOS. .wslconfig already created and ready.

### Quick Tasks Completed

| # | Description | Date | Commit | Directory |
|---|-------------|------|--------|-----------|
| 260318-wkg | Web dashboard + racecontrol watchdog (auto-restart on crash, HKLM Run key) | 2026-03-18 | 95c0d10 | [260318-wkg](./quick/260318-wkg-research-and-fix-node-js-web-dashboard-p/) |
| 260318-xbn | Kiosk watchdog (same pattern applied to port 3300) | 2026-03-18 | — | [260318-xbn](./quick/260318-xbn-apply-watchdog-pattern-to-kiosk-node-js-/) |

## Session Continuity

Last session: 2026-03-18T18:05:33.061Z
Stopped at: Completed 260318-wkg-01 (web dashboard watchdog)
Resume file: None
Next action: `/gsd:plan-phase 36`
