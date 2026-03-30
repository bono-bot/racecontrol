---
gsd_state_version: 1.0
milestone: v31.0
milestone_name: Autonomous Survival System
status: planning
stopped_at: null
last_updated: "2026-03-30T18:00:00.000Z"
last_activity: 2026-03-30
progress:
  total_phases: 0
  completed_phases: 0
  total_plans: 0
  completed_plans: 0
  percent: 0
---

## Current Position

Phase: Not started (defining requirements)
Plan: —
Status: Defining requirements
Last activity: 2026-03-30 — Milestone v31.0 started

Progress: [░░░░░░░░░░] 0%

## Project Reference

**Milestone:** v31.0 Autonomous Survival System — 3-Layer MI Independence
**Core value:** No single system failure can kill the healing brain — 3 independent survival layers with Unified MMA Protocol
**Phase range:** 267+
**Roadmap:** .planning/ROADMAP-v31.md (pending)
**Requirements:** .planning/REQUIREMENTS.md (pending)

See: .planning/PROJECT.md (project context)
See: UNIFIED-PROTOCOL.md (operations protocol v3.1)
See: .planning/ROADMAP-v30.md (this milestone's roadmap)

## Performance Metrics

- Requirements defined: 30
- Phases planned: 6
- Plans written: 7
- Plans complete: 6
- Ship gate status: Not started

## Phase Index

| # | Phase | Requirements | Status |
|---|-------|-------------|--------|
| 261 | Design System Foundation | DS-01, DS-02, DS-03, DS-04, DS-05, DS-06, DS-07 | Complete |
| 262 | Deploy Pipeline Hardening | DQ-01, DQ-02 | Complete |
| 263 | Web Primitive Components | SC-01..SC-10, LP-01, LP-02 | Not started |
| 264 | Web Dashboard Pages | WD-01..WD-08 | Not started |
| 265 | Kiosk Pages | KS-01..KS-05 | Not started |
| 266 | Quality Gate & Audit | DQ-03, DQ-04 | Not started |
| Phase 263 P01 | 3min | 2 tasks | 4 files |
| Phase 263 P02 | 4min | 2 tasks | 6 files |
| Phase 264 P04 | 3min | 2 tasks | 7 files |
| Phase 264 P02 | 3m | 2 tasks | 2 files |
| Phase 265 P02 | 8m | 2 tasks | 2 files |

## Accumulated Context

### Key Architectural Decisions (from research)

- **Token divergence must be fixed first** (Phase 261): web has `rp-red-light`, kiosk has `rp-red-hover` for the same value. All component work blocks on `packages/shared-tokens/tokens.css` existing first.
- **No shared component package** (from ARCHITECTURE.md): Web is mouse-driven/scrollable; kiosk is touch-driven/fixed-screen. Shared tokens + shared types only. Components are app-local.
- **motion@12, NOT framer-motion**: Same codebase, different package name. Import from `motion/react`.
- **tw-animate-css, NOT tailwindcss-animate**: `tailwindcss-animate` uses the v3 plugin API — removed in Tailwind v4.
- **TanStack Table in web only**: Kiosk leaderboard uses `AnimatePresence` + `layout` for animated list, not a sortable data grid. Do NOT add `@tanstack/react-table` to kiosk.
- **Deploy pipeline MUST be hardened before pages ship** (Phase 262): Static file 404 has already burned this codebase once (2026-03-25). The copy step and smoke test must be in place before any redesigned page reaches the server.
- **LeaderboardTable is highest risk**: WS reconnect logic (cleanup on unmount), SSR boundary (`window is not defined` if WS opened server-side), auth boundary (leaderboard-display must NOT be behind AuthGate). Extract WS logic into a `useRef`+`useEffect` hook with cleanup.
- **Kiosk touch verification on real hardware** (Phase 265, 266): Browser devtools touch simulation does not accurately reproduce hover state on a physical touchscreen. Must test on an actual pod before marking kiosk phases shipped.
- **React Compiler must NOT be enabled**: TanStack Table breaks with React 19 auto-memoization (issue #5567). Leave `experimental.reactCompiler` disabled in `web/next.config.ts`.

### Key Risks From Research

- NEXT_PUBLIC_ env vars baked at build time — audit before every build, verify from a non-server machine
- Standalone deploy requires `cp -r .next/static .next/standalone/.next/static` — NOT automatic
- `outputFileTracingRoot: path.join(__dirname)` must be in both `next.config.ts` files — never remove it
- Kiosk `basePath: "/kiosk"` — all hrefs in kiosk components must be root-relative (no `/kiosk/...` hardcoding)
- Recharts components need `dynamic` import with `ssr: false` — any new chart must follow this pattern
- Deprecated orange `#FF4400` must not appear in any new component

### Deferred (Out of Scope for v30.0)

- Command palette (Ctrl+K) for power users
- QR code telemetry sharing
- Animated route transitions (full-page)
- Light mode toggle
- Ambient race-mode background on kiosk (CSS animation when most pods active)
- Sim game logo assets (requires asset sourcing)
- Revenue sparkline in billing header

## Session Continuity

Stopped at: Completed 265-02-PLAN.md
Next action: Continue Phase 263 (Web Primitive Components) — Plan 263-02

Ship gate reminder (Unified Protocol v3.1):

1. Quality Gate: `cd comms-link && COMMS_PSK="..." bash test/run-all.sh`
2. E2E: live exec + chain + health round-trip (REALTIME mode)
3. Standing Rules: auto-push, Bono synced, watchdog, rules categorized
4. Multi-Model AI Audit: all consensus P1s fixed, P2s triaged
