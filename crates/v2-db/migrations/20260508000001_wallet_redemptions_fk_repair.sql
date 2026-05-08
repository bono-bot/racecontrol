-- V2 Wave 1 W1-S2 NF-james-4 — wallet_redemptions FK repair.
--
-- Surfaced 2026-05-08 ~21:30 IST during Session 2 W1-S2 first behavior-consumer
-- (WalletService::reconcile_redemption test). PHASE-1-WAVE-1-PLAN.md §12 sibling.
--
-- Bug: migration 20260503000003 (PACT-018 staff_id FK) used the recreate-table
-- pattern on `sessions`:
--     ALTER TABLE sessions RENAME TO sessions_old_pact018;
--     CREATE TABLE sessions (...);
--     INSERT INTO sessions SELECT * FROM sessions_old_pact018;
--     DROP TABLE sessions_old_pact018;
--
-- With `PRAGMA legacy_alter_table=0` (SQLite default since 3.25), `ALTER TABLE
-- RENAME` rewrites foreign key references in OTHER tables that point at the
-- renamed table. So `wallet_redemptions.session_id REFERENCES sessions(id)`
-- (created in migration 001) was rewritten to
-- `REFERENCES sessions_old_pact018(id)` during the migration 003 rename.
--
-- Migration 003 then DROPs sessions_old_pact018 (line 149), leaving
-- wallet_redemptions FK dangling. Any INSERT into wallet_redemptions trips
-- "no such table: main.sessions_old_pact018".
--
-- Migration 003 applied the recreate-table fix to `wallet_topups` (lines
-- 164-198) but missed `wallet_redemptions` — likely because migration 003
-- predated any wallet_redemptions consumer (Wave 1 W1-S2 is the first;
-- production wallet_redemptions has zero rows at this migration's runtime).
--
-- Fix: same recreate-table pattern, now applied to wallet_redemptions.
-- Empty-table-safe (V2 has no live data flow yet per V2-MASTER-STATE §S-117
-- live-data-flow axis = 0.75/15 substrate-only).

-- Drop trigger first (recreated post-swap to preserve NEVER-DELETE invariant
-- from migration 20260503000002).
DROP TRIGGER trg_wallet_redemptions_no_delete;

ALTER TABLE wallet_redemptions RENAME TO wallet_redemptions_old_w1s2;

CREATE TABLE wallet_redemptions (
    id                  TEXT PRIMARY KEY NOT NULL,
    wallet_id           TEXT NOT NULL
                        REFERENCES wallets(id) ON DELETE RESTRICT,
    session_id          TEXT NOT NULL
                        REFERENCES sessions(id) ON DELETE RESTRICT,
    credits_redeemed    INTEGER NOT NULL CHECK (credits_redeemed > 0),
    redeemed_for        TEXT NOT NULL
                        CHECK (redeemed_for IN ('sim','ps5')),
    created_at          TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

INSERT INTO wallet_redemptions (id, wallet_id, session_id, credits_redeemed, redeemed_for, created_at)
SELECT id, wallet_id, session_id, credits_redeemed, redeemed_for, created_at
FROM wallet_redemptions_old_w1s2;

DROP TABLE wallet_redemptions_old_w1s2;

-- Recreate indexes (verbatim from migration 20260503000001).
CREATE INDEX idx_redemptions_wallet ON wallet_redemptions(wallet_id);
CREATE INDEX idx_redemptions_session ON wallet_redemptions(session_id);

-- Recreate the BEFORE DELETE trigger (verbatim from migration
-- 20260503000002 — preserve MMA VERIFY P0 NEVER-DELETE invariant + 8-year
-- audit retention per CGST + Wallet Framing C).
CREATE TRIGGER trg_wallet_redemptions_no_delete
BEFORE DELETE ON wallet_redemptions
BEGIN
    SELECT RAISE(ABORT,
        'wallet_redemptions is append-only audit trail (8-year retention per CGST + Wallet Framing C)');
END;
