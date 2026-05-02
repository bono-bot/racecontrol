-- PACT-20260503-003 Phase 0.2 — addresses MMA VERIFY P0 (Grok adversarial round):
-- "NEVER-DELETE on wallet_topups and wallet_redemptions not enforceable
-- structurally" → enforce via BEFORE DELETE triggers that ABORT.
--
-- Doctrine: 8-year audit retention per CGST tax law + Wallet Framing C
-- audit-trail integrity. Customer DPDP erasure is anonymization (separate
-- ledger requirement), NOT row deletion on these tables.

CREATE TRIGGER trg_wallet_topups_no_delete
BEFORE DELETE ON wallet_topups
BEGIN
    SELECT RAISE(ABORT,
        'wallet_topups is append-only audit trail (8-year retention per CGST + Wallet Framing C)');
END;

CREATE TRIGGER trg_wallet_redemptions_no_delete
BEFORE DELETE ON wallet_redemptions
BEGIN
    SELECT RAISE(ABORT,
        'wallet_redemptions is append-only audit trail (8-year retention per CGST + Wallet Framing C)');
END;
