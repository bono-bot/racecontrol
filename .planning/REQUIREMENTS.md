# Requirements: RacingPoint v49.0 — Unified RaceControl Operations

**Defined:** 2026-04-14
**Core Value:** Customers can seamlessly book a sim racing session — single or multiplayer — and start racing with minimal friction, while all lap times, telemetry, and payments are tracked automatically. Every time. For every game.

**Predecessor:** v48.0 (Codebase Architecture). v49 ships v48's committed code, completes lap recording, then builds autonomous revenue systems.

## Prior Art — What's Already Done

The following capabilities exist in code (committed or deployed). v49 does NOT rebuild them — it deploys, verifies, and builds on top.

| Capability | Source | Status |
|---|---|---|
| Native Win32 lock screen (no Edge) | James v44.0 Phase 329 | Deployed |
| VMS architecture integration | James v44.0 Phases 329-336 | Code complete |
| Credits/rupees wallet separation | James v45.0 Phases 337-342 | Deployed |
| Post-launch config verification | James v46.0 Phase 362 | Deployed (build a9b5eaa3) |
| Data recording verification | James v46.0 Phase 363 | Code complete, **NOT deployed** |
| Game intelligence + fleet health | James v41.0 Phases 315-320 | Deployed |
| Reliability dashboard | James v41.0 Phase 319 | Deployed |
| Health + WhatsApp alerts | James v47.0 Phase 352 | Deployed |
| Auth resilience (lockout, break-glass) | James v47.0 Phase 348 | Deployed |
| Data durability (backups, WAL) | James v47.0 Phase 351 | Deployed |
| DB sync via Google Drive | James v47.0 Phase 349 | Deployed |
| Runbooks + staff training | James v47.0 Phase 353 | Deployed |
| Session trace ID propagation | James v39.0 Phase 310 | Partial (1/2 plans) |
| Security hardening (TLS, RBAC, JWT) | James v38.0 Phases 305-309 | Deployed |
| Self-audit + visual regression | James v43.0 Phases 325-328 | Deployed |
| Live launch status + autonomous debug | James Phase 368 | Deployed |
| routes.rs split (26K → 55 modules) | Bono v48 Phase 380 | Committed, not deployed |
| launch_contract types | Bono v48 Phase 369 | Committed, not deployed |
| billing_arcade model | Bono v48 Phase 372 | Committed, not deployed |
| PWA self-service PIN launch | Bono v48 Phase 374 | Committed, not deployed |
| Credit types (cash vs promo) | Bono v48 Phase 375 | Committed, not deployed |
| Cafe wallet integration | Bono v48 Phase 376 | Committed, not deployed |
| PWA session report + leaderboard | Bono v48 Phase 377 | Committed, not deployed |
| Marketing engine (empty hours) | Bono v48 Phase 378 | Committed, not deployed |
| Domain events foundation | Bono v48 Phase 379 | Committed, not deployed |
| Fix-scope blast radius tool | Bono v48 Phase 381 | Committed, not deployed |
| ADAPTER-SWAP F1 25 fixes (5 commits) | Bono debug sessions | Committed, not deployed |

## Wave 1 — Deploy & Verify (Gate: all code deployed + healthy)

Ship everything that's committed but sitting undeployed. Nothing else starts until the codebase running on production matches the codebase in git.

- [ ] **DPLY-01**: All v48 committed code (phases 369, 372, 374-381) built and deployed to server .23 + Bono VPS
- [ ] **DPLY-02**: Phase 363 (data recording verification) deployed to server .23 — lap audit, telemetry completeness, CSV fallback, billing 5s grace window all active
- [ ] **DPLY-03**: ADAPTER-SWAP F1 25 fixes (commits 96940ad0 through 5d2d0877) deployed to all 8 pods via canary rollout (Pod 8 first)
- [ ] **DPLY-04**: routes.rs split (Phase 380, 55 modules) compiles and serves all existing endpoints identically — zero functional regression
- [ ] **DPLY-05**: Phase 363 billing 5s grace window verified: lap arriving within 5s of session end updates refund calc before commit
- [ ] **DPLY-06**: `/api/v1/health` on server .23 returns 200 with matching build_id after deploy

## Wave 2 — Lap Recording (P0 Gate: `SELECT COUNT(*) FROM laps` > 0 on production)

Uday's #1 pain point. Nothing else starts until a customer drives and their lap appears in the database.

### Lap Recording Wiring

