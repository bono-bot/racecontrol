# Requirements: v30.0 Racing Dashboard UI Redesign

**Defined:** 2026-03-30
**Core Value:** Every venue management page looks and feels like a premium F1 pit wall — fast, precise, motorsport-inspired

## Design System (DS)
- [x] **DS-01**: Shared design tokens CSS file with full color ramp (surfaces, semantics, status colors)
- [x] **DS-02**: JetBrains Mono integrated via next/font for numeric displays in both apps
- [x] **DS-03**: shadcn/ui initialized in web app with motorsport theme variables
- [x] **DS-04**: shadcn/ui initialized in kiosk app with touch-optimized theme
- [x] **DS-05**: Lucide React icons replace all emoji sidebar icons
- [x] **DS-06**: motion@12 integrated for micro-interactions (pod transitions, leaderboard reorders)
- [x] **DS-07**: tw-animate-css integrated for shadcn component animations

## Shared Components (SC)
- [x] **SC-01**: StatusBadge redesign with racing flag color system (ready/occupied/warning/offline/maintenance)
- [x] **SC-02**: MetricCard component (title, value, delta, sparkline, alert state)
- [ ] **SC-03**: PodCard redesign with F1 timing tower row style (status bar, countdown, driver)
- [ ] **SC-04**: AppShell with motorsport sidebar, top bar, and racing-control aesthetic
- [ ] **SC-05**: PinPad component (6-digit, reusable for login + kiosk + supervisor auth)
- [ ] **SC-06**: CountdownTimer redesign with radial progress ring and threshold colors
- [ ] **SC-07**: LiveDataTable built on TanStack Table with sticky header, sort, select (web only)
- [ ] **SC-08**: LeaderboardTable with F1-style rank, driver, lap time, gap, PB highlighting
- [x] **SC-09**: Toast/notification system for feedback on actions
- [x] **SC-10**: Loading skeletons and empty states for all data views

## Login Page (LP)
- [ ] **LP-01**: Login page redesign with 6-digit PinPad, motorsport aesthetic, Racing Red accents
- [ ] **LP-02**: Auth error states with clear feedback and lockout indication

## Web Dashboard (WD)
- [ ] **WD-01**: Dashboard home with KPI tiles (active sessions, pods online, revenue, queue)
- [ ] **WD-02**: Pods page with F1 timing tower vertical strip + detail drawer
- [ ] **WD-03**: Sessions page with active/completed session list and session detail cards
- [ ] **WD-04**: Billing page with wallet management, top-up, session history
- [ ] **WD-05**: Fleet Health page with pod grid, status indicators, health timeline
- [ ] **WD-06**: Leaderboards page with F1-style lap time table, PB/session-best highlights
- [ ] **WD-07**: Settings page with venue configuration, theme preview
- [ ] **WD-08**: All remaining pages updated to use new AppShell + design tokens

## Kiosk (KS)
- [ ] **KS-01**: Kiosk home with touch-optimized pod selection grid (44px+ touch targets)
- [ ] **KS-02**: Game launch flow with sim selection, difficulty, AI config
- [ ] **KS-03**: Billing/payment view with wallet balance, session timer, countdown ring
- [ ] **KS-04**: Staff tools page with pin-protected access
- [ ] **KS-05**: Kiosk leaderboard with animated rank changes

## Deploy & Quality (DQ)
- [x] **DQ-01**: Deploy pipeline with static file copy verification and _next/static/ smoke test
- [x] **DQ-02**: NEXT_PUBLIC_ env var audit across all apps before each deploy
- [ ] **DQ-03**: MMA design audit after each phase batch (minimum 3-model consensus)
- [ ] **DQ-04**: Touch verification on actual pod hardware for kiosk changes

## Traceability

| REQ-ID | Phase | Plan | Status |
|--------|-------|------|--------|
| DS-01 | Phase 261 | 261-01 | Complete |
| DS-02 | Phase 261 | TBD | Pending |
| DS-03 | Phase 261 | 261-02 | Complete |
| DS-04 | Phase 261 | 261-02 | Complete |
| DS-05 | Phase 261 | 261-02 | Complete |
| DS-06 | Phase 261 | TBD | Pending |
| DS-07 | Phase 261 | 261-02 | Complete |
| DQ-01 | Phase 262 | 262-01, 262-02 | Complete |
| DQ-02 | Phase 262 | 262-01 | Complete |
| SC-01 | Phase 263 | TBD | Pending |
| SC-02 | Phase 263 | TBD | Pending |
| SC-03 | Phase 263 | TBD | Pending |
| SC-04 | Phase 263 | TBD | Pending |
| SC-05 | Phase 263 | TBD | Pending |
| SC-06 | Phase 263 | TBD | Pending |
| SC-07 | Phase 263 | TBD | Pending |
| SC-08 | Phase 263 | TBD | Pending |
| SC-09 | Phase 263 | TBD | Pending |
| SC-10 | Phase 263 | TBD | Pending |
| LP-01 | Phase 263 | TBD | Pending |
| LP-02 | Phase 263 | TBD | Pending |
| WD-01 | Phase 264 | TBD | Pending |
| WD-02 | Phase 264 | TBD | Pending |
| WD-03 | Phase 264 | TBD | Pending |
| WD-04 | Phase 264 | TBD | Pending |
| WD-05 | Phase 264 | TBD | Pending |
| WD-06 | Phase 264 | TBD | Pending |
| WD-07 | Phase 264 | TBD | Pending |
| WD-08 | Phase 264 | TBD | Pending |
| KS-01 | Phase 265 | TBD | Pending |
| KS-02 | Phase 265 | TBD | Pending |
| KS-03 | Phase 265 | TBD | Pending |
| KS-04 | Phase 265 | TBD | Pending |
| KS-05 | Phase 265 | TBD | Pending |
| DQ-03 | Phase 266 | TBD | Pending |
| DQ-04 | Phase 266 | TBD | Pending |

## Future Requirements (Deferred)
- Command palette (Ctrl+K) for power users
- QR code telemetry sharing
- Animated route transitions
- Light mode toggle

## Out of Scope
- Backend API changes (all existing endpoints preserved)
- Light mode (dark only per brand guidelines)
- Pod reordering in grid (fixed pod numbering per standing rules)
- Mobile app (web-only, kiosk is touch but not phone)
