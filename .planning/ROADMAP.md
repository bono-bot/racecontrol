# Roadmap: v30.0 Racing Dashboard UI Redesign

## Overview

Transform every venue management page into a premium F1 pit wall aesthetic — tokens first to fix the existing CSS divergence between web and kiosk globals.css, deploy pipeline hardened before any redesigned page ships, web primitive components built bottom-up so pages can assemble them rather than invent them, web dashboard pages redesigned, kiosk pages redesigned with touch verification on real pod hardware, and a final MMA design audit closes the milestone.

Phase numbering continues from v27.0 (last phase was 260). Phases 261-266.

## Phases

**Phase Numbering:**
- Integer phases: Planned milestone work
- Decimal phases: Urgent insertions (created via /gsd:insert-phase)

- [x] **Phase 261: Design System Foundation** - Unified token file, shadcn/ui init in both apps, motion@12, tw-animate-css, JetBrains Mono in web — zero component work, full coverage unlocked (completed 2026-03-30)
- [x] **Phase 262: Deploy Pipeline Hardening** - Static file copy verification, env var audit, smoke test with static serving check — locks the ship path before any redesigned page lands (completed 2026-03-30)
- [ ] **Phase 263: Web Primitive Components** - All shared components (SC-01..SC-10) and login page (LP-01..LP-02) built before web pages depend on them
- [x] **Phase 264: Web Dashboard Pages** - All 8 web dashboard pages redesigned using Phase 263 primitives (completed 2026-03-30)
- [ ] **Phase 265: Kiosk Pages** - All 5 kiosk screens redesigned with touch-optimized primitives, verified on actual pod hardware
- [ ] **Phase 266: Quality Gate & Audit** - MMA design audit (3-model minimum), touch verification on actual pod hardware, cross-app consistency check

## Phase Details

### Phase 261: Design System Foundation
**Goal**: Both apps share one token source of truth, shadcn/ui is initialized with the motorsport theme, and all animation/font dependencies are installed — no component work, zero runtime risk, full token coverage unlocked for all downstream phases
**Depends on**: Nothing (first phase of v30.0)
**Requirements**: DS-01, DS-02, DS-03, DS-04, DS-05, DS-06, DS-07
**Success Criteria** (what must be TRUE):
  1. `packages/shared-tokens/tokens.css` exists with all 10 color tokens; both `web/src/app/globals.css` and `kiosk/src/app/globals.css` import it — `grep -rn "rp-red-light" web/src/ kiosk/src/` returns zero hits (old name fully removed, canonical name is `rp-red-hover`)
  2. `npx shadcn@latest init` has run in both `web/` and `kiosk/` with `new-york` style and Tailwind v4 mode — `web/src/components/ui/` and `kiosk/src/components/ui/` exist, `tw-animate-css` is imported in both globals.css, no `tailwindcss-animate` in either `package.json`
  3. `motion` (not `framer-motion`) is in both `web/package.json` and `kiosk/package.json`; import resolves as `import { motion } from "motion/react"`
  4. JetBrains Mono is loaded via `next/font/google` in `web/src/app/layout.tsx` and `--font-mono` resolves to JetBrains Mono in web (replacing Geist Mono); kiosk already has JetBrains Mono and is unchanged
  5. `cd web && npm run build` and `cd kiosk && npm run build` both complete with 0 TypeScript errors; `grep -rn "tailwindcss-animate" web/ kiosk/` returns zero hits
**Plans**: 3 plans
Plans:
- [x] 261-01-PLAN.md — Shared token file (packages/shared-tokens/tokens.css) + both globals.css updated
- [x] 261-02-PLAN.md — shadcn/ui init in both apps + tw-animate-css + Lucide icons in Sidebar
- [x] 261-03-PLAN.md — motion@12 install + JetBrains Mono in web layout + build verification
**UI hint**: yes

### Phase 262: Deploy Pipeline Hardening
**Goal**: The deploy pipeline for both web and kiosk apps guarantees static files are copied into standalone, env vars are audited with LAN IPs before every build, and every deploy is smoke-tested — no redesigned page can silently ship as unstyled HTML or with localhost WebSocket URLs
**Depends on**: Phase 261
**Requirements**: DQ-01, DQ-02
**Success Criteria** (what must be TRUE):
  1. Deploy script for web app includes `cp -r .next/static .next/standalone/.next/static` and `cp -r public .next/standalone/public` — after a fresh deploy `curl -I http://192.168.31.23:3200/_next/static/css/app.css` returns HTTP 200 (not 404)
  2. Deploy script for kiosk app includes equivalent static copy steps — after a fresh kiosk deploy `curl -I http://192.168.31.23:3300/kiosk/_next/static/css/app.css` returns HTTP 200
  3. Pre-build env var audit step `grep -rn NEXT_PUBLIC_ web/src/ kiosk/src/` is documented and run before every build — every `NEXT_PUBLIC_` var has a LAN IP (192.168.31.23, not localhost) in `.env.production.local`
  4. `grep outputFileTracingRoot web/next.config.ts kiosk/next.config.ts` returns hits in both files — `outputFileTracingRoot: path.join(__dirname)` present and correct in both
  5. Post-deploy smoke test verifies `/leaderboard-display` is unauthenticated: `curl -s -o /dev/null -w "%{http_code}" http://192.168.31.23:3200/leaderboard-display` returns 200, not 302