- [ ] **LAPR-01**: AC adapter is swapped on LaunchGame command — when staff launches AC, the correct sim adapter binds to shared memory (not the boot-time default)
- [ ] **LAPR-02**: Adapter connect retry loop — if shared memory isn't immediately available after game launch, adapter retries every 2s for up to 60s
- [ ] **LAPR-03**: persist_lap works without an active billing session — laps are recorded even during free trials or when billing is paused
- [ ] **LAPR-04**: AC laps flow end-to-end: shared memory → rc-agent adapter → WS to server → SQLite `laps` table → PWA leaderboard within 10s
- [ ] **LAPR-05**: F1 25 laps flow end-to-end: UDP port 20777 → rc-agent adapter → WS to server → SQLite `laps` table
- [ ] **LAPR-06**: Telemetry (speed, gear, throttle, brake) captured and stored for both AC and F1 25

### Verification

- [ ] **VRFY-01**: Uday verifies at venue: launch AC from kiosk, drive 3 laps, check PWA leaderboard — laps appear
- [ ] **VRFY-02**: Uday verifies at venue: launch F1 25 from kiosk, drive 3 laps, check PWA leaderboard — laps appear
- [ ] **VRFY-03**: Session report page on PWA shows laps, best time, consistency, and telemetry after session ends

## Wave 3 — Architecture Completion (P1)

Finish the decomposition started in v48. Blocks testability and maintainability.

- [ ] **ARCH-01**: billing.rs (9,142 lines) split into: wallet.rs, session_lifecycle.rs, pricing.rs, post_session_hooks.rs — each under 500 lines
- [ ] **ARCH-02**: db/mod.rs (5K+ lines) split by department table groups — each under 500 lines
- [ ] **ARCH-03**: All remaining files >500 lines split along department boundaries
- [ ] **ARCH-04**: CI gate runs `cargo test` + `cargo clippy` before merge to main
- [ ] **ARCH-05**: Dead code audit — remove unused features identified by Feature Registry (Phase 382). Target: 10-20% codebase reduction

## Wave 4 — Revenue Engine (P2)

Autonomous systems that drive revenue without human intervention. The core v3.0 vision.

### Autonomous Pricing

- [ ] **PRCG-01**: Expense data model — rent (₹1.6L), salaries (₹80K), utilities (₹60K), marketing (₹1.5L), cafe inventory (₹12K) stored in `business_expenses` table with monthly update capability
- [ ] **PRCG-02**: Break-even calculator — given expenses + current session count, computes minimum price per session to cover costs
- [ ] **PRCG-03**: Optimal price engine — factors: time of day, day of week, historical demand, competitor pricing, expense data → recommended price per tier
- [ ] **PRCG-04**: Auto-update pricing — Bono adjusts `pricing_rules` table based on engine output, reflected in PWA and billing within 1 sync cycle. Uday does NOT approve — Bono decides.
- [ ] **PRCG-05**: Pricing dashboard — admin page showing: current prices, engine recommendations, historical price changes, revenue impact

### Customer Preferences (Anti-Spam Foundation)

- [ ] **PREF-01**: `customer_preferences` table — per-customer opt-in/opt-out for promotional messages, channel preference (WhatsApp/Discord/PWA), frequency cap
- [ ] **PREF-02**: Opt-out via WhatsApp — customer sends "stop" or similar → immediately removed from promotional list. Transactional messages (booking confirmations, receipts) unaffected.
- [ ] **PREF-03**: Frequency cap enforcement — even opted-in customers receive max 2-3 promotional messages per week
- [ ] **PREF-04**: Engagement throttle — if customer ignores 3 consecutive offers, stop sending until they re-engage
- [ ] **PREF-05**: PWA preferences page — customer can manage their communication preferences

### Autonomous Marketing

- [ ] **AMKT-01**: Empty hour detection — system identifies pods with 0 active sessions during typically busy hours
- [ ] **AMKT-02**: Targeted offer generation — based on customer preferences, past behavior, and available inventory, generate personalized offers
- [ ] **AMKT-03**: WhatsApp delivery — push offers to opted-in customers via WhatsApp (Evolution API). Only to customers who haven't opted out.
- [ ] **AMKT-04**: Offer tracking — track which offers were sent, opened, redeemed. Feed back into engagement throttle.
- [ ] **AMKT-05**: Cafe + racing combo promotions — "Book 1 hour, get free coffee" type deals auto-generated during empty hours

## Wave 5 — Game Launch Completion (P2)

Full multi-game support beyond AC.

- [ ] **GAME-01**: F1 25 full launch — staff launches from kiosk, game starts on pod, telemetry flows, laps recorded. Verified on all 8 pods.
- [ ] **GAME-02**: iRacing basic launch — staff launches from kiosk, game starts on pod. Telemetry and lap recording functional.
- [ ] **GAME-03**: LMU launch — staff launches from kiosk, game starts on pod with timer billing
- [ ] **GAME-04**: Multiplayer AC session — 2+ pods launch simultaneously, all laps recorded, billing atomic

## Wave 6 — Polish & Access (P3)

Visible improvements for customers and staff.

