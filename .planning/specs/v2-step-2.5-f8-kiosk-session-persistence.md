# F8 — Kiosk Session Persistence (Step 2.5 Resilience Foundation)

**Status:** SPEC-SKELETON — implementation gated on Captain explicit Step 2.5 implementation-execute verb
**Created:** 2026-05-02 (composite-ratify-event #2 substrate landing)
**Owner:** james-LEAD (per PACT-070 first-mover; bono AMPLIFIER eligible)
**Sub-sequence position:** **2nd** (F9 → F8 → F7 → F12) — F8 ships after F9 atomic-deploy infra is ready; closes acute Kiosk silent-state-loss before F7 closes Pods truth
**Ratifies:** PACT-20260502-001 quartet F7+F8+F9+F12 + CONSTRAINT-018 ACTIVE
**Substrate-anchor:** comms-link `7d86032` (composite-ratify-event #2 minimal substrate)

---

## Goal

Eliminate Kiosk silent-state-loss-on-refresh failure mode (V1 antipattern empirically observed). Every state transition in V2 Kiosk 13-step flow persists server-side within 10s of transition; refresh / crash / pod-reboot resumes from last-persisted state.

---

## Contract

**Binding:** CONSTRAINT-018 — *"Kiosk state mutations MUST persist server-side via F8 within 10s of transition."*

**Persistence shape:**
- Each state-machine node transition emits a `kiosk_session_state` record to server-side store (likely SQLite via racecontrol) within 10s window.
- Record carries: `session_id`, `pod_id`, `state_node` (PIN / ForgotPin / Register / PickGame / AcMode / AcSpSetup / Experience / Handoff / Live / Topup / EndConfirm / ConsoleFinished / Results), `state_payload` (JSON), `transition_ts_utc`, `customer_id?`, `staff_id?`.
- Resume primitive: `GET /api/v1/kiosk/session/{pod_id}/active` returns latest non-terminal state-node + payload; Kiosk on-mount checks for active session before rendering PIN entry.

**SLO:** server-side persistence write completes within 10s of client-emit (P99). Tail-latency violations counted; >1% violation rate triggers HALO probe alert.

---

## Composes-with

- **CONSTRAINT-018** (this contract; binding-text in PACT-CHARTER §V2.0)
- **F9 Atomic Deploy** (F8 client + server modules deploy together via F9; ships **after** F9 per sub-sequence)
- **F1 Connection Hub / F2 Feature Flag Service** (V2 substrate; F8 endpoints route through F1 connection layer + F2 gates rollout per pod)
- **P2 Event ledger** (architectural milestone; F8 writes are first-class events in P2 ledger when P2 lands; pre-P2 F8 writes go to dedicated table)
- **V2 Kiosk page** (`kiosk/src/app/v2/kiosk/page.tsx`) state machine — F8 client integrates as side-effect on each state transition

---

## State-node coverage matrix

| Kiosk state node | Persistence required | Resume behavior |
|------------------|----------------------|-----------------|
| PIN | NO (entry only; ephemeral) | resume → re-enter PIN |
| ForgotPin | YES (staff-side workflow state) | resume → resume staff workflow |
| Register | YES (in-progress registration form) | resume → form re-populated |
| PickGame | YES (game catalog selection) | resume → highlight prior selection |
| AcMode | YES (SP/MP choice) | resume → mode locked |
| AcSpSetup | YES (track / car / AI config) | resume → config restored |
| Experience | YES (chosen experience) | resume → experience locked |
| Handoff | YES (mid-handoff state) | resume → continue handoff or restart |
| **Live** | YES (active session — most critical) | resume → re-bind to active billing session via racecontrol |
| Topup | YES (payment in-flight) | resume → re-display invoice |
| EndConfirm | YES (end-confirm dialog state) | resume → re-display dialog |
| ConsoleFinished | YES (terminal result state) | resume → re-display result |
| Results | YES (session results dashboard) | resume → re-display results |

---

## Acute failure modes closed

- **Browser refresh during Live**: V1 loses entire client state; F2 dictates pod still racing but Kiosk shows PIN screen (state-mismatch). F8 resumes Live state on refresh.
- **Pod reboot mid-session**: V1 customer credits leak (server billing session still active; client-state gone). F8 resume re-binds to active billing session.
- **Network blip during state-transition**: V1 state-node may render before server persists; refresh during blip orphans the transition. F8 retries persistence with idempotency-key (composes with F12).

---

## Out of scope (F8 v1)

- Multi-device session resume (customer continues on PWA after Kiosk session) — V2.1+
- State-machine offline-mode (F8 v1 requires F1 connection live; offline-resilience deferred)
- Server-side state-machine validation (F8 v1 trusts client-emitted state; server-side state-progression validation is operational follow-up)

---

## Implementation gating

**Phase 1 (this commit):** spec-shape only. CONSTRAINT-018 binding-text ACTIVE. No code change in `kiosk/`.

**Phase 2 (gated on F9 ship + Captain Step-2.5-implement verb):**
- Server: `racecontrol/crates/racecontrol/src/kiosk_session.rs` — POST `/api/v1/kiosk/session/transition` endpoint + GET `/api/v1/kiosk/session/{pod_id}/active`
- Schema: `kiosk_session_state` table (idempotent migration)
- Client: `kiosk/src/app/v2/kiosk/page.tsx` — `useEffect` side-effect on state-machine transition; on-mount resume check
- HALO probe `kiosk-state-persistence-latency` (verifies 10s SLO)

**Phase 3 (gated on F8 v1 7-day soak PASS):**
- F12 idempotency-key honoring on F8 endpoints (composes with F12 ACTIVE)
- CONSTRAINT-018 enforcement flips from honor-system → HALO-driven alert

---

## NOT TESTED (post-spec-shape)

- F8 server endpoints (Phase 2 implementation)
- 13-state-node coverage matrix (Phase 2 implementation per node)
- 10s SLO under load (8 concurrent sessions)
- Resume behavior on actual pod reboot (live verification)
- F12 idempotency composition (Phase 3)

---

## Stale-at

Durable until F8 Phase 2 implementation lands OR scope re-shape via sibling-PACT.
