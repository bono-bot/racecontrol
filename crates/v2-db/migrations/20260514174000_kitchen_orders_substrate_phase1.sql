-- Row 1.12 Kitchen Orders V2 substrate — Phase 1 of N
--
-- V2 doctrine alignment: comms-link/v2-skeleton/05-definition-of-done.md
--   §1.5 L93:  "14:42 IST — Customer wants food. Two paths (Captain-locked
--               under R1-C split — POS .130 does NOT do cafe orders in v2.0;
--               cafe orders are PWA-self or Kiosk-staff only)"
--   §1.5 L95:  "Path A — PWA self-order: customer orders via PWA → wallet
--               debit → kitchen notification → chef tablet"
--   §1.5 L96:  "Path B — staff via Kiosk .23: Kiosk surfaces a cafe-order
--               flow at the same surface where staff launch games..."
--   §1.5 L98:  "cafe orders only go through Kiosk so the queue has one
--               source of truth, not two"
--   §3.3 L342-356: wallet.source_tag enumeration locked v2.0 — including
--               `cafe / PWA` and `cafe / Kiosk` (NO `cafe / POS` per R1-C
--               lock; source enum encoded as `cafe/PWA` and `cafe/Kiosk`
--               in this schema, matching spec test §3 (1) routing-state
--               mechanism + cafe-order-routing.spec.ts contract source enum)
--   §1.5 L104: "Wallet auto-tallies session minutes × rate + cafe-order
--               amount" (cafe-order amount feeds auto-bill — Layer 1.13
--               composition gates on this row's session_id linkage)
--   §3 L328:   "Kitchen notification: Chef sees orders entered via PWA or
--               Kiosk on a single tablet in the kitchen. POS .130 does NOT
--               enter cafe orders in v2.0" (Captain-locked)
--
-- Spec test: tests/contract/cafe-order-routing.spec.ts (commit f81a3a28
--   §S-217 iter3 cascade). 5 tests covering canonical-singular shape +
--   source enum + idempotency + auto-bill composition + cross-surface ≤2s.
--   Phase 1 closes F1 G-F1-3 (table shape exists). Phase 2 deferred —
--   endpoint registrations (POST /api/v1/cafe/order + GET /api/v1/cafe/
--   kitchen-queue), source-enum validation handler, idempotency-replay
--   return, auto-bill composition wire-up to Layer 1.13.
--
-- §S-146 V1↔V2 RCA: .planning/audits/RCA-2026-05-14-row-1.12-kitchen-orders-substrate.md
--   foundational boundary class (DB schema, additive only — new table; no
--   existing-table mutation). Captain pre-commit 2026-05-14 ~23:08 IST
--   verbatim "I authorize you to Proceed with your recommendation that is
--   aligned with Racing Point ecosystem v2 development. Proceed autonomously"
--   covers row 1.12 substrate per apply-recommendations-autonomously
--   doctrine + §S-339 precedent (Walk-In Fallback Phase 1 substrate landed
--   under identical autonomous-eligibility class 2026-05-14 ~20:43 IST
--   commits 22df4bfb + a97ac4fb).
--
-- §S-340 close-anchor pending atomic Bash-chain append to comms-link
-- V2-MASTER-STATE post-push (per sn-namespace-collision-via-background-
-- partner-sync N=2-ACTIVE structural-fix discipline).

-- ─── §1 Create kitchen_orders table ─────────────────────────────────────
-- Source-tagged (cafe/PWA or cafe/Kiosk — no cafe/POS per R1-C lock).
--
-- DPDP §12 aggregate-only: links via session_id (which carries customer
-- linkage through the sessions table); direct customer_id NOT stored here
-- to keep the kitchen tablet rendering aggregate-only — chef does not need
-- PII to fulfill orders. Customer linkage retrievable via JOIN sessions.
--
-- Idempotency: client-supplied UUID prevents network-retry double-create
-- per spec test §3 routing-state mechanism item (3) — wallet billing_start
-- pattern at billing_start_validate.rs:28-156 mirrored at the cafe boundary.
--
-- State machine: pending → preparing → ready (or → cancelled). Phase 1
-- creates the schema only; transition logic is Phase 2 handler-class scope.
CREATE TABLE IF NOT EXISTS kitchen_orders (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL REFERENCES sessions(id),
    source TEXT NOT NULL CHECK (source IN ('cafe/PWA', 'cafe/Kiosk')),
    items_json TEXT NOT NULL,
    total_paise INTEGER NOT NULL CHECK (total_paise >= 0),
    idempotency_key TEXT NOT NULL,
    placed_at_utc TEXT NOT NULL,
    fulfilled_at_utc TEXT,
    state TEXT NOT NULL DEFAULT 'pending'
        CHECK (state IN ('pending', 'preparing', 'ready', 'cancelled'))
);

-- ─── §2 UNIQUE constraint on idempotency_key ───────────────────────────
-- Prevents network-retry double-create per spec test §3 (3). UNIQUE on a
-- separate index (not inline column constraint) allows future migration to
-- add scope qualification (e.g. UNIQUE(idempotency_key, session_id))
-- without the recreate-table dance that NF-james-4 surfaced 2026-05-08
-- (PRAGMA legacy_alter_table=0 FK-rewrite class).
CREATE UNIQUE INDEX IF NOT EXISTS idx_kitchen_orders_idempotency
    ON kitchen_orders(idempotency_key);

-- ─── §3 Index for kitchen-tablet active-queue pull ─────────────────────
-- Chef tablet pulls rows in {pending, preparing} state ordered by
-- placed_at_utc. Partial index on the rare-active-state case (most rows
-- transition to ready/cancelled quickly) keeps the index minimal.
--
-- Requires SQLite >= 3.8.9 (partial index support). sqlx 0.8.x bundles a
-- modern SQLite (3.45+); current toolchain satisfies. Same SQLite-version
-- dependency as §S-339 row 1.15 partial index at customers.discount_ineligible
-- (migration 20260514151300 §4) — F1 MAOR finding pattern noted.
CREATE INDEX IF NOT EXISTS idx_kitchen_orders_active_queue
    ON kitchen_orders(state, placed_at_utc)
    WHERE state IN ('pending', 'preparing');

-- ─── §4 Index for session-scoped queries (Layer 1.13 auto-bill compose) ─
-- Layer 1.13 auto-bill SELECTs cafe-order total per active session by
-- JOIN on session_id. Standard B-tree index for FK lookups.
CREATE INDEX IF NOT EXISTS idx_kitchen_orders_session_id
    ON kitchen_orders(session_id);
