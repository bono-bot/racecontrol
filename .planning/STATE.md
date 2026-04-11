---
gsd_state_version: 1.0
milestone: v40.0
milestone_name: Game Launch Reliability
status: Phase complete — ready for verification
stopped_at: Completed 368-04-PLAN.md Tasks 0-3; stopped at Task 4 MMA audit checkpoint
last_updated: "2026-04-11T14:22:21.960Z"
progress:
  total_phases: 4
  completed_phases: 4
  total_plans: 4
  completed_plans: 4
  percent: 100
---

# Project State — v47.0 Admin Dashboard Venue-Ready Hardening (ACTIVE)

## Project Reference

See: .planning/PROJECT.md

**Core value (v47.0):** Make the admin dashboard (venue .23:3201 + cloud admin.racingpoint.cloud) a resilient, venue-ready single source of truth before customers start using the venue. Close 18 audit findings from the 2026-04-09 Vishal-PIN incident + the 2026-04-09 Admin Dashboard audit (cloud admin down, ABI mismatch, dead cafe menu table, no venue↔cloud replication, 3 missing rc endpoints), and absorb the superseded Phase 343 Plan 03 (admin PIN UI).

**Current focus:** Phase 368 — live-launch-status-with-autonomous-debug

## Parallel Milestone (PAUSED)

**v46.0 Game Launch Diagnostics** paused at user directive 2026-04-10. State preserved at `.planning/milestones/v46.0-STATE-snapshot.md`.

Phase 363 is **CODE-COMPLETE but NOT SHIPPED** — all 3 plans committed (`e4784c51`, `aadefeb6`, `09be10e6`, `7e46227b`, `11450490`, `1e3eff44`), 891 racecontrol-crate tests pass + 254 rc-agent-crate tests pass (zero failures), 7 new Phase 363-03 tests green (F-05 formula + SQL invariant + 3 grace window + 2 lap reject). **MMA audit + binary build + server deploy + cloud parity still outstanding.** Production still runs `d4359d2e` (pre-v46.0); F-05 refund bug and GLD-C-04 lap-reject race remain live until deploy.

Resume v46.0 via: `cp .planning/milestones/v46.0-STATE-snapshot.md .planning/STATE.md`

## Current Position

Phase: 368 (live-launch-status-with-autonomous-debug) — COMPLETE
Plan: 4 of 4 (all plans done)
**Milestone:** v47.0 Admin Dashboard Venue-Ready Hardening
**Progress:** [██████████] 100%
**Next unblocked phases (no deploy/infra dependencies):** 348, 352

### Phase 368 Key Decisions

- **sqlx::query() dynamic over sqlx::query!() macro:** Codebase has no DATABASE_URL or .sqlx offline cache; macro breaks compilation. Used dynamic queries with explicit type annotation.
- **StaffClaims.sub as staff_id and staff_name:** StaffClaims struct has no name field; sub is the staff identifier. Both fields use claims.sub until a future phase adds name to JWT claims.
- **Tier 2+ approve-fix wires to LaunchStateMachine only:** Full tier_engine wiring deferred per plan spec as TODO(368-follow-up). Approval transitions card to IssueBeingFixed and broadcasts LaunchStatusChanged.

### Shipped to git (from 2026-04-09 session per SESSION-HANDOFF.md)

| Phase | Scope | Racecontrol commit | Admin commit | Ship state |
|---|---|---|---|---|
| Scaffold | Milestone artifacts (PROJECT, STATE, REQUIREMENTS, ROADMAP, research×5) | `bc355a11` | — | git |
| 344 Unbreakable Deploys | admin-deploy.sh + verify-deploy.js + server-bootstrap.js + Node pin + stale PIN script archive | `a7859cad` | `b10b487` | git — **NOT live-deployed** |
| 345 Backend Resilience | rc proxy env-in-handler + admin.db lazy-load + AdminDbError + C5 JWT literal removal + C6 webhook rejection | `7e00d1e4` | `f4268d1` | git — **NOT live-deployed** |
| 346-01 Cafe Proxy (scaffolding) | Dual-path cafe/menu route with CAFE_PROXY_ENABLED flag (default off) + schema diff doc | `b6f2effa` | `613d1c4` | git — flag off, safe |
| 343 Staff PIN Hardening | Cloud-authority 409 guard + post-write verify + delayed sync verify + E2E spec | `b31c38e0`, `6c870f99`, `4074bb0d` | — | git — **NOT live-deployed** |
| 354-01 Nav Cleanup | Hide /memberships + /wallet-transactions from sidebar | — | `79d2649` | git — **NOT live-deployed** |
| 354-02 Skeleton Loading | Replace 13 "Loading..." text states with SkeletonTable across 11 pages | — | `4c24bad` | git — **NOT live-deployed** |
| 368-01 State Machine | LaunchStateMachine + 4-state model + DashboardEvent + LaunchNoteEvent + protocol types | `368-01 commits` | — | git — code-complete |
| 368-02 WS Push | rc-agent→server LaunchStatusUpdate fanout, ws_handler hydration, retry boundary emissions | `368-02 commits` | — | git — code-complete |
| 368-03 DB + REST | launch_notes table + staff_dismissed_at ALTER + kiosk_launch_cards_enabled flag + 5 REST endpoints | `74c06377`, `9c91f102` | — | git — code-complete |

### Phase status

