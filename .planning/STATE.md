---
gsd_state_version: 1.0
milestone: v31.0
milestone_name: milestone
status: verifying
stopped_at: Completed 267-03-PLAN.md — all 5 recovery systems sentinel-aware (SF-05)
last_updated: "2026-03-30T14:28:54.271Z"
last_activity: 2026-03-30
progress:
  total_phases: 6
  completed_phases: 1
  total_plans: 3
  completed_plans: 3
  percent: 0
---

## Current Position

Phase: 267 (Survival Foundation) — EXECUTING
Plan: 3 of 3
Status: Phase complete — ready for verification
Last activity: 2026-03-30

Progress: [░░░░░░░░░░] 0%

## Project Reference

**Milestone:** v31.0 Autonomous Survival System — 3-Layer MI Independence
**Core value:** No single system failure can kill the healing brain — 3 independent survival layers with Unified MMA Protocol
**Phase range:** 267-272
**Roadmap:** .planning/ROADMAP-v31.md
**Requirements:** .planning/REQUIREMENTS.md

See: .planning/PROJECT.md (project context)
See: UNIFIED-PROTOCOL.md (operations protocol v3.1)

## Phase Index

| # | Phase | Requirements | Status |
|---|-------|-------------|--------|
| 267 | Survival Foundation | SF-01..SF-05 | Not started |
| 268 | Unified MMA Protocol | MP-01..MP-09 | Not started |
| 269 | Layer 1 Smart Watchdog | SW-01..SW-14 | Not started |
| 270 | Layer 2 Server Fleet Healer | FH-01..FH-12 | Not started |
| 271 | Layer 3 External Guardian | EG-01..EG-10 | Not started |
| 272 | Integration & MMA Audit | (cross-layer gate) | Not started |

## Accumulated Context

### Key Architectural Decisions

- **Build order is strict:** Foundation (267) → MMA Protocol (268) → Layer 1 (269) and Layer 2 (270) and Layer 3 (271) in parallel → Integration (272). Layer 1/2/3 all depend on 267 and 268.
- **OpenRouter client trait in rc-common only** — trait definition, no reqwest dependency. Implementation lives in higher layers to avoid circular deps.
- **rc-watchdog has NO tokio runtime** — Phase 269 must create Runtime::new() for async OpenRouter calls. Never use reqwest::blocking in the main watchdog poll loop.
- **rc-guardian is a new Linux crate** — separate binary for Bono VPS, NOT an extension of rc-watchdog. Target: x86_64-unknown-linux-musl or gnu.
- **goblin crate with `features = ["pe"]` only** — do not enable default features (avoids pulling in Mach-O/ELF parsers).
- **HEAL_IN_PROGRESS sentinel** defined in rc-common (Phase 267) BEFORE any healing logic in later phases.
- **Layer 2 SSH diagnostic runner** is the prerequisite for ALL autonomous remote repair (FH-01 must ship before FH-05).

### Key Risks From Research

- Windows SYSTEM context (Session 0) HTTP: certificate validation may fail for watchdog direct reporting (SW-06) — test on real hardware
- MAINTENANCE_MODE lockout: watchdog must NOT write it during MMA cycle; only read and escalate to Layer 2
- Rollback loop: depth counter in rollback-state.json required before rollback code ships (SW-04 before SW-03)
- Split-brain guardians: GUARDIAN_ACTING via comms-link WS must be implemented before EG-03 (restart) ships
- Budget persistence: budget_state.json must be written on every OpenRouter call, not just at session end

### Blockers/Concerns

None yet. Research is complete, architecture is clear.

## Session Continuity

Stopped at: Completed 267-03-PLAN.md — all 5 recovery systems sentinel-aware (SF-05)
Next action: Run /gsd:plan-phase 267 to begin Survival Foundation planning.

Ship gate reminder (Unified Protocol v3.1):

1. Quality Gate: `cd comms-link && COMMS_PSK="..." bash test/run-all.sh`
2. E2E: live exec + chain + health round-trip (REALTIME mode)
3. Standing Rules: auto-push, Bono synced, watchdog, rules categorized
4. Multi-Model AI Audit: all consensus P1s fixed, P2s triaged
