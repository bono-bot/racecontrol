# Requirements: RaceControl Billing & POS (v3.0)

**Total:** 25 requirements across 5 categories
**Status:** All pending

---

## Category 1: Billing Cloud Sync (SYNC)

| ID | Requirement | Phase | Status |
|----|-------------|-------|--------|
| SYNC-01 | Completed `billing_sessions` rows are pushed to cloud within 60 seconds of `status = Completed \| EndedEarly` | 1 | [x] |
| SYNC-02 | `wallet_transactions` rows are pushed to cloud within 60 seconds of creation | 1 | [x] |
| SYNC-03 | `billing_events` rows are pushed to cloud within 60 seconds of session end | 1 | [x] |
| SYNC-04 | Cloud copy is read-only — no cloud→venue push for billing tables (venue stays authoritative) | 1 | [x] |
| SYNC-05 | Sync survives venue network outage — queues and retries on reconnect, no data loss | 1 | [x] |

---

## Category 2: Cloud Dashboard — Live Operations (LIVE)

| ID | Requirement | Phase | Status |
|----|-------------|-------|--------|
| LIVE-01 | Dashboard shows all 8 pods in a live grid: pod number, customer name, status (idle/active/connecting), time remaining | 2 | [ ] |
| LIVE-02 | Dashboard shows today's total revenue (₹), active session count, and total pods in use | 2 | [ ] |
| LIVE-03 | Pod grid and revenue ticker update within 60 seconds of any session change (start, end, pause) | 2 | [ ] |
| LIVE-04 | Dashboard highlights any pod whose active session has ≤ 5 minutes remaining | 2 | [ ] |
| LIVE-05 | Dashboard is accessible at dashboard.racingpoint.cloud — no VPN or local network required | 2 | [ ] |

---

## Category 3: Cloud Dashboard — Analytics & Reports (ANA)

| ID | Requirement | Phase | Status |
|----|-------------|-------|--------|
| ANA-01 | Staff can view a session list filtered by date range, pod, pricing tier, and status | 3 | [ ] |
| ANA-02 | Dashboard shows a daily revenue bar chart for the last 30 days (total ₹ per day) | 3 | [ ] |
| ANA-03 | Dashboard shows revenue breakdown by pricing tier (e.g. 30 min vs 1 hr vs trial) as a chart | 3 | [ ] |
| ANA-04 | Dashboard shows a utilization heatmap (hour-of-day × day-of-week, % pods in use) | 3 | [ ] |
| ANA-05 | Staff can export the filtered session list to CSV with all billing fields | 3 | [ ] |

---

## Category 4: PWA Customer Experience (PWA)

| ID | Requirement | Phase | Status |
|----|-------------|-------|--------|
| PWA-01 | Customer can view session cost breakdown in session detail: base price, discount applied, final amount charged, any refund | 4 | [ ] |
| PWA-02 | Customer can view session performance in session detail: total laps, best lap time, top speed (where telemetry available) | 4 | [ ] |
| PWA-03 | Customer can view session timeline in session detail: session start, each pause/resume with reason, warnings, session end | 4 | [ ] |
| PWA-04 | Customer receives a WhatsApp message after session end summarising duration, cost, best lap, and wallet balance remaining | 4 | [ ] |
| PWA-05 | Session results page has a shareable link (no auth required to view public summary) | 4 | [ ] |

---

## Category 5: Kiosk POS — Staff Operations (POS)

| ID | Requirement | Phase | Status |
|----|-------------|-------|--------|
| POS-01 | Staff can record payment method (cash / UPI / card / wallet) for a session from the kiosk — stored in `billing_sessions` | 5 | [ ] |
| POS-02 | Staff can issue a full or partial refund for any completed session from the kiosk, with a mandatory reason code | 5 | [ ] |
| POS-03 | Refund is applied to customer wallet immediately and appears in `wallet_transactions` with `txn_type = refund_session` | 5 | [ ] |
| POS-04 | Staff can apply a one-time discount (fixed ₹ or percentage) before session start, with a mandatory reason | 5 | [ ] |
| POS-05 | All refunds and discounts are recorded in `audit_log` with the acting `staff_id` | 5 | [ ] |

---

## Traceability Matrix

| Phase | Requirements |
|-------|-------------|
| Phase 1: Billing Sync | SYNC-01, SYNC-02, SYNC-03, SYNC-04, SYNC-05 |
| Phase 2: Cloud Live Dashboard | LIVE-01, LIVE-02, LIVE-03, LIVE-04, LIVE-05 |
| Phase 3: Cloud Analytics | ANA-01, ANA-02, ANA-03, ANA-04, ANA-05 |
| Phase 4: PWA Session Results | PWA-01, PWA-02, PWA-03, PWA-04, PWA-05 |
| Phase 5: Kiosk POS Operations | POS-01, POS-02, POS-03, POS-04, POS-05 |
