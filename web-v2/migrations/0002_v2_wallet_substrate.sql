-- ============================================================================
-- 0002_v2_wallet_substrate.sql · 2026-05-19
-- ============================================================================
--
-- Wallet substrate for the V2.0 5-op cluster catalogued in R-46 Pass #2 §6:
--   reconcilePending (HIGH)        — wallet idempotency replay + reconciliation
--   grantApologyCredit (HIGH)      — CD-10 apology-credit defaults flow
--   getWalletBalance (LOW read)
--   getMyWalletBalance (LOW read)  — customer-tier scoped read
--   getWalletTransactions (LOW read)
--
-- Doctrine anchors:
--   - CD-6 single-writer doctrine (heart is the only wallet writer; Bono Admin
--     proxies into Bono cloud; Kiosk/POS/PWA never write directly).
--   - CD-7 wallet-cache independence across PCs (per-PC Postgres-backed cache;
--     shared Redis question DEFERRED to V2.1+ per TWO-PC-3).
--   - §J wallet source-tag enum (13 LOCKED values, see §F INVENTORY 2026-05-18).
--     `source_tag` modeled as TEXT in V2.0; CHECK enum constraint deferred to
--     0003 once Replit `wallet.yaml` ratifies the exact strings (some §J
--     entries are templated with `[pod_id]` / `[station_id]` / `[game name]`
--     slots and need regex-class enforcement, not strict atoms).
--   - §F commercial locks: 1 ₹ = 1 credit; bonus ladder <₹2000 1:1 / ≥₹2000
--     +10% / ≥₹4000 +20%; non-stacking with session discount (separate ledger
--     entries source-tagged `bonus/10pct` or `bonus/20pct`).
--   - R-41 invariant: idempotency cache (`idempotency_keys` from 0001) is the
--     wallet replay surface; this migration does NOT add a wallet-specific
--     idempotency table — wallet ops share the global `idempotency_keys` cache
--     keyed by (operation_id, actor_scope, request_id).
--   - R-42 §4: append-only ledger; `wallet_transactions` is the canonical
--     record. `wallet_balances` is a materialized cache (single-writer per
--     CD-6; recomputable from `wallet_transactions` if ever drifts).
--
-- Forward-only ledger: migration appends one row to schema_migrations.
-- ============================================================================

BEGIN;

-- ============================================================================
-- wallet_balances: per-household current balance + optimistic-locking version
-- Single row per household. Single-writer per CD-6 (Admin proxy → heart).
-- Recomputable from wallet_transactions if cache drifts (recovery oracle).
-- ============================================================================

CREATE TABLE wallet_balances (
    household_id     UUID PRIMARY KEY REFERENCES households(household_id) ON DELETE RESTRICT,
    balance_cents    BIGINT NOT NULL DEFAULT 0,
    version          BIGINT NOT NULL DEFAULT 0,
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT wallet_balances_nonnegative CHECK (balance_cents >= 0)
);

CREATE INDEX wallet_balances_updated_idx ON wallet_balances (updated_at DESC);

-- ============================================================================
-- wallet_transactions: append-only ledger.
-- amount_cents is signed: top-ups positive, session debits + auto-bill negative.
-- balance_after_cents is the post-application snapshot (audit reconstructability
-- without replay — caller computes and writes atomically in same txn).
-- operation_id + request_id soft-link to idempotency_keys (no FK because the
-- idempotency entry expires after 24h; tx ledger is permanent).
-- ============================================================================

CREATE TABLE wallet_transactions (
    id                    BIGSERIAL PRIMARY KEY,
    household_id          UUID NOT NULL REFERENCES households(household_id) ON DELETE RESTRICT,
    ts                    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    amount_cents          BIGINT NOT NULL,
    balance_after_cents   BIGINT NOT NULL,
    txn_type              TEXT NOT NULL,
    source_tag            TEXT NOT NULL,
    operation_id          TEXT,
    request_id            TEXT,
    actor_id              TEXT NOT NULL,
    actor_tier            TEXT NOT NULL,
    detail                JSONB,
    CONSTRAINT wallet_transactions_txn_type_check
        CHECK (txn_type IN ('topup', 'bonus', 'session_debit', 'cafe_debit', 'apology_credit', 'auto_bill_close', 'reversal')),
    CONSTRAINT wallet_transactions_actor_tier_check
        CHECK (actor_tier IN ('customer', 'staff', 'shift_lead', 'captain', 'system')),
    CONSTRAINT wallet_transactions_balance_after_nonnegative
        CHECK (balance_after_cents >= 0),
    -- topups + bonuses + apology credits MUST be positive; debits + reversal
    -- direction enforced by application layer (reversal can go either way).
    CONSTRAINT wallet_transactions_amount_sign_check
        CHECK (
            (txn_type IN ('topup', 'bonus', 'apology_credit') AND amount_cents > 0)
            OR (txn_type IN ('session_debit', 'cafe_debit', 'auto_bill_close') AND amount_cents < 0)
            OR (txn_type = 'reversal')
        )
);

CREATE INDEX wallet_transactions_household_ts_idx ON wallet_transactions (household_id, ts DESC);
CREATE INDEX wallet_transactions_operation_idx ON wallet_transactions (operation_id) WHERE operation_id IS NOT NULL;
CREATE INDEX wallet_transactions_source_tag_idx ON wallet_transactions (source_tag);
CREATE INDEX wallet_transactions_ts_idx ON wallet_transactions (ts DESC);

-- Append-only enforcement: deny UPDATE + DELETE at the table level.
-- Reversals are NEW rows with txn_type='reversal' referencing the original via detail->>'reverses_id'.
CREATE OR REPLACE FUNCTION wallet_transactions_deny_mutation()
RETURNS TRIGGER AS $$
BEGIN
    RAISE EXCEPTION 'wallet_transactions is append-only (R-42 §4 invariant); use txn_type=reversal for corrections';
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER wallet_transactions_no_update
    BEFORE UPDATE ON wallet_transactions
    FOR EACH ROW EXECUTE FUNCTION wallet_transactions_deny_mutation();

CREATE TRIGGER wallet_transactions_no_delete
    BEFORE DELETE ON wallet_transactions
    FOR EACH ROW EXECUTE FUNCTION wallet_transactions_deny_mutation();

-- ============================================================================
-- Migration ledger entry
-- ============================================================================

INSERT INTO schema_migrations (version) VALUES ('0002_v2_wallet_substrate');

COMMIT;
