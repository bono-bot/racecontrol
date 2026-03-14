# Roadmap: RaceControl Billing & Point of Sale (v3.0)

## Overview

The billing engine in rc-core is complete — timers, cost calculation, dynamic pricing, wallet, split sessions, and double-entry journal entries all work. This milestone adds the *visibility and operations layer* on top: sync billing data to cloud, build the live dashboard and analytics at dashboard.racingpoint.cloud, give customers a proper session receipt on PWA, and give staff refund/discount controls on the kiosk.

No billing engine rewrite. This is pure wiring, frontend, and surface-level backend.

## Phases

**Phase Numbering:**
- Integer phases (1, 2, 3): Planned milestone work
- Decimal phases (2.1, 2.2): Urgent insertions (marked with INSERTED)

Decimal phases appear between their surrounding integers in numeric order.

- [x] **Phase 1: Billing Cloud Sync** - Push completed sessions, wallet transactions, and billing events to cloud within 60s
- [ ] **Phase 2: Cloud Dashboard — Live Operations** - Real-time pod grid, revenue ticker, and session alerts at dashboard.racingpoint.cloud
- [ ] **Phase 3: Cloud Dashboard — Analytics & Reports** - Session list with filters, revenue charts, utilization heatmap, CSV export
- [x] **Phase 4: PWA Session Results & Receipt** - Session detail page (cost, laps, timeline) + WhatsApp receipt on session end
- [ ] **Phase 5: Kiosk POS — Staff Operations** - Payment method recording, refund UI, manual discount UI

## Phase Details

### Phase 1: Billing Cloud Sync
**Goal**: Cloud has a complete, up-to-date read-only copy of all billing data — completed sessions, wallet transactions, and billing events sync within 60 seconds of creation, with no data loss during network outages
**Depends on**: Nothing (first phase)
**Requirements**: SYNC-01, SYNC-02, SYNC-03, SYNC-04, SYNC-05
**Success Criteria** (what must be TRUE):
  1. A completed `billing_sessions` row appears in the cloud database within 60 seconds of `status` being set to `Completed` or `EndedEarly`
  2. A `wallet_transactions` row appears in the cloud database within 60 seconds of being written locally
  3. A `billing_events` row appears in the cloud database within 60 seconds of being written locally
  4. Cloud tables have no write-back path to venue — venue billing data is never overwritten by cloud
  5. If internet is down during a session, all queued rows sync successfully within 60 seconds of reconnect
**Plans**: 1 plan

Plans:
- [x] 01-01-PLAN.md — Add billing_events to push payload + cloud receiver, add missing billing_sessions columns, add created_at index

### Phase 2: Cloud Dashboard — Live Operations
**Goal**: Uday can open dashboard.racingpoint.cloud and immediately see all 8 pods' live status, time remaining, customer names, and today's total revenue — updating every 60 seconds without a page refresh
**Depends on**: Phase 1
**Requirements**: LIVE-01, LIVE-02, LIVE-03, LIVE-04, LIVE-05
**Success Criteria** (what must be TRUE):
  1. Dashboard shows an 8-pod grid with pod number, customer name (or "Idle"), status badge (Idle / Active / Connecting), and time remaining for active sessions
  2. Dashboard shows today's total revenue (₹), number of active sessions right now, and number of completed sessions today
  3. Pod grid and revenue ticker reflect changes within 60 seconds of any session event (start, end, pause)
  4. A pod with ≤ 5 minutes remaining is visually highlighted (e.g. amber border)
  5. Page is accessible at dashboard.racingpoint.cloud — no VPN or venue network required
**Plans**: 2 plans