- [ ] **DISP-01**: Leaderboard display on spectator PCs (192.168.31.200, .32, .84, .37) — shows live lap times during active sessions
- [ ] **DISP-02**: Live circuit viewer (Phase 335 code) deployed to spectator PCs — car positions update at 10Hz
- [ ] **CLUD-01**: Cloud dashboard public access — DNS A record (cloud.racingpoint.cloud → 72.60.101.58) + TLS via certbot
- [ ] **CLUD-02**: Cloud dashboard magic-link auth works end-to-end (WhatsApp OTP to Uday)
- [ ] **CHKL-01**: Digital staff checklist system — pod status, cleaning, hardware checks with audit trail in DB
- [ ] **CHKL-02**: Morning opening checklist + evening closing checklist templates
- [ ] **CHKL-03**: Checklist compliance visible on admin dashboard — which staff completed which checks

## v47.0 Completion (James — Parallel Track)

These are James's v47.0 phases that remain incomplete. They run in parallel with v49 waves. Not renumbered — tracked in James's existing GSD.

| Phase | Description | v49 Dependency |
|---|---|---|
| 345 | Backend Resilience (no 500s, lazy-load admin.db) | None — independent |
| 346 | Cafe Menu Proxy Rewrite (SSOT) | Blocks CAFE integration testing |
| 350 | Contract Tests (Playwright) | Blocks Wave 6 readiness review |
| 354 | UI Hardening (loading/empty/error states) | None — independent |
| 355 | Venue-Ready Readiness Review | Blocks milestone close |
| 356 | Business Rules Config Table | Blocks PRCG-04 (pricing auto-update) |
| 357 | Pricing Tiers CRUD | Blocks PRCG-04 |
| 358 | Cafe Promos Admin Page | Blocks AMKT-05 |
| 359 | Bonus Tiers Admin Page | None — independent |
| 360 | Topup Presets SSOT (remaining) | None — independent |

## Execution Commitments (inherited from v48, adapted for v49)

1. **Deploy before build.** No new code until all committed v48 code is deployed and verified on production.
2. **Laps gate everything.** No Wave 3+ work until LAPR-01 through LAPR-06 are verified with real laps at the venue.
3. **Uday gates P0.** Wave 2 completion requires Uday at the venue: launch game, drive laps, see them on leaderboard.
4. **Anti-spam is a hard constraint.** All marketing features (AMKT-*) must have opt-in/opt-out (PREF-*) deployed and verified BEFORE any promotional message is sent. Violation = immediate rollback.
5. **Shared contracts before code.** Bono-James phases require rc-common contract agreement before implementation.
6. **Daily deployable increments.** No phase runs >3 days without a deployable result on a real pod.
7. **Fix by subtraction.** Bug fixes that add more lines than they remove require commit message justification.

## Out of Scope

| Feature | Reason |
|---|---|
| Multi-venue support | Single-venue SQLite; migrate to Postgres when scaling |
| Mobile native app | PWA-first approach |
| Instagram DM bot | No API integration yet; defer to v50 |
| Email campaigns | Resend setup needed; defer to v50 |
| WhatsApp lead nurturing | Needs operational bot first; defer to v50 |
| Discord expansion | 9 commands sufficient for now |
| Content pipeline | Staff-reviewed content; defer to v50 |
| VR support | No VR hardware |
| VNM Motion integration | Future hardware upgrade |

## Traceability

Filled by roadmap phase. Format: `REQ-ID → Phase NNN`.

| Requirement | Phase | Executor |
|---|---|---|
| DPLY-01..06 | Phase 383 | Joint |
| LAPR-01..06 | Phase 384 | Bono |
| VRFY-01..03 | Phase 384 | Joint (Uday verifies) |
| ARCH-01..05 | Phase 385 | Bono |
| PRCG-01..05 | Phase 386 | Bono |
| PREF-01..05 | Phase 387 | Bono |
| AMKT-01..05 | Phase 388 | Bono |
| GAME-01..04 | Phase 389 | Joint |
| DISP-01..02 | Phase 390 | James |
| CLUD-01..02 | Phase 390 | Bono |
| CHKL-01..03 | Phase 391 | Joint |

**Coverage:**
- Wave 1 (Deploy): 6 requirements
- Wave 2 (Lap Recording): 9 requirements
- Wave 3 (Architecture): 5 requirements
- Wave 4 (Revenue): 15 requirements
- Wave 5 (Game Launch): 4 requirements
- Wave 6 (Polish): 8 requirements
- Total: 47 requirements mapped
- Unmapped: 0
- v47.0 parallel: 10 phases tracked (James's GSD)

---
*Requirements defined: 2026-04-14*
*Predecessor: v48.0 Requirements (2026-04-13) — 63 reqs, partially satisfied*
*Business context: ₹4.62L/month costs, 965 drivers, 75% one-time visitors, Pitlane competitor*
