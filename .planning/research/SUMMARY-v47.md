---
milestone: v47.0
dimension: summary
date: 2026-04-09
sources:
  - STACK-v47.md
  - FEATURES-v47.md
  - ARCHITECTURE-v47.md
  - PITFALLS-v47.md
  - .planning/phases/343-staff-pin-hardening/343-COORDINATION.md
---

# v47.0 Research Synthesis — Admin Dashboard Venue-Ready Hardening

## TL;DR

- **11 hardening themes** across deploys, resilience, SSOT, sync contract, health/alerts, contract tests, auth, backups, UI, runbook, and staff management
- **Phase 343 Plans 01+02 are hard prerequisites** for theme 11 (admin PIN UI). All other themes can proceed in parallel.
- **Stack additions:** Litestream (venue→cloud read replica, Option A), nothing else new. Explicitly NO Sentry, Redis, Docker, new SaaS dependencies.
- **Critical P0 pitfalls** (ship-blockers): Phase 343 dependency, Next.js standalone env loading, better-sqlite3 ABI on Node 24, cafe menu cutover data loss, admin deploy path mismatch.
- **Phase numbering starts at 344**. Proposed 12 phases, 3 waves.
- **Success gate:** 18-criterion "Venue Opening Readiness Checklist" (see FEATURES-v47.md)

## Stack additions (final)

| Name | Version | Why | Risk |
|---|---|---|---|
| **Litestream** | v0.3.x (native Windows binary) | venue→cloud racecontrol.db replication | Medium — Windows support relatively new, needs pre-flight validation in Phase 349 |
| (no other new services) | — | Keep stack lean — Uday has no ops time | — |

Explicit NO: Sentry, Redis, Docker, GlitchTip, Loki, Postgres, rqlite, LiteFS, Turso, Keycloak, React Query, tRPC.

## Feature table stakes (must ship before venue open)

Grouped by theme. Each maps to one or more phases.

| Theme | Table stakes (P0/P1) | Phases |
|---|---|---|
| 1 Unbreakable deploys | admin-deploy.sh, verify-deploy.js, rollback, Node pin, archive stale PIN scripts | 344 |
| 2 Backend resilience | module-load errors → JSON 503, admin.db lazy-load, remove hardcoded JWT default (C5), halt on missing webhook secret (C6) | 345 |
| 3 Single source of truth | cafe menu proxy to rc, drop dead tables (D4), identity consolidation (C8), schema guard | 346 |
| 4 Sync contract | Litestream venue→cloud, cloud read-only badge, lag detection | 349 |
| 5 Health + alerting | per-subsystem probes, WhatsApp alert via comms-link, dedup | 352 |
| 6 Contract tests | Playwright admin→POS/kiosk contract tests, 46-page smoke, 70s sync wait pattern | 350 |
| 7 Auth resilience | per-staff-id lockout (C4), lockout DB persist (C7), 12h JWT, multi-device, break-glass | 348 |
| 8 Data durability | daily sqlite3 .backup, 30d retention, rsync to Bono, restore drill | 351 |
| 9 UI hardening | hide dead routes, loading/empty/error states, 46-page smoke | 354 |
| 10 Runbook + training | printed one-pagers, incident log, staff training | 353 |
| 11 Admin Staff Management | /admin/staff page, change_staff_pin_safe, sync/pull-now (blocked on 343) | 347 |

## Watch out for (P0 pitfalls — from PITFALLS-v47.md)

1. **Phase 343 ships AFTER v47.0 theme 11** — silent PIN revert returns. Feature-flag theme 11, gate on 343 commits.
2. **Next.js standalone doesn't load `.env.local`** — env vars MUST come from process env at Node start. Use `server-bootstrap.js` or explicit bat/pm2 exports.
3. **better-sqlite3 ABI break on Node 24** — pin Node to 22 LTS on venue .23 as PRE-WORK (not inside v47.0 deploy).
4. **Admin deploy path mismatch** (tar extracts to `admin/` not `admin/.next/standalone/`) — cite `feedback_admin_deploy_path.md` in Phase 344 plan.
5. **Cafe menu cutover data loss** — schema diff doc in PLAN.md, dual-read never dual-write, pre-cutover snapshot, maintenance window only.
6. **Litestream Windows support** — pre-flight validation in Phase 349 before committing the topology.
7. **Playwright contract test flakes** — 70s sync-wait tests need `test.slow()` + unique test data + cleanup.
8. **WhatsApp alert storm** — dedup window, rate limiting in comms-link relay.
9. **Auth lockout UX disaster** — whitelist .23 and .20 IPs, break-glass token at venue.
10. **Probes that lie** — test degradation detection by deliberately breaking a subsystem in Phase 352 drill.