Plans:
- [ ] 02-01-PLAN.md — Cloud API: GET /dashboard/live (8-pod grid + today's revenue), polling every 30s from synced billing data
- [ ] 02-02-PLAN.md — racingpoint-admin: Live Operations page with pod grid, revenue ticker, 5-min alert highlight (React, 30s auto-refresh)

### Phase 3: Cloud Dashboard — Analytics & Reports
**Goal**: Staff can answer any revenue question for any date range using the cloud dashboard — session list, daily revenue chart, tier breakdown, hourly heatmap, and CSV export — all sourced from synced billing data
**Depends on**: Phase 1
**Requirements**: ANA-01, ANA-02, ANA-03, ANA-04, ANA-05
**Success Criteria** (what must be TRUE):
  1. Staff can view a paginated session list filtered by date range, pod, pricing tier, and status — showing session id, customer name, duration, cost, status
  2. Dashboard shows a bar chart of daily revenue for the last 30 days
  3. Dashboard shows a pie or bar chart of revenue split by pricing tier (30 min, 1 hr, trial, custom)
  4. Dashboard shows a heatmap of pod utilization by hour-of-day (x) and day-of-week (y) as % of pods active
  5. Staff can export the filtered session list as a CSV file with all billing fields
**Plans**: 2 plans

Plans:
- [ ] 03-01-PLAN.md — Cloud API: GET /dashboard/sessions (list + filters), GET /dashboard/analytics (daily revenue, tier breakdown, heatmap), GET /dashboard/export/csv
- [ ] 03-02-PLAN.md — racingpoint-admin: Analytics page with date picker, charts (recharts), heatmap, and CSV download button

### Phase 4: PWA Session Results & Receipt
**Goal**: Customers see a complete session record in the PWA — cost breakdown, lap performance, and pause timeline — and receive a WhatsApp summary after every session ends, so they have a receipt without staff involvement
**Depends on**: Nothing (independent of cloud phases)
**Requirements**: PWA-01, PWA-02, PWA-03, PWA-04, PWA-05
**Success Criteria** (what must be TRUE):
  1. Customer opens `/sessions/{id}` in PWA and sees: base price, discount applied (if any), final amount charged, and refund amount (if any)
  2. Customer sees session performance: total laps, best lap time (mm:ss.mmm), top speed km/h (where telemetry available — N/A otherwise)
  3. Customer sees a session timeline showing: session start time, each pause with reason and duration, each warning (5 min, 1 min), and session end
  4. Within 60 seconds of session end, customer receives a WhatsApp message via Evolution API with: duration, cost, best lap, wallet balance remaining
  5. `/sessions/{id}/public` returns a shareable session summary page with no auth required (first name, duration, best lap only)
**Plans**: 2 plans

Plans:
- [x] 04-01-PLAN.md — rc-core: add events to customer_session_detail, public_session_summary endpoint, WhatsApp receipt via Evolution API in post_session_hooks
- [x] 04-02-PLAN.md — PWA: session timeline + top speed N/A on detail page, public shareable session page at /sessions/[id]/public

### Phase 5: Kiosk POS — Staff Operations
**Goal**: Staff can record how a customer paid, issue refunds for any session, and apply manual discounts — all from the kiosk dashboard without touching a terminal
**Depends on**: Nothing (independent)
**Requirements**: POS-01, POS-02, POS-03, POS-04, POS-05
**Success Criteria** (what must be TRUE):
  1. When creating a booking, staff can select payment method (cash / UPI / card / wallet deduct) — stored in `billing_sessions.payment_method`
  2. Staff can open any completed session from the kiosk and issue a full or partial refund, selecting a reason from a dropdown
  3. Refund is credited to customer wallet immediately — `wallet_transactions` row created, balance updated, session `refund_paise` updated
  4. When creating a booking, staff can apply a fixed ₹ or percentage discount with a mandatory free-text reason — stored in `billing_sessions.discount_paise` and `discount_reason`
  5. Every refund and discount action creates an `audit_log` entry with `staff_id`, `action`, `old_values`, `new_values`, and timestamp
**Plans**: 2 plans

Plans:
- [ ] 05-01-PLAN.md — rc-core: payment_method column in billing_sessions, POST /billing/{id}/refund (already exists — add audit_log write); POST /billing/{id}/discount for pre-session discount; audit_log on all POS mutations
- [ ] 05-02-PLAN.md — Kiosk: payment method selector in booking wizard; Refund modal on session detail; Discount panel on booking review step

## Progress

**Execution Order:**
Phase 1 first (enables cloud phases). Phases 2 and 3 depend on Phase 1. Phases 4 and 5 are independent and can run in parallel with any cloud phase.

| Phase | Plans | Status | Completed |
|-------|-------|--------|-----------|
| 1. Billing Cloud Sync | 1/1 | Complete | 2026-03-14 |
| 2. Cloud Dashboard — Live | 0/2 | Not started | - |
| 3. Cloud Dashboard — Analytics | 0/2 | Not started | - |
| 4. PWA Session Results & Receipt | 2/2 | Complete | 2026-03-14 |
| 5. Kiosk POS — Staff Operations | 0/2 | Not started | - |

**Total: 3/9 plans complete**

## Dependency Graph

```
Phase 1: Billing Cloud Sync
    |
    +---> Phase 2: Cloud Dashboard Live   ---> Phase 3: Cloud Analytics
    |
    +-- (independent of below)

Phase 4: PWA Session Results    (independent, can start any time)
Phase 5: Kiosk POS Operations   (independent, can start any time)
```

Recommended execution order given single-developer velocity:
1 -> 4 -> 5 -> 2 -> 3

Rationale: Phase 1 unblocks cloud. While cloud API builds (Phase 2), PWA and kiosk work (Phases 4/5) can run. Cloud analytics (Phase 3) is last -- needs both sync data and the UI patterns established in Phase 2.

## Infrastructure Changes (outside GSD)

| Date | Commit | Change |
|------|--------|--------|
| 2026-03-14 | cc3da21 | Billing credits migration: INR → credits, 2-tier retroactive → 3-tier non-retroactive, DB-driven billing_rates table + admin CRUD, all UI shows "cr" |
