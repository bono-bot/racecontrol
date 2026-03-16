# Roadmap: RaceControl

## Completed Milestones

<details>
<summary>v1.0 RaceControl HUD & Safety — 5 phases, 15 plans (Shipped 2026-03-13)</summary>

See [milestones/v1.0-ROADMAP.md](milestones/v1.0-ROADMAP.md) for full phase details and plan breakdown.

Phases: State Wiring & Config Hardening → Watchdog Hardening → WebSocket Resilience → Deployment Pipeline Hardening → Blanking Screen Protocol

</details>

<details>
<summary>v2.0 Kiosk URL Reliability — 6 phases, 12 plans (Shipped 2026-03-14)</summary>

Phases: Diagnosis → Server-Side Pinning → Pod Lock Screen Hardening → Edge Browser Hardening → Staff Dashboard Controls → Customer Experience Polish

</details>

<details>
<summary>v3.0 Leaderboards, Telemetry & Competitive — Phases 12–13.1 complete, 14–15 paused (2026-03-15)</summary>

Phases complete: Data Foundation → Leaderboard Core → Pod Fleet Reliability (inserted)
Phases paused: Events and Championships (Phase 14), Telemetry and Driver Rating (Phase 15) — deferred until v4.0 completes.

</details>

<details>
<summary>v4.0 Pod Fleet Self-Healing — Phases 16–22 (Shipped 2026-03-16)</summary>

Phases: Firewall Auto-Config → WebSocket Exec → Startup Self-Healing → Watchdog Service → Deploy Resilience → Fleet Health Dashboard → Pod 6/7/8 Recovery and Remote Restart Reliability

</details>

<details>
<summary>v4.5 AC Launch Reliability — Phases 28–32 (Shipped 2026-03-16)</summary>

Phases: Billing-Game Lifecycle → Game Crash Recovery → Launch Resilience → Multiplayer Server Lifecycle → Synchronized Group Play

Key: billing↔game lifecycle wired end-to-end; CM fallback diagnostics; acServer.exe auto-start/stop on booking/billing; kiosk self-serve multiplayer with per-pod PINs; coordinated group launch + continuous race mode + join failure recovery.

</details>

<details>
<summary>v5.0 RC Bot Expansion — Phases 23–26 (Shipped 2026-03-16)</summary>

Phases: Protocol Contract + Concurrency Safety → Crash, Hang, Launch + USB Bot Patterns → Billing Guard + Server Bot Coordinator → Lap Filter, PIN Security, Telemetry + Multiplayer

</details>

## Current Milestone

### v5.5 Billing Credits (Phases 33–35)

**Milestone Goal:** Replace hardcoded paise billing tiers with DB-driven credits (1 credit = ₹1 = 100 paise), non-retroactive 3-tier per-minute rates configurable from the admin panel without a code deploy, and every screen shows credits instead of rupees.

## Phases

**Phase Numbering:**
- Integer phases (1, 2, 3): Planned milestone work
- Decimal phases (2.1, 2.2): Urgent insertions (marked with INSERTED)

Decimal phases appear between their surrounding integers in numeric order.

- [x] **Phase 33: DB Schema + Billing Engine** - billing_rates table, seed data, cloud sync registration, non-retroactive cost algorithm, in-memory rate cache, and rc-common protocol field rename land together — consuming crates compile against the new schema and types before any UI or API is written (completed 2026-03-16)
- [x] **Phase 34: Admin Rates API** - four CRUD endpoints for billing_rates are wired into racecontrol routes — staff can read, create, update, and delete rate tiers via HTTP, and every write immediately invalidates the in-memory cache (completed 2026-03-16)
- [x] **Phase 35: Credits UI** - every user-facing surface that previously showed rupees now shows credits — overlay, kiosk billing modal, admin billing history, and admin pricing page are all updated in a single frontend pass (completed 2026-03-16)

## Phase Details

### Phase 33: DB Schema + Billing Engine
**Goal**: The billing_rates table exists in the DB with seed data, the non-retroactive cost algorithm is live, the in-memory rate cache is wired into BillingManager, and the rc-common protocol field is renamed with backward-compat alias — all consuming crates compile and existing tests stay green before any admin API or UI is built
**Depends on**: Phase 32 (v4.5 Synchronized Group Play — last completed phase)
**Requirements**: RATE-01, RATE-02, RATE-03, BILLC-02, BILLC-03, BILLC-04, BILLC-05, PROTOC-01, PROTOC-02
**Success Criteria** (what must be TRUE):
  1. `cargo test -p rc-common` passes after `minutes_to_next_tier` rename with `#[serde(alias = "minutes_to_value_tier")]` — old JSON field name still deserializes without error, confirmed by a round-trip unit test
  2. A billing session of 45 minutes costs exactly 1050 cr: `compute_session_cost(45, &tiers)` returns 1050 — (30 × 25) + (15 × 20), not 45 × 20 — confirmed by unit test in billing.rs
  3. BillingManager starts with hardcoded defaults in its rate cache and the `cargo test -p racecontrol-crate` billing suite passes — no DB required for the cache to initialise
  4. After server startup, `GET /billing/rates` returns 3 seed rows (Standard 2500 p/min, Extended 2000 p/min, Marathon 1500 p/min) read from the DB
  5. The billing_rates table name appears in SYNC_TABLES and a cloud sync push run does not error on a clean DB — confirmed by checking cloud_sync.rs SYNC_TABLES list and running `cargo test` with sync tests passing
