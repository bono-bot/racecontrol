# F12 — Idempotency Layer (Step 2.5 Resilience Foundation)

**Status:** SPEC-SKELETON — implementation gated on Captain explicit Step 2.5 implementation-execute verb
**Created:** 2026-05-02 (composite-ratify-event #2 substrate landing)
**Owner:** james-LEAD (per PACT-070 first-mover; bono AMPLIFIER eligible)
**Sub-sequence position:** **4th / last** (F9 → F8 → F7 → F12) — F12 builds on F9 atomic-deploy + F7/F8 stability primitives; financial-irreversibility class
**Ratifies:** PACT-20260502-001 quartet F7+F8+F9+F12 + CONSTRAINT-020 ACTIVE
**Substrate-anchor:** comms-link `7d86032` (composite-ratify-event #2 minimal substrate)

**Promotion path:** F12 originally CANDIDATE-N1 sibling-class to F10/F11/F13/F14/F15. Bono close-scope C1 caveat surfaced financial-irreversibility-class warrants different threshold than infra-degradation N=2-default. Close-scope ABSORB → F12 promoted into QUARTET at composite-ratify-event #2.

---

## Goal

Eliminate double-charge / duplicate-mutation risk on transactional mutating endpoints (wallet / customers / sessions / payments). Every mutating endpoint accepts and honors `X-Idempotency-Key` header per F12 spec; replay with same key returns prior result without re-mutation.

---

## Contract

**Binding:** CONSTRAINT-020 — *"Mutating API endpoints (POST/PUT/DELETE on transactional resources — wallet/customers/sessions/payments) MUST accept and honor `X-Idempotency-Key` header per F12 spec. Razorpay integration PR REQUIRES F12 ACTIVE before merge."*

**Header semantics:**
- Client emits `X-Idempotency-Key: <UUID-v4>` on mutating request.
- Server checks `idempotency_keys` table (key, request_hash, response_payload, ts, ttl_24h).
- First-emit: server processes mutation, persists response, returns 200/201.
- Replay (same key, same request_hash): server returns prior response_payload without re-mutating.
- Replay (same key, DIFFERENT request_hash): server returns 422 Idempotency-Conflict (client error — same key reused for semantically-different request).
- TTL: 24h (records auto-expire; 24h covers most retry scenarios + payment-gateway settlement windows).

---

## Endpoint coverage matrix (F12 v1 scope)

| Resource | Endpoint | Mutating? | F12-required |
|----------|----------|-----------|--------------|
| Wallet | POST `/api/v1/wallet/topup` | YES | ✅ MUST |
| Wallet | POST `/api/v1/wallet/debit` | YES | ✅ MUST |
| Wallet | POST `/api/v1/wallet/refund` | YES | ✅ MUST |
| Customers | POST `/api/v1/customers` (registration) | YES | ✅ MUST |
| Customers | PUT `/api/v1/customers/{id}` | YES | ✅ MUST |
| Customers | DELETE `/api/v1/customers/{id}` (DPDP-erase) | YES | ✅ MUST |
| Sessions | POST `/api/v1/billing/start` | YES | ✅ MUST |
| Sessions | POST `/api/v1/billing/end` | YES | ✅ MUST |
| Sessions | POST `/api/v1/games/launch` | YES | ✅ MUST |
| Sessions | POST `/api/v1/games/stop` | YES | ✅ MUST |
| Payments | POST `/api/v1/payments/razorpay/charge` | YES | ✅ MUST + Razorpay PR gate |
| Payments | POST `/api/v1/payments/razorpay/refund` | YES | ✅ MUST + Razorpay PR gate |
| Payments | POST `/api/v1/payments/upi/initiate` | YES | ✅ MUST |

**Out of scope (read-only):** all GET endpoints. F12 v1 is mutation-only.

---

## Razorpay PR gate (CONSTRAINT-020)

**Gate ACTIVE immediately at composite-ratify-event #2** (pre-F12-ship).

- Razorpay integration PR cannot merge until F12 status=ACTIVE-ENFORCEMENT (post-F12-ship + 7d soak).
- Pre-F12 Razorpay PR-merge attempt = CONSTRAINT-020 violation.
- Post-F12 Razorpay PR adds CI gate `f12-active-required` (GitHub Actions check) that blocks merge until `idempotency_keys` table exists + endpoint coverage HALO probe PASS.

**Rationale:** Razorpay charges are not idempotent at gateway-layer; double-charge = customer-money loss without recourse. Financial-irreversibility class warrants pre-emptive PR gate (different threshold from N=2-default infra-degradation).

---

## Composes-with

- **CONSTRAINT-020** (this contract; binding-text in PACT-CHARTER §V2.0)
- **F9 Atomic Deploy** (F12 server middleware ships via F9; PR gate ships via F9 GitHub Actions workflow)
- **F8 Kiosk Session Persistence** (F8 retries persistence with idempotency-key when F12 ACTIVE; pre-F12 F8 retries are honor-system)
- **F7 Pod Resilience** (F7 emits heartbeats with idempotency-key when F12 ACTIVE for replay-safety on transient WS reconnects)
- **F1 Connection Hub / F2 Feature Flag Service** (V2 substrate; F12 middleware composes with F1 connection layer; F2 gates per-endpoint rollout)
- **GDPR / DPDP erase contract** (Standing Rule — `customer_data_delete()`); F12 ensures replay-safety on erase requests
- **CGP H3 PERMANENCE GATE** — F12 fails-CLOSED on duplicate-mutation; survives redeploy because middleware is source-code

---

## Failure modes closed

- **Razorpay double-charge on retry**: V1 has no idempotency layer; client retry on flaky network = duplicate charge. F12 + Razorpay-side idempotency-key composition closes this.
- **Wallet topup retry duplicates credit**: V1 customer retries failed topup → server processes both → customer over-credited. F12 returns prior response without re-crediting.
- **Session-start replay races**: V1 simultaneous session-start requests race; both succeed → customer billed twice. F12 + same idempotency-key returns single response.
- **DPDP-erase replay**: V1 DPDP-erase retry returns 404 (already deleted) instead of original success. F12 replay returns original response payload.

---

## Out of scope (F12 v1)

- Cross-server idempotency (F12 v1 is single-server scope; multi-server replication out of scope)
- Cross-process idempotency (F12 v1 is per-server-process; cross-process via shared `idempotency_keys` table is implicit but not explicitly tested)
- Auto-retry on client side (F12 v1 honors client emit; client-side retry layer is separate concern)
- Idempotency for non-HTTP transports (WS message idempotency uses different key shape; v1 HTTP-only)

---

## Implementation gating

**Phase 1 (this commit):** spec-shape only. CONSTRAINT-020 binding-text ACTIVE. **Razorpay PR gate ACTIVE immediately** (CI shape lands at F9 Phase 2 substrate). No code change in `crates/` middleware.

**Phase 2 (gated on F7 ship + Captain Step-2.5-implement verb):**
- Server: `racecontrol/crates/racecontrol/src/idempotency.rs` — Axum middleware layer
- Schema: `idempotency_keys` table (idempotent migration; UNIQUE(key) + index on ts for TTL cleanup)
- Per-endpoint integration on coverage matrix (13 endpoints listed above)
- HALO probe `idempotency-key-coverage` (verifies all transactional mutating endpoints honor header)
- GitHub Actions check `f12-active-required` (blocks Razorpay PR until F12 status=ACTIVE-ENFORCEMENT)

**Phase 3 (gated on F12 v1 7-day soak PASS):**
- CONSTRAINT-020 enforcement flips: HALO probe → fail-CLOSED middleware (mutation rejected without `X-Idempotency-Key` header on covered endpoints)
- Razorpay PR-merge gate becomes mechanical (status check required)

---

## NOT TESTED (post-spec-shape)

- F12 middleware (Phase 2 implementation)
- 13-endpoint coverage matrix (Phase 2 implementation per endpoint)
- Replay correctness under load
- TTL cleanup at 24h boundary
- Razorpay PR CI gate (Phase 2 substrate)
- Razorpay-side idempotency-key composition (post-F12-ship integration)
- Compose with F8 retry-with-idempotency-key (Phase 3)
- Cross-server replication scenarios (out of scope v1)

---

## Stale-at

Durable until F12 Phase 2 implementation lands OR scope re-shape via sibling-PACT.
