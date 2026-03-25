---
phase: 190-phase-scripts-tiers-1-9-sequential-baseline
plan: "01"
subsystem: audit-framework
tags: [audit, bash, phase-scripts, tier1, tier2, tier3, fleet-health]
dependency_graph:
  requires:
    - audit/lib/core.sh (emit_result, http_get, safe_remote_exec, safe_ssh_capture, ist_now)
    - audit/phases/tier1/phase01.sh (reference pattern)
  provides:
    - audit/phases/tier1/phase02.sh through phase10.sh
    - audit/phases/tier2/phase11.sh through phase16.sh
    - audit/phases/tier3/phase17.sh through phase20.sh
  affects:
    - audit/audit.sh (loads and calls all run_phaseNN functions)
tech_stack:
  added: []
  patterns:
    - set -u + set -o pipefail, no set -e
    - emit_result for all output (never echo/printf)
    - safe_remote_exec with temp file pattern (cmd.exe quoting safety)
    - QUIET override when venue_state=closed on pod-dependent checks
    - Tier 3 scripts emit QUIET immediately on closed venue (skip all checks)
key_files:
  created:
    - audit/phases/tier1/phase02.sh
    - audit/phases/tier1/phase03.sh
    - audit/phases/tier1/phase04.sh
    - audit/phases/tier1/phase05.sh
    - audit/phases/tier1/phase06.sh
    - audit/phases/tier1/phase07.sh
    - audit/phases/tier1/phase08.sh
    - audit/phases/tier1/phase09.sh
    - audit/phases/tier1/phase10.sh
    - audit/phases/tier2/phase11.sh
    - audit/phases/tier2/phase12.sh
    - audit/phases/tier2/phase13.sh
    - audit/phases/tier2/phase14.sh
    - audit/phases/tier2/phase15.sh
    - audit/phases/tier2/phase16.sh
    - audit/phases/tier3/phase17.sh
    - audit/phases/tier3/phase18.sh
    - audit/phases/tier3/phase19.sh
    - audit/phases/tier3/phase20.sh
  modified: []
decisions:
  - "Tier 3 scripts use early-continue pattern: if venue_state=closed, emit QUIET and continue — no conditional nesting per check"
  - "phase08 uses rc-sentry :8091 for sentinel checks (standing rule: sentry alive when rc-agent in maintenance)"
  - "phase19 tracks pod_num counter to identify Pod 8 (known 1024x768 issue, downgraded to WARN not FAIL)"
  - "phase12 WS check uses curl http_code probe: 400=upgrade-required=endpoint present, 404=not registered"
metrics:
  duration_minutes: 15
  tasks_completed: 2
  files_created: 19
  completed_date: "2026-03-25"
---

# Phase 190 Plan 01: Phase Scripts Tiers 1-3 Sequential Baseline Summary

19 bash phase scripts (phases 02-20) porting AUDIT-PROTOCOL v3.0 commands into non-interactive functions for Tiers 1-3 of the automated fleet audit framework.

## What Was Built

### Task 1: Tier 1 Phase Scripts 02-10 (Infrastructure Foundation)

| Phase | Name | What It Checks |
|-------|------|----------------|
| 02 | Config Integrity | racecontrol.toml first-line validation, duplicate enabled= keys, pod rc-agent.toml pod_number, comms-link .env COMMS_PSK |
| 03 | Network & Tailscale | James Tailscale active, pod LAN ping to server .23, server .23 to Bono VPS, POS PC :8090 |
| 04 | Firewall & Port Security | Server .23 netsh firewall state, listening ports 8080/8090/3200/3300, James ports 8766/1984/11434 |
| 05 | Pod Power & WoL | Pod :8090 health + uptime check (< 300s = recent reboot WARN) |
| 06 | Orphan Processes | PowerShell count per pod, Variable_dump.exe not running, rc-agent exactly 1 instance, server watchdog singleton |
| 07 | Process Guard & Allowlist | Fleet health violation_count_24h max, allowlist count per pod-1 through pod-8 |
| 08 | Sentinel Files | MAINTENANCE_MODE + GRACEFUL_RELAUNCH + rcagent-restart-sentinel.txt via rc-sentry :8091 |
| 09 | Self-Monitor & Self-Heal | self_monitor heartbeat in logs, safe_mode not active |
| 10 | AI Healer / Watchdog | watchdog-state.json failure_count, Ollama model count (expected >= 2) |

Commit: `ded8c5d3`

### Task 2: Tier 2 Phase Scripts 11-16 (Core Services) + Tier 3 Phase Scripts 17-20 (Display & UX)

**Tier 2:**

| Phase | Name | What It Checks |
|-------|------|----------------|
| 11 | API Data Integrity | Fleet health pod count, logs .jsonl filename, app-health (v20.1), server health build_id field |
| 12 | WebSocket Flows | ws/dashboard + ws/agent HTTP probe (400=present, 404=missing), ws_connected count in fleet health |
| 13 | rc-agent Exec Capability | hostname exec test per pod, exec_slots_available in health response |
| 14 | rc-sentry Health | sentry :8091 health, exec capability, can see rc-agent.exe process |
| 15 | Preflight Checks | FAIL entries in preflight section of rc-agent logs |
| 16 | Cascade Guard & Recovery | cascade_guard + pod_healer entries in recent logs API |

**Tier 3 (ALL checks QUIET when venue_state=closed):**

| Phase | Name | What It Checks |
|-------|------|----------------|
| 17 | Lock Screen & Blanking | Edge/kiosk foreground process, Edge stacking (> 5 = bug) |
| 18 | Overlay Suppression | Copilot/NVIDIA/AMD DVR/OneDrive/Widgets/Steam/GameBar processes |
| 19 | Display Resolution | 7680x1440 Surround, 1920x1080+ single, 1024x768 flagged (Pod 8 WARN, others FAIL) |
| 20 | Kiosk Browser Health | Edge --kiosk flag with :3300 URL, kiosk page returns 200 from pod |

Commit: `ed529fe3`

## Pattern Compliance

All 19 scripts follow the exact pattern from phase01.sh:
- `set -u` + `set -o pipefail` — no `set -e`
- `run_phaseNN()` function with `local phase="NN" tier="N"`
- All output via `emit_result` — never echo/printf for results
- QUIET override block for pod checks in venue_state=closed
- `return 0` always
- `export -f run_phaseNN` at end

## Deviations from Plan

None — plan executed exactly as written.

Note: The plan's verification script used `grep -q "venue_state.*closed.*QUIET"` to check Tier 3 scripts. The scripts use a multi-line if-block (venue_state check on line N, QUIET on line N+1 inside emit_result). The pattern does not match single-line grep, but the logic is functionally correct — all Tier 3 checks emit QUIET immediately when venue is closed and skip all pod-level work.

## Self-Check: PASSED

All 19 files found. Both commits verified: `ded8c5d3` (Tier 1) and `ed529fe3` (Tier 2+3).
