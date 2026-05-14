-- Row 1.15 Walk-In Fallback substrate — Phase 1 of N
--
-- V2 doctrine alignment: v2-skeleton/05-definition-of-done.md
--   §3 line 314 Joint 1: "Two named fallback customer accounts on substrate,
--     flagged discount_ineligible: true, for extreme-edge no-phone walk-ins."
--   §3.3 line 341: "customer record . discount_ineligible: bool — true for
--     the two named fallback accounts (Walk-In Guest 1, Walk-In Guest 2);
--     false for all real customer profiles."
--   line 775: "Named fallback accounts: 2 — Walk-In Guest 1 and
--     Walk-In Guest 2; persist forever in audit; excluded from public
--     leaderboards; cannot be claimed/merged."
--
-- Phase 1 scope: enable C1 (existence) + C2 (discount_ineligible=true flag)
--   acceptance test contracts of tests/contract/walkin-guest-discount-ineligible.spec.ts.
--   C3 (billable) + C4 (no bonus ladder) + C5 (source_tag) gated on follow-on PRs.
--
-- §S-146 V1↔V2 RCA: .planning/audits/RCA-2026-05-14-row-1.15-walkin-guest-substrate.md
--   foundational boundary class (DB schema + wallet/billing). Captain pre-commit
--   exception 2026-05-14 ~17:52 IST "complete TEST-SCAFFOLDED with MAOR" covers
--   row 1.15 (enumerated in V2-PROGRESS-MAP §0.2 forward-delta TEST-SCAFFOLDED
--   batch at time of pre-commitment).

-- ─── §1 Add discount_ineligible column to customers ────────────────────────
-- SQLite ALTER TABLE ADD COLUMN is idempotent only via try/catch; sqlx::migrate
-- runs each .sql file once on a checksum-tracked basis. Default 0 means all
-- existing customers are correctly flagged not-ineligible (= eligible for
-- discounts) per DoD §3.3 line 341.
ALTER TABLE customers ADD COLUMN discount_ineligible INTEGER NOT NULL DEFAULT 0
    CHECK (discount_ineligible IN (0, 1));

-- ─── §2 Seed the 2 named Walk-In Guest accounts (DoD line 775) ─────────────
-- INSERT OR IGNORE: idempotent — re-runs are no-op. Customer IDs aligned with
-- cirs_lookup.rs:427-438 synthesize_walkin_preview() which returns
-- customer_id = format!("walk_in_guest_{guest_id}"). This alignment means the
-- synthetic preview's customer_id resolves to a real DB row for downstream
-- billing/wallet operations (Phase 2 unblocks C3 billable + C5 source_tag).
--
-- Synthetic phone values (+0000000001/+0000000002) satisfy customers.phone
-- UNIQUE constraint without colliding with any real customer phone (real
-- Indian phones start with +91 and follow 10-digit national pattern).
-- DPDP consent flags left at default (registered_at_pwa=0, no tnc/dpdp ts) —
-- walk-in guests are by definition no-phone fallback paths, not registered.
INSERT OR IGNORE INTO customers (id, phone, display_name, driver_class, registered_at_pwa, discount_ineligible)
VALUES
  ('walk_in_guest_1', '+0000000001', 'Walk-In Guest 1', 'rookie', 0, 1),
  ('walk_in_guest_2', '+0000000002', 'Walk-In Guest 2', 'rookie', 0, 1);

-- ─── §3 Seed the 2 wallets for Walk-In Guest accounts ──────────────────────
-- Wallet rows must exist for billing/top-up endpoints to write ledger entries.
-- balance_credits=0 default (no pre-funded credit). breakage_recognized_at NULL
-- since walk-in accounts are never claimed/merged per DoD line 775.
INSERT OR IGNORE INTO wallets (id, customer_id, balance_credits)
VALUES
  ('wallet_walk_in_guest_1', 'walk_in_guest_1', 0),
  ('wallet_walk_in_guest_2', 'walk_in_guest_2', 0);

-- ─── §4 Index for fast discount_ineligible filtering ──────────────────────
-- Anti-pattern guard: leaderboard queries + bonus-ladder computation should
-- be able to filter discount_ineligible customers cheaply. Partial index on
-- the rare-true case (only 2 rows today; bounded growth) keeps the index
-- minimal.
--
-- Requires SQLite >= 3.8.9 (partial index support). sqlx 0.8.x bundles a
-- modern SQLite (3.45+); current toolchain satisfies. If future SQLite
-- downgrade is contemplated, replace partial index with full index.
CREATE INDEX IF NOT EXISTS idx_customers_discount_ineligible
    ON customers(discount_ineligible)
    WHERE discount_ineligible = 1;
