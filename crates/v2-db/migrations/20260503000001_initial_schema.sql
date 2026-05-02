-- PACT-20260503-003 Phase 0.2 initial schema.
-- Applies to: SQLite (foundation). On Postgres pivot the same DDL ports with
-- minor adjustments documented in POSTGRES-PIVOT.md §Schema-portability.
--
-- Conventions:
--   - UUID columns stored as TEXT (SQLite has no native UUID; Postgres-pivot
--     swaps to native UUID with a single `ALTER COLUMN ... TYPE uuid USING ...`)
--   - Timestamps stored as TEXT in RFC3339 (SQLite ISO-8601). Sortable lexically.
--   - Money in INTEGER paise. Credits in INTEGER count. NO floats.
--   - foreign_keys=ON pragma is set in lib.rs::open(); enforced at query time.

-- ─── customers ─────────────────────────────────────────────────────────────
CREATE TABLE customers (
    id                   TEXT PRIMARY KEY NOT NULL,
    phone                TEXT NOT NULL UNIQUE,
    display_name         TEXT,
    email                TEXT,
    driver_class         TEXT NOT NULL DEFAULT 'rookie'
                         CHECK (driver_class IN ('rookie','apex','podium','champion')),
    registered_at_pwa    INTEGER NOT NULL DEFAULT 0,
    tnc_accepted_at      TEXT,
    dpdp_consent_at      TEXT,
    created_at           TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at           TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX idx_customers_phone ON customers(phone);

-- ─── wallets ───────────────────────────────────────────────────────────────
-- Single bucket per customer. balance_credits is the running net.
-- The audit trail (topups + redemptions) is the source of truth for
-- reconciliation; balance_credits is a denormalised cache. SUM(topups) -
-- SUM(redemptions) MUST equal balance_credits at all times (CHECK enforced
-- via trigger TBD in Phase 0.2.1; not in this migration to keep it minimal).
CREATE TABLE wallets (
    id                       TEXT PRIMARY KEY NOT NULL,
    customer_id              TEXT NOT NULL UNIQUE
                             REFERENCES customers(id) ON DELETE RESTRICT,
    balance_credits          INTEGER NOT NULL DEFAULT 0
                             CHECK (balance_credits >= 0),
    last_activity_at         TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    breakage_recognized_at   TEXT,
    created_at               TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at               TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

-- ─── pods ──────────────────────────────────────────────────────────────────
-- Pod IDs are TEXT (e.g. 'pod-1', 'ps5-1') matching the painted door label.
CREATE TABLE pods (
    id                TEXT PRIMARY KEY NOT NULL,
    display_name      TEXT NOT NULL,
    hardware_class    TEXT NOT NULL
                      CHECK (hardware_class IN ('sim_pod','ps5_station')),
    ip_address        TEXT,
    status            TEXT NOT NULL DEFAULT 'active'
                      CHECK (status IN ('active','maintenance','offline')),
    updated_at        TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

-- ─── lobbies ───────────────────────────────────────────────────────────────
CREATE TABLE lobbies (
    id                  TEXT PRIMARY KEY NOT NULL,
    host_customer_id    TEXT NOT NULL
                        REFERENCES customers(id) ON DELETE RESTRICT,
    pod_count           INTEGER NOT NULL CHECK (pod_count >= 1),
    billing_mode        TEXT NOT NULL
                        CHECK (billing_mode IN ('split_equal','host_pays','per_session')),
    started_at          TEXT NOT NULL,
    ended_at            TEXT,
    created_at          TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX idx_lobbies_host ON lobbies(host_customer_id);

-- ─── sessions ──────────────────────────────────────────────────────────────
-- Per CR-6 pod-is-unit-of-session: one row per (customer × pod × time-window).
-- An MP lobby produces N session rows, one per pod, all sharing lobby_id.
CREATE TABLE sessions (
    id                  TEXT PRIMARY KEY NOT NULL,
    customer_id         TEXT NOT NULL
                        REFERENCES customers(id) ON DELETE RESTRICT,
    pod_id              TEXT NOT NULL
                        REFERENCES pods(id) ON DELETE RESTRICT,
    lobby_id            TEXT
                        REFERENCES lobbies(id) ON DELETE SET NULL,
    session_type        TEXT NOT NULL
                        CHECK (session_type IN ('solo','mp_lobby_member','ps5')),
    started_at          TEXT NOT NULL,
    ended_at            TEXT,
    billing_state       TEXT NOT NULL DEFAULT 'active'
                        CHECK (billing_state IN (
                            'active','blanking_grace',
                            'ended_normal','ended_low_balance',
                            'ended_hardware_fail','ended_game_crash',
                            'ended_staff_cancelled'
                        )),
    credits_consumed    INTEGER NOT NULL DEFAULT 0
                        CHECK (credits_consumed >= 0),
    staff_id            TEXT NOT NULL,
    created_at          TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at          TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX idx_sessions_customer ON sessions(customer_id);
CREATE INDEX idx_sessions_pod ON sessions(pod_id);
CREATE INDEX idx_sessions_lobby ON sessions(lobby_id) WHERE lobby_id IS NOT NULL;
CREATE INDEX idx_sessions_active ON sessions(billing_state) WHERE billing_state IN ('active','blanking_grace');

-- ─── wallet_topups (audit trail, 8-year retention per tax law) ─────────────
-- NEVER delete rows. Constitutes the tax-invoice ledger required by CGST.
CREATE TABLE wallet_topups (
    id                    TEXT PRIMARY KEY NOT NULL,
    wallet_id             TEXT NOT NULL
                          REFERENCES wallets(id) ON DELETE RESTRICT,
    credits_purchased     INTEGER NOT NULL CHECK (credits_purchased > 0),
    gst_collected_paise   INTEGER NOT NULL CHECK (gst_collected_paise >= 0),
    amount_paid_paise     INTEGER NOT NULL CHECK (amount_paid_paise > 0),
    gst_rate_bps          INTEGER NOT NULL DEFAULT 1800
                          CHECK (gst_rate_bps = 1800), -- Framing C locks 18%
    payment_method        TEXT NOT NULL
                          CHECK (payment_method IN ('cash','upi','card')),
    payment_ref           TEXT,
    staff_id              TEXT NOT NULL,
    pos_id                TEXT NOT NULL,
    tax_invoice_no        TEXT NOT NULL UNIQUE,
    created_at            TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX idx_topups_wallet ON wallet_topups(wallet_id);
CREATE INDEX idx_topups_staff ON wallet_topups(staff_id);

-- ─── wallet_redemptions (audit trail, 8-year retention) ────────────────────
-- NEVER delete rows. Source-of-truth for the per-session redemption ledger.
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
CREATE INDEX idx_redemptions_wallet ON wallet_redemptions(wallet_id);
CREATE INDEX idx_redemptions_session ON wallet_redemptions(session_id);