**Plans**: 2 plans
Plans:
- [x] 262-01-PLAN.md -- Env var audit script (check-frontend-env.sh) + static smoke test injected into deploy-nextjs.sh
- [x] 262-02-PLAN.md -- Post-deploy verification gate script (verify-frontend-deploy.sh) covering all 5 ROADMAP success criteria

### Phase 263: Web Primitive Components
**Goal**: All shared components for the web dashboard exist in redesigned form — StatusBadge, MetricCard, PodCard, AppShell, PinPad, CountdownTimer, LiveDataTable (TanStack Table), LeaderboardTable (with WS and AnimatePresence), Toast, and loading skeletons — plus the login page; Phase 264 page work assembles these primitives and invents nothing
**Depends on**: Phase 261 (design system), Phase 262 (deploy pipeline must be in place before shipping)
**Requirements**: SC-01, SC-02, SC-03, SC-04, SC-05, SC-06, SC-07, SC-08, SC-09, SC-10, LP-01, LP-02
**Success Criteria** (what must be TRUE):
  1. Login page renders with the 6-digit PinPad component (SC-05), Racing Red accents, motorsport aesthetic — auth error states (LP-02) show lockout countdown with clear messaging; a staff member can complete the full login flow end-to-end
  2. AppShell (SC-04) renders with Lucide icons replacing all emoji sidebar icons (DS-05), active nav item has left-border `border-l-4` accent, fleet health heatmap strip shows real pod status dots at sidebar footer, and WS connection indicator is persistent in sidebar (not per-page)
  3. PodCard (SC-03) renders in F1 timing tower row style with status bar, countdown, and driver name — CountdownTimer (SC-06) shows radial SVG progress ring that changes color at threshold (< 5 min red pulse)
  4. LeaderboardTable (SC-08) renders with F1-style rank, driver, lap time, gap, and PB row highlighting; WS reconnect logic lives in a `useRef`+`useEffect` hook with cleanup (`return () => ws.close()`) and minimum 1s reconnect delay; `AnimatePresence` with `layout` prop handles row reordering
  5. LiveDataTable (SC-07) is built on `@tanstack/react-table` with sticky header, sort, and row select — `@tanstack/react-table` is in `web/package.json` only, NOT in `kiosk/package.json`; React Compiler is NOT enabled in `web/next.config.ts`
  6. Toast system (SC-09) fires feedback on user actions; loading skeletons (SC-10) and empty states replace all plain-text "Loading..." and "No data" strings across all data-fetching components
**Plans**: 4 plans
Plans:
- [x] 263-01-PLAN.md — StatusBadge racing flag redesign + MetricCard (new) + Toast system + Skeleton/EmptyState
- [x] 263-02-PLAN.md — PodCard F1 timing row + CountdownTimer radial SVG + PinPad (extracted) + AppShell/Sidebar redesign
- [x] 263-03-PLAN.md — LiveDataTable (TanStack Table, web-only) + LeaderboardTable (WS reconnect + AnimatePresence)
- [ ] 263-04-PLAN.md — Login page redesign: PinPad integration + motorsport aesthetic + auth error states
**UI hint**: yes

### Phase 264: Web Dashboard Pages
**Goal**: All 8 web dashboard pages are redesigned using Phase 263 primitives — staff see KPI tiles on home, F1 timing tower on Pods, full data tables on Sessions and Billing, Fleet Health heatmap, F1-style Leaderboards with achievement overlay, Settings, and every remaining page using the new AppShell and design tokens
**Depends on**: Phase 263 (all web primitives must exist)
**Requirements**: WD-01, WD-02, WD-03, WD-04, WD-05, WD-06, WD-07, WD-08
**Success Criteria** (what must be TRUE):
  1. Dashboard home (WD-01) shows KPI tiles for active sessions, pods online, revenue today, and queue length with live data from WS — MetricCard (SC-02) renders title, value, and delta correctly; data updates without page refresh
  2. Pods page (WD-02) renders F1 timing tower vertical strip — each pod is one row with status bar, countdown, and driver; clicking a row opens a detail drawer; pod status updates within one WS tick
  3. Leaderboards page (WD-06) shows F1-style lap time table with PB and session-best highlights; `RecordBrokenEvent` from WS fires the achievement overlay; `curl -s -o /dev/null -w "%{http_code}" http://192.168.31.23:3200/leaderboard-display` returns 200 (auth boundary preserved — AuthGate does NOT wrap the leaderboard-display route)
  4. Sessions (WD-03), Billing (WD-04), Fleet Health (WD-05), Settings (WD-07), and all remaining pages (WD-08) render using the new AppShell with Lucide icons — `grep -rn "FF4400\|#ff4400\|rp-red-light" web/src/` returns zero hits
  5. All web pages verified from James's browser at `.23:3200` (not from the server itself) — WebSocket data flows, `NEXT_PUBLIC_WS_URL` does not resolve to localhost in the bundle; all Recharts chart components use `dynamic` import with `ssr: false`
