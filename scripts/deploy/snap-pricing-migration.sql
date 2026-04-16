-- Snap Pricing Migration (2026-04-16)
-- Run on server (.23) and cloud (Bono VPS) after deploying the new binary.
-- INSERT OR IGNORE in migrate_billing.rs won't update existing rows.

-- Update 30-min tier price from 75000 (₹750) to 70000 (₹700)
UPDATE pricing_tiers SET price_paise = 70000 WHERE id = 'tier_30min' AND price_paise = 75000;

-- Verify
SELECT id, name, price_paise, billing_mode FROM pricing_tiers WHERE id IN ('tier_30min', 'tier_60min', 'tier_per_minute');
