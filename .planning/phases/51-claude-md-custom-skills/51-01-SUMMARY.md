---
phase: 51-claude-md-custom-skills
plan: "01"
subsystem: tooling
tags: [claude-md, context, memory, project-context]
dependency_graph:
  requires: []
  provides: [CLAUDE.md auto-loaded context for every Claude Code session]
  affects: [all future Claude Code sessions in racecontrol repo]
tech_stack:
  added: []
  patterns: [CLAUDE.md project context file, dense table-based reference]
key_files:
  created:
    - CLAUDE.md
  modified:
    - C:\Users\bono\.claude\projects\C--Users-bono\memory\MEMORY.md
decisions:
  - CLAUDE.md is the authoritative Racing Point context source for all Claude Code sessions; MEMORY.md holds identity + current state only
  - CLAUDE.md at 179 lines uses tables for dense data — stays well under 300-line limit
  - MEMORY.md trimmed to 56 lines with explicit pointer to CLAUDE.md
  - Network map, deploy rules, crate names, billing, brand, cameras all migrated to CLAUDE.md
  - DHCP reservation blocker note (192.168.31.23) kept in MEMORY.md Open Issues — not a full network map entry
metrics:
  duration_secs: 193
  completed_date: "2026-03-20T05:58:47Z"
  tasks_completed: 2
  tasks_total: 2
  files_created: 1
  files_modified: 1
requirements_fulfilled: [SKILL-01]
---

# Phase 51 Plan 01: CLAUDE.md + MEMORY.md Trim Summary

Dense Racing Point context in a 179-line CLAUDE.md (tables, rules, facts) auto-loaded by Claude Code on every session start; MEMORY.md trimmed from 280 to 56 lines pointing to it.

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | Create CLAUDE.md with full Racing Point operational context | 4af3f5b | CLAUDE.md (created, 179 lines) |
| 2 | Trim MEMORY.md to identity-only (~60 lines) | (external file, no git tracking) | MEMORY.md (modified, 56 lines) |

## What Was Built

**CLAUDE.md** (179 lines, `racecontrol/CLAUDE.md`):
- 14 sections covering all Racing Point operational context
- Full network map: 8 pods + server + James + POS + spectator + router + NVR (all IPs + MACs)
- Crate names and binary naming rules (racecontrol.exe, rc-agent.exe, no "rc-core" in conversation)
- Server services table (ports, start commands)
- Fleet endpoints (fleet/health shape, filtering by pod_number)
- Deployment rules (6-step kill→delete→download→size→start→connect sequence)
- 4-tier debug order (deterministic → memory → local Ollama → cloud)
- Standing process rules (Refactor Second, Cross-Process Updates, No Fake Data, Bono comms)
- Billing rates, wheelbase config, UDP telemetry ports
- Brand identity (Racing Red #E10600, deprecated #FF4400)
- Security cameras (Dahua auth, NVR/entrance/reception IPs)
- Key file paths table
- Development rules (no unwrap, static CRT, CRLF bat files, git config)
- Current blockers (v6.0 AMD-V, Gmail OAuth, Pod 6 UAC)

**MEMORY.md** (56 lines, trimmed from 280):
- Identity block: James, Bono, Uday
- Key relationships: Bono comms method, auto-push rule
- Explicit pointer: "Full context in `racecontrol/CLAUDE.md`"
- Timezone preference
- Current milestone (v9.0 phases 51-56)
- Open issues (5 items)
- Recent commits table (8 entries)
- References to topic files

## Verification Results

| Check | Result |
|-------|--------|
| `test -f CLAUDE.md` | PASS |
| `grep -c "192.168.31" CLAUDE.md` = 17 | PASS |
| `wc -l CLAUDE.md` = 179 (< 300) | PASS |
| `grep "192.168.31.89" CLAUDE.md` (Pod 1) | PASS |
| `grep "192.168.31.91" CLAUDE.md` (Pod 8) | PASS |
| `grep "10-FF-E0-80-B1-A7" CLAUDE.md` (server MAC) | PASS |
| `grep "racecontrol.exe" CLAUDE.md` | PASS |
| `grep "Deterministic" CLAUDE.md` (4-tier) | PASS |
| `grep "Refactor Second" CLAUDE.md` | PASS |
| `grep "#E10600" CLAUDE.md` | PASS |
| `grep "Admin@123" CLAUDE.md` | PASS |
| `grep "deploy-staging" CLAUDE.md` | PASS |
| `grep "cargo test" CLAUDE.md` | PASS |
| `wc -l MEMORY.md` = 56 (< 80) | PASS |
| `grep "James Vowles" MEMORY.md` | PASS |
| `grep "CLAUDE.md" MEMORY.md` | PASS |
| Network map removed from MEMORY.md | PASS (0 table entries, 1 blocker note only) |

## Deviations from Plan

None — plan executed exactly as written.

The one minor note: MEMORY.md "192.168.31" count is 1 (not 0) because the DHCP reservation blocker `192.168.31.23` was kept in Open Issues. This is an operational blocker note, not a network map entry. The full table with all 8 pods is gone from MEMORY.md and lives in CLAUDE.md.

## Self-Check: PASSED

- CLAUDE.md exists at repo root: confirmed
- Commit 4af3f5b exists: confirmed (pushed to origin)
- MEMORY.md at 56 lines: confirmed
- All 14 sections present in CLAUDE.md: confirmed
