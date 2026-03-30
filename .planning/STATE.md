---
gsd_state_version: 1.0
milestone: v30.0
milestone_name: milestone
status: verifying
stopped_at: Completed 263-01-PLAN.md (Web Primitive Components)
last_updated: "2026-03-30T10:37:50.150Z"
last_activity: 2026-03-30
progress:
  total_phases: 6
  completed_phases: 1
  total_plans: 18
  completed_plans: 4
  percent: 17
---

## Current Position

Phase: 261 of 266 (Design System Foundation) -- COMPLETE
Plan: 03 of 03 complete (phase done)
Status: Phase complete — ready for verification
Last activity: 2026-03-30

Progress: [██░░░░░░░░] 17%

## Project Reference

**Milestone:** v30.0 Racing Dashboard UI Redesign
**Core value:** Every venue management page looks and feels like a premium F1 pit wall — fast, precise, motorsport-inspired
**Phase range:** 261–266
**Roadmap:** .planning/ROADMAP-v30.md
**Requirements:** .planning/REQUIREMENTS.md (30 requirements: DS-01..07, SC-01..10, LP-01..02, WD-01..08, KS-01..05, DQ-01..04)

See: .planning/PROJECT.md (project context)
See: UNIFIED-PROTOCOL.md (operations protocol v3.1)
See: .planning/ROADMAP-v30.md (this milestone's roadmap)

## Performance Metrics

- Requirements defined: 30
- Phases planned: 6
- Plans written: 7
- Plans complete: 3
- Ship gate status: Not started

## Phase Index

| # | Phase | Requirements | Status |
|---|-------|-------------|--------|
| 261 | Design System Foundation | DS-01, DS-02, DS-03, DS-04, DS-05, DS-06, DS-07 | Complete |
| 262 | Deploy Pipeline Hardening | DQ-01, DQ-02 | Not started |
| 263 | Web Primitive Components | SC-01..SC-10, LP-01, LP-02 | Not started |
| 264 | Web Dashboard Pages | WD-01..WD-08 | Not started |
| 265 | Kiosk Pages | KS-01..KS-05 | Not started |
| 266 | Quality Gate & Audit | DQ-03, DQ-04 | Not started |
| Phase 263 P01 | 3min | 2 tasks | 4 files |

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

Stopped at: Completed 263-01-PLAN.md (Web Primitive Components)
Next action: Execute Phase 262 (Deploy Pipeline Hardening)

Ship gate reminder (Unified Protocol v3.1):

1. Quality Gate: `cd comms-link && COMMS_PSK="..." bash test/run-all.sh`
2. E2E: live exec + chain + health round-trip (REALTIME mode)
3. Standing Rules: auto-push, Bono synced, watchdog, rules categorized
4. Multi-Model AI Audit: all consensus P1s fixed, P2s triaged