**Plans**: 1 plan

Plans:
- [ ] 34-01-PLAN.md — Wave 1: Fix POST/DELETE status codes (201/204) + 4 integration tests (ADMIN-01, ADMIN-02, ADMIN-03, ADMIN-04)

Plans:
- [ ] 33-01-PLAN.md — Wave 1: Fix seed capitalization, add serde alias round-trip test, add billing_rates seed count assertion (RATE-01, RATE-02, RATE-03, BILLC-02, BILLC-03, BILLC-04, BILLC-05, PROTOC-01, PROTOC-02)

### Phase 34: Admin Rates API
**Goal**: Staff can manage billing rate tiers through four CRUD HTTP endpoints — every write triggers immediate cache invalidation so the billing engine picks up the new rates on the next per-second tick without waiting for the 60-second background refresh
**Depends on**: Phase 33
**Requirements**: ADMIN-01, ADMIN-02, ADMIN-03, ADMIN-04
**Success Criteria** (what must be TRUE):
  1. `GET /billing/rates` returns all active rate tiers as JSON — confirmed by integration test asserting the 3 seed rows are returned after a clean migration
  2. `POST /billing/rates` with a valid payload inserts a new tier and returns 201 — a subsequent GET includes the new row
  3. `PUT /billing/rates/{id}` with an updated rate_per_min_paise value persists the change and the BillingManager rate cache reflects the new value within one billing tick (under 1 second) — no server restart required
  4. `DELETE /billing/rates/{id}` removes the tier and returns 204 — a subsequent compute_session_cost() call does not include the deleted tier's contribution, confirmed by unit test
**Plans**: 1 plan

Plans:
- [ ] 34-01-PLAN.md — Wave 1: Fix POST/DELETE status codes (201/204) + 4 integration tests (ADMIN-01, ADMIN-02, ADMIN-03, ADMIN-04)

### Phase 35: Credits UI
**Goal**: Every customer-facing and staff-facing screen that previously displayed a rupee amount now shows a credit value — the string "Rs." or formatINR no longer appears anywhere in the overlay, kiosk, or admin billing pages
**Depends on**: Phase 34
**Requirements**: BILLC-01, UIC-01, UIC-02, UIC-03, UIC-04
**Success Criteria** (what must be TRUE):
  1. The rc-agent overlay's `format_cost()` function renders "45 cr" for a 45-credit cost — "Rs." and "₹" are absent from overlay output, confirmed by unit test on the format function
  2. The admin billing history page shows the credit amount (e.g. "1050 cr") in the cost column — no call to formatINR remains in the billing history component, confirmed by grep
  3. The admin pricing page displays a Per-Minute Rates table with the 3 seed tiers and allows inline editing of rate_per_min_paise — a staff member can change a rate and save it without leaving the page
  4. BillingStartModal shows the estimated cost in credits ("~25 cr/min for first 30 min") — no rupee formatting in the modal, confirmed by UI component test
  5. A full booking flow from kiosk start to session end shows credits at every cost display point — overlay during play, summary screen at end, admin history after session — no rupee strings anywhere in the user journey
**Plans**: 1 plan

Plans:
- [ ] 35-01-PLAN.md — Wave 1: Add UIC-01 test assertions + grep verification (BILLC-01, UIC-01, UIC-02, UIC-03, UIC-04)

## Progress

**Execution Order:**
Phases execute in numeric order: 33 → 34 → 35

Note: Phase 33 (DB + Engine) is non-negotiable first — billing_rates table and the BillingRateTier type must exist before CRUD routes can reference them, and the rc-common protocol rename must compile cleanly in both consuming crates before any phase-34 or phase-35 code touches protocol.rs. Phase 34 (Admin API) depends on the DB schema and cache from Phase 33. Phase 35 (Credits UI) depends on Phase 34 having live endpoints so the pricing page can wire up inline editing against real API calls.