**Plans**: 4 plans
Plans:
- [x] 264-01-PLAN.md � Dashboard home (MetricCard KPI row + F1 timing tower) + Pods page (F1 tower + detail drawer)
- [x] 264-02-PLAN.md � Sessions page (LiveDataTable) + Billing page (Toast + skeleton)
- [x] 264-03-PLAN.md � Fleet Health page (pod grid + 30s polling) + Leaderboards page (achievement overlay + WS reconnect)
- [x] 264-04-PLAN.md � Settings page (theme preview) + all remaining pages (Skeleton + EmptyState + colour purge)
**UI hint**: yes

### Phase 265: Kiosk Pages
**Goal**: All 5 kiosk screens are redesigned with touch-optimized primitives — pod selection grid with 44px+ touch targets, full game launch flow, billing/payment view with countdown ring, pin-protected staff tools, and animated leaderboard — verified on an actual pod touchscreen before the phase closes
**Depends on**: Phase 261 (kiosk design system), Phase 262 (deploy pipeline)
**Requirements**: KS-01, KS-02, KS-03, KS-04, KS-05
**Success Criteria** (what must be TRUE):
  1. Kiosk home pod grid (KS-01) renders with all interactive elements at minimum 44x44px (`min-h-11 min-w-11`) — `active:scale-[0.97]` press feedback fires on tap; offline pod count appears in KioskHeader alongside available and racing counts
  2. Game launch flow (KS-02) with sim selection, difficulty, and AI config completes end-to-end without hover interactions — all content is visible without hovering, all interactive elements use `onClick`; kiosk basePath links use root-relative hrefs (no hardcoded `/kiosk/...` paths in component code)
  3. Billing/payment view (KS-03) shows wallet balance, session timer, and CountdownTimer radial progress ring — ring pulses red when `remaining_seconds < 300`; remaining session time (not elapsed) is the primary display
  4. Staff tools page (KS-04) requires 6-digit PinPad entry using the same SC-05 component — pin-protected access verified working on touch; `overflow: hidden` and `user-select: none` are preserved on kiosk body (no scroll introduced)
  5. Kiosk leaderboard (KS-05) animates rank changes using `AnimatePresence` + `layout` prop — tested on an actual pod touchscreen (not browser devtools touch simulation); kiosk build does NOT contain `@tanstack/react-table`
**Plans**: 3 plans
Plans:
- [ ] 265-01-PLAN.md -- Kiosk home pod grid: offline count, press feedback, remaining time on active cards (KS-01)
- [ ] 265-02-PLAN.md -- Booking wizard touch-optimize + billing countdown ring + 6-digit PinPad in fleet page (KS-02, KS-03, KS-04)
- [ ] 265-03-PLAN.md -- KioskLeaderboard with AnimatePresence + spectator page wire-up + pod hardware verification (KS-05)
**UI hint**: yes

### Phase 266: Quality Gate & Audit
**Goal**: A minimum 3-model MMA design audit runs after all pages ship, all P1 consensus findings are fixed and re-audited, P2 findings are explicitly triaged, and physical touch verification on actual pod hardware confirms kiosk UX — the milestone ships only when audit and hardware verification both pass
**Depends on**: Phase 264 (web pages complete), Phase 265 (kiosk pages complete)
**Requirements**: DQ-03, DQ-04
**Success Criteria** (what must be TRUE):
  1. MMA design audit runs with at least 3 models; all P1 consensus findings are fixed and confirmed resolved in a re-audit pass; P2 findings have explicit documented disposition (fix now or defer with reason)
  2. Physical touch verification on an actual pod touchscreen confirms all kiosk interactive elements respond to tap — no hover-only interactions remain; `grep -rn "onMouseEnter\|onMouseLeave\|hover:" kiosk/src/` returns zero hits in primary interaction paths
  3. `grep -rn "rp-red-light\|FF4400\|#ff4400\|tailwindcss-animate\|framer-motion" web/src/ kiosk/src/` returns zero hits — deprecated tokens and wrong package names fully purged
  4. Cross-app consistency check passes: same status string values produce same semantic colors in both apps, `MM:SS` timer format is consistent, both `npm run build` complete clean, and static serving verified with HTTP 200 from a non-server machine
**Plans**: 2 plans
Plans:
- [ ] 266-01-PLAN.md — MMA design audit (3-model council, P1 fix, P2 triage) + deprecated token grep sweep
- [ ] 266-02-PLAN.md — Touch verification on Pod 1 + Pod 2 + cross-app consistency check + build verification

## Progress Table

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 261. Design System Foundation | 3/3 | Complete   | 2026-03-30 |
| 262. Deploy Pipeline Hardening | 0/2 | Not started | - |
| 263. Web Primitive Components | 3/4 | In Progress|  |
| 264. Web Dashboard Pages | 4/4 | Complete   | 2026-03-30 |
| 265. Kiosk Pages | 0/3 | Not started | - |
| 266. Quality Gate & Audit | 0/2 | Not started | - |