## Build order (critical path)

```
Pre-work (NOT in v47.0, but required first):
  (0a) Node 22 LTS install on server .23 (bundle with v47.0 Phase 344 if safe)
  (0b) Phase 343 Plans 01+02+04 executed + deployed in racecontrol

Wave 1 (v47.0 — can start immediately, no Phase 343 dependency):
  Phase 344: Unbreakable deploys (P0) — admin-deploy.sh + verify + rollback
  Phase 345: Backend resilience (P0) — module-load, env validation, admin.db lazy-load
  Phase 346: Cafe menu proxy rewrite (P0) — SSOT for cafe, drop dead tables

Wave 2 (v47.0 — Wave 1 must be green):
  Phase 348: Auth resilience (P1) — lockout, JWT 12h, break-glass
  Phase 349: Litestream sync contract (P1) — replication, lag detection
  Phase 352: Health + WhatsApp alerts (P1) — per-subsystem probes, comms-link integration
  Phase 354: UI hardening (P1) — loading/empty/error, hide dead routes, 46-page smoke

Wave 3 (v47.0 — Phase 343 must be shipped in racecontrol):
  Phase 347: Admin Staff Management (P1) — /admin/staff, change_staff_pin_safe, sync/pull-now
  Phase 350: Contract tests (P1) — Playwright admin→POS/kiosk

Wave 4 (v47.0 — final):
  Phase 351: Data durability (P1) — daily backups, restore drill
  Phase 353: Runbook + staff training (P2) — printed one-pagers, incident log
  Phase 355: Venue-ready readiness review — 18-criterion checklist verification
```

## Architectural decisions (consolidated)

1. **Sync topology: Option A (venue writer, cloud read replica via Litestream)** — confirmed by user.
2. **Storage for Litestream: Backblaze B2** (cheap, egress-free via CF Bandwidth Alliance) — effectively free for our 120MB DB.
3. **Env loading: shared `server-bootstrap.js`** — one file, works on both Windows bat + Linux pm2.
4. **Lockout counters: racecontrol.db** via new `lockout.rs` module, extends existing `auth/admin.rs` LazyLock.
5. **WhatsApp alerts: comms-link relay** at James .27 `/relay/alert` endpoint. Admin + racecontrol both POST.
6. **Error tracking: structured JSON logs** → daily rsync to Bono. No Sentry.
7. **Node pin: 22 LTS** (`.nvmrc`, `"engines"`, bat file path, CI matrix).
8. **Phase 343 dependency enforcement: feature flag + deploy gate script** that checks for merge commits in racecontrol git log.

## Venue Opening Readiness Checklist (from FEATURES-v47.md)

See FEATURES-v47.md "Venue Opening Readiness Checklist" — 18 criteria. Phase 355 will execute the checklist and produce VERIFICATION.md. Criteria 1-15 are P0, 16-18 are P1.

## Open questions (escalate during phase planning)

1. **Does cloud racecontrol already have a read-replica mode?** Phase 349 plan must audit.
2. **Do racecontrol cafe_items fields match admin UI expectations?** Phase 346 plan must grep both schemas FIRST.
3. **Does comms-link `/relay/alert` endpoint exist?** Phase 352 plan must confirm.
4. **Litestream Windows binary viability on Server 2022?** Phase 349 pre-flight.

## What this synthesis is NOT

- Not a phase plan — that's the roadmapper's job (`/gsd:plan-phase 344` etc.)
- Not an implementation guide — plan-phase reads this + other context to produce PLAN.md
- Not authoritative on library versions — verify at implementation time (`npm view`, `cargo search`)
- Not a final scope — user may adjust during requirements confirmation

## Files referenced

| File | Purpose |
|---|---|
| `.planning/research/STACK-v47.md` | Technology choices + Litestream + deploy pattern |
| `.planning/research/FEATURES-v47.md` | Table stakes / differentiators / anti-features per theme |
| `.planning/research/ARCHITECTURE-v47.md` | Topology, integration points, build order, rollback |
| `.planning/research/PITFALLS-v47.md` | Ruthless pitfall enumeration with prevention |
| `.planning/phases/343-staff-pin-hardening/343-COORDINATION.md` | Phase 343 handoff with 18 pre-mapped findings |

---

*Synthesized directly due to agent API overload (synthesizer agent would have done this).*