| Phase | Milestone | Plans Complete | Status | Completed |
|-------|-----------|----------------|--------|-----------|
| 1. State Wiring & Config Hardening | v1.0 | 3/3 | Complete | 2026-03-13 |
| 2. Watchdog Hardening | v1.0 | 3/3 | Complete | 2026-03-13 |
| 3. WebSocket Resilience | v1.0 | 3/3 | Complete | 2026-03-13 |
| 4. Deployment Pipeline Hardening | v1.0 | 3/3 | Complete | 2026-03-13 |
| 5. Blanking Screen Protocol | v1.0 | 3/3 | Complete | 2026-03-13 |
| 6. Diagnosis | v2.0 | 2/2 | Complete | 2026-03-13 |
| 7. Server-Side Pinning | v2.0 | 2/2 | Complete | 2026-03-14 |
| 8. Pod Lock Screen Hardening | v2.0 | 3/3 | Complete | 2026-03-14 |
| 9. Edge Browser Hardening | v2.0 | 1/1 | Complete | 2026-03-14 |
| 10. Staff Dashboard Controls | v2.0 | 2/2 | Complete | 2026-03-14 |
| 11. Customer Experience Polish | v2.0 | 2/2 | Complete | 2026-03-14 |
| 12. Data Foundation | v3.0 | 2/2 | Complete | 2026-03-14 |
| 13. Leaderboard Core | v3.0 | 5/5 | Complete | 2026-03-15 |
| 13.1. Pod Fleet Reliability | v3.0 | 3/3 | Complete | 2026-03-15 |
| 14. Events and Championships | 5/5 | Complete    | 2026-03-16 | - |
| 15. Telemetry and Driver Rating | v3.0 | 0/? | Deferred | - |
| 16. Firewall Auto-Config | v4.0 | 1/1 | Complete | 2026-03-15 |
| 17. WebSocket Exec | v4.0 | 3/3 | Complete | 2026-03-15 |
| 18. Startup Self-Healing | v4.0 | 2/2 | Complete | 2026-03-15 |
| 19. Watchdog Service | v4.0 | 2/2 | Complete | 2026-03-15 |
| 20. Deploy Resilience | v4.0 | 2/2 | Complete | 2026-03-15 |
| 21. Fleet Health Dashboard | v4.0 | 2/2 | Complete | 2026-03-15 |
| 22. Pod 6/7/8 Recovery + Remote Restart Reliability | v4.0 | 2/2 | Complete | 2026-03-16 |
| 28. Billing-Game Lifecycle | v4.5 | 2/2 | Complete | 2026-03-16 |
| 29. Game Crash Recovery | v4.5 | 2/2 | Complete | 2026-03-16 |
| 30. Launch Resilience | v4.5 | 2/2 | Complete | 2026-03-16 |
| 31. Multiplayer Server Lifecycle | v4.5 | 2/2 | Complete | 2026-03-16 |
| 32. Synchronized Group Play | v4.5 | 2/2 | Complete | 2026-03-16 |
| 23. Protocol Contract + Concurrency Safety | v5.0 | 2/2 | Complete | 2026-03-16 |
| 24. Crash, Hang, Launch + USB Bot Patterns | v5.0 | 4/4 | Complete | 2026-03-16 |
| 25. Billing Guard + Server Bot Coordinator | v5.0 | 4/4 | Complete | 2026-03-16 |
| 26. Lap Filter, PIN Security, Telemetry + Multiplayer | v5.0 | 4/4 | Complete | 2026-03-16 |
| 27. Tailscale Mesh + Internet Fallback | v5.0 | 5/5 | Complete | 2026-03-16 |
| 33. DB Schema + Billing Engine | 1/1 | Complete    | 2026-03-16 | - |
| 34. Admin Rates API | 1/1 | Complete    | 2026-03-16 | - |
| 35. Credits UI | 1/1 | Complete   | 2026-03-16 | - |

### Phase 27: Tailscale Mesh + Internet Fallback

**Goal:** All 8 pods, server, and Bono's VPS join a Tailscale mesh network — installed as a Windows Service via WinRM, cloud_sync routes through Tailscale IP, and the server pushes telemetry/game state/pod health events to Bono in real time with a bidirectional command relay for PWA-triggered game launches
**Requirements**: TS-01, TS-02, TS-03, TS-04, TS-05, TS-06, TS-DEPLOY
**Depends on:** Phase 26
**Plans:** 1/1 plans complete

Plans:
- [x] 27-01-PLAN.md — Wave 1 (TDD): BonoConfig in config.rs + bono_relay.rs skeleton with 3 RED test stubs (TS-01, TS-02, TS-03, TS-04)
- [x] 27-02-PLAN.md — Wave 2: Full bono_relay.rs implementation — spawn loop, push_event, handle_command, build_relay_router; AppState bono_event_tx channel (TS-02, TS-03, TS-04)
- [x] 27-03-PLAN.md — Wave 3: main.rs wiring — bono_relay::spawn() + second Axum listener on Tailscale IP:8099 (TS-02, TS-03, TS-06)
- [x] 27-04-PLAN.md — Wave 2 (parallel): scripts/deploy-tailscale.ps1 — WinRM fleet deploy script, canary Pod 8 first (TS-DEPLOY)
- [x] 27-05-PLAN.md — Wave 4: racecontrol.toml [bono] section + build + deploy + human verify Pod 8 Tailscale IP + relay 401 auth (TS-05, TS-06, TS-DEPLOY)
