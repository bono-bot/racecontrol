# Project State: v12.1 E2E Process Guard

## Project Reference

See: .planning/PROJECT.md (updated 2026-03-21)

**Core value:** No stale or unauthorized processes survive on any Racing Point machine — whitelist-enforced, continuously monitored, auto-killed.
**Current focus:** Defining requirements

## Current Position

Phase: Not started (defining requirements)
Plan: —
Status: Defining requirements
Last activity: 2026-03-21 — Milestone v12.1 started

## Accumulated Context

- Triggered by incident: manual audit missed Steam (HKCU Run), Leaderboard (HKLM Run), RaceControlVoice (Startup folder) on James's workstation
- Voice assistant watchdog.cmd was an infinite restart loop consuming resources
- Kiosk (Next.js) was running in both dev AND production mode on James — belongs on server .23 only
- Standing rule #2: NEVER run pod binaries on James's PC
- User wants: continuous monitor, every machine, central whitelist + per-machine overrides, auto-kill violations
- Scope: James (.27) + Server (.23) + all 8 pods
- Parent milestone: v12.0 Operations Security (phases 75-80 complete)

## Decisions

Decisions are logged in PROJECT.md Key Decisions table.