- ✅ **344** Unbreakable Deploys — shipped to git, 3 plans committed
- ✅ **345** Backend Resilience — shipped to git, 3 plans committed
- 🟡 **346** Cafe Menu Proxy — 346-01 scaffolding shipped; 346-02 cutover REQUIRES venue closure maintenance window (can't do autonomously)
- ✅ **343** Staff PIN Hardening — code-complete. Plans 01 (`b31c38e0`), 02 (`6c870f99`), 04 (`4074bb0d`) all committed. Deploy pending.
- 🔒 **347** Admin Staff Management — **BLOCKED** on Phase 343 live deploy.
- 🟢 **348** Auth Resilience — no external deps; new `lockout.rs` Rust module in racecontrol, extend admin lockout. MMA audit required (cross-system auth bridge).
- ⏸ **349** Litestream Sync Contract — requires Backblaze B2 setup + live Windows infra
- 🟡 **350** Contract Tests — can start but staff PIN tests depend on Phase 347 (which depends on 343)
- ⏸ **351** Data Durability — backups, live infra
- 🟢 **352** Health + WhatsApp Alerts — no external deps; per-subsystem probes in `/api/health`, comms-link relay integration. MMA audit likely required (cross-system alert bridge).
- ⏸ **353** Runbook + Staff Training — physical printed materials, requires Uday
- ✅ **354** UI Hardening — 354-01 nav cleanup (`79d2649`) + 354-02 skeleton loading (`4c24bad`) shipped to git. 354-03 (/settings/health) already existed. Deploy pending.
- 🔲 **355** Venue-Ready Readiness Review — final human gate, requires 344-354 shipped
- 🔲 **356** Business Rules Config Table — pending
- 🔲 **357** Pricing Tiers CRUD — pending
- 🔲 **358** Cafe Promos Admin Page — pending
- 🔲 **359** Bonus Tiers Admin Page — pending
- 🟡 **360** Topup Presets SSOT — partially shipped 2026-04-09 (commit `0c7a8d86`), Playwright coverage pending via Phase 350

## Accumulated Context

### Milestone origin (from 2026-04-09 session)

v47.0 was triggered by the 2026-04-09 Admin Dashboard audit which found:

- Cloud admin fully down (login 500 from missing RC_URL env, static assets 404)
- Local admin better-sqlite3 ABI mismatch (Node 24 vs binding built for Node 22)
- Cafe menu editor wired to dead `admin.db.menu_items` table (never reaches POS/kiosk)
- No racecontrol.db replication between venue and cloud (210 drivers venue vs 21 cloud)
- 3 missing rc endpoints (`/customer/drivers`, `/customer/membership/active`, `/customer/membership/tiers`)
- Phase 343 Vishal-PIN incident findings (18 code + data gaps)

Expanded 2026-04-09 (commit `f1c741e2`) from 12 → 17 phases after SSOT gap audit — added business rules config (356), pricing tiers CRUD (357), cafe promos (358), bonus tiers (359), topup presets SSOT (360).

### Scope decisions (2026-04-09)

- **Sync topology:** Litestream venue→cloud read replica (Option A)
- **Phase numbering:** starts at 344 (continues from Phase 343 precursor)
- **Hard dependency:** Phase 343 Plans 01+02+04 must ship BEFORE Phase 347
- **Subagent gates:** frontend phases require ui-researcher + ui-auditor; business logic phases require nyquist-auditor; new cross-system bridges require MMA audit

### Roadmap Evolution

- **2026-04-09:** v47.0 expanded 12 → 17 phases (added 356–360: business rules, pricing tiers CRUD, cafe promos, bonus tiers, topup presets SSOT)
- **2026-04-11:** Phase 368 added — Live Launch Status with Autonomous Debug. Kiosk /debug page grows real-time per-launch status cards (4-state model), WS-push only, launch-phase bounded, billing internal, inline staff notes. Depends on Phase 275 (autonomous rc-agent retry already shipped). NOTE: gsd-tools phase add initially computed collision number 315; manually renumbered to 368 (max integer phase + 1).

### Session Continuity

Last session: 2026-04-11T14:22:21.956Z
Stopped at: Completed 368-04-PLAN.md Tasks 0-3; stopped at Task 4 MMA audit checkpoint

Phase 368 Plans 01-03 complete. Plan 04 (kiosk TypeScript LaunchCard component) is the final plan in Phase 368.

## Next Action

**Pick one unblocked phase to execute (code-only, no deploy):**

1. **Phase 354 UI Hardening** — Lowest risk. Pure frontend Next.js. Scope: hide `/memberships` + `/wallet-transactions` nav, add loading/empty/error states. No racecontrol binary changes, no MMA required, no deploy. Mandatory gates: ui-researcher → plan → execute → ui-auditor.

2. **Phase 348 Auth Resilience** — Medium complexity. New `lockout.rs` Rust module in racecontrol + admin lockout extension. Cross-system auth bridge → **MMA audit MANDATORY per CLAUDE.md.** Mandatory gate: nyquist-auditor.

3. **Phase 352 Health + WhatsApp Alerts** — Medium complexity. Per-subsystem probes in `/api/health` + comms-link relay integration for alert dispatch. Cross-system alert bridge → **MMA audit likely required.** Probe design must avoid "lies" (CLAUDE.md §"Probes that lie").

**Recommendation:** 354 first (fastest to complete, zero MMA cost, clean gates). 348 next (biggest security value, MMA-gated). 352 last (needs external infra probes).

Awaiting user pick.
