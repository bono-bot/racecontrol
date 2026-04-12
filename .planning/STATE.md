---
gsd_state_version: 1.0
milestone: v48.0
milestone_name: "Codebase Architecture — Department-Driven Event Mesh"
status: Defining requirements
last_updated: "2026-04-13"
progress:
  total_phases: 0
  completed_phases: 0
  total_plans: 0
  completed_plans: 0
  percent: 0
---

# Project State — v48.0 Codebase Architecture — Department-Driven Event Mesh

## Project Reference

See: .planning/PROJECT.md (updated 2026-04-13)

**Core value:** Provide the absolute best customer experience by making the system simple enough to always work.
**Current focus:** Defining requirements for 9 department contracts + fix tooling

## Current Position

Phase: Not started (defining requirements)
Plan: —
Status: Defining requirements
Last activity: 2026-04-13 — Milestone v48.0 started

## Accumulated Context

### From v46.0 + v47.0 (shipped 2026-04-12)

- All code merged to main, deployed to venue + cloud (build `8e8c07ba`)
- Cloud auto-rebuild fixed (PATH bug resolved — was silently failing 3,443 times)
- 18 admin audit findings closed, game launch diagnostics shipped

### Codebase Analysis (2026-04-12 session)

- **419K lines** across 12 repos, 83% in racecontrol
- **335K lines** (80%) touched by debug work (1,397 commits)
- **36K lines** of net bloat from fix commits (3.7:1 insertion:deletion ratio)
- **141 files** over 500 lines, 9 files over 3,000 lines
- **routes.rs** at 26,459 lines (biggest single file)
- **AC launch path** spans 19,597 lines across 12 files (VMS does it in ~200)
- Two launch methods (Staff + PWA PIN) tangled in same code

### Business Operations (from Uday, 2026-04-13)

- RacingPoint = motorsport community + arcade gaming + cafe
- Wallet: Rupees → Credits, promotional credits non-refundable
- Game Launch Method 1: Staff kiosk launch
- Game Launch Method 2: PWA → 4-digit PIN → pod (modern arcade model)
- Cafe + racing combo deals needed (cafe sales below prediction)
- Marketing fills empty hours (weekday afternoons = zero customers)
- Staff role: host, recommend, connect about motorsport
- Future: pods in multiple locations, PIN-based remote launch

## Next Action

Define REQUIREMENTS.md from 9 departments, then spawn roadmapper.
