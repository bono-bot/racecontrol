-- 0001_v2_skeleton.sql
--
-- V2 skeleton migration for `racingpoint_v2` Postgres database.
--
-- Scope (this migration):
--   - schema_migrations tracking table (forward-only ledger)
--   - households (V2 identity per §B-16: phone is credential, not FK)
--   - staff (V2 staff identity + tier)
--   - audit_log (R-41 SEAM #2 · F5 append-only)
--   - idempotency_keys (R-41 SEAM #3 · 24h TTL)
--
-- Out of scope (deferred):
--   - wallet_entries, wallet_balances → James-AI per CD-7 single-writer + WAL-1..4
--   - pods, sessions → downstream phase per CR-6
--   - pricing_happy_hour_schedule → downstream phase
--   - otp_records (Postgres backstop) → optional; Redis-primary for V2.0
--
-- Doctrine alignment:
--   §B-16: NO `phone` column on customer-identity tables; identity = household_id (UUID)
--   R-41 invariant #4: F5 audit is append-only; UPDATE/DELETE revoked at app-role grant time
--   R-41 invariant #3: idempotency unique-index on (operation_id, actor_scope, request_id)
--
-- Author: bono-AI (Captain auth "author the initial migration" 2026-05-19 15:12 IST)
-- Apply: sudo -u postgres psql -d racingpoint_v2 -f 0001_v2_skeleton.sql

BEGIN;

-- ============================================================================
-- Extensions (pgcrypto loaded by Captain auth 'A' 2026-05-19 15:08 IST; CREATE
-- IF NOT EXISTS for portability)
-- ============================================================================

CREATE EXTENSION IF NOT EXISTS pgcrypto;

-- ============================================================================
-- schema_migrations · forward-only ledger
-- ============================================================================

CREATE TABLE schema_migrations (
    version    TEXT PRIMARY KEY,
    applied_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- ============================================================================
-- households · V2 customer identity per §B-16
-- ============================================================================

CREATE TABLE households (
    household_id    UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    display_name    TEXT,
    whatsapp_optin  BOOLEAN NOT NULL DEFAULT FALSE,
    optin_at        TIMESTAMPTZ
    -- §B-16 enforced: NO phone column; phone is a credential at the OTP layer
    -- and maps to household_id only after verified OTP. Profile-level fields
    -- (display_name etc.) live here; PII separation enforced by absence.
);
CREATE INDEX households_created_at_idx ON households (created_at DESC);

-- ============================================================================
-- staff · V2 staff identity + tier
-- ============================================================================

CREATE TABLE staff (
    staff_id       UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    created_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    display_name   TEXT NOT NULL,
    tier           TEXT NOT NULL CHECK (tier IN ('staff', 'shift_lead', 'captain')),
    active         BOOLEAN NOT NULL DEFAULT TRUE,
    email          TEXT UNIQUE,
    last_login_at  TIMESTAMPTZ
);
CREATE INDEX staff_active_tier_idx ON staff (tier) WHERE active = TRUE;

-- ============================================================================
-- audit_log · R-41 SEAM #2 · F5 append-only ledger
--
-- Append-only invariant: at app-role creation, GRANT INSERT, SELECT only.
-- REVOKE UPDATE, DELETE so the role cannot rewrite history.
-- ============================================================================

CREATE TABLE audit_log (
    id            BIGSERIAL PRIMARY KEY,
    ts            TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    actor_id      TEXT NOT NULL,
    actor_tier    TEXT NOT NULL CHECK (actor_tier IN ('customer', 'staff', 'shift_lead', 'captain')),
    action        TEXT NOT NULL,
    target_type   TEXT NOT NULL,
    target_id     TEXT NOT NULL,
    outcome       TEXT NOT NULL CHECK (outcome IN ('success', 'failure', 'throttled', 'conflict')),
    request_id    TEXT,
    operation_id  TEXT,
    body_hash     TEXT,
    detail        JSONB
);
CREATE INDEX audit_log_ts_idx ON audit_log (ts DESC);
CREATE INDEX audit_log_actor_ts_idx ON audit_log (actor_id, ts DESC);
CREATE INDEX audit_log_target_idx ON audit_log (target_type, target_id);
CREATE INDEX audit_log_operation_idx ON audit_log (operation_id) WHERE operation_id IS NOT NULL;

-- ============================================================================
-- idempotency_keys · R-41 SEAM #3 · 24h TTL
--
-- Unique key triple per R-42 §4: (operation_id, actor_scope, request_id)
-- Replay semantics (R-41 invariant #3):
--   - exact match → return cached response (200)
--   - same (op_id, actor) different request_id → 409 conflict (concurrent retry)
--   - same op_id different actor_scope → 409 (cross-actor reuse · IDOR sentinel)
--
-- TTL sweep: DELETE FROM idempotency_keys WHERE expires_at < NOW();
-- Optionally schedule via pg_cron when extension lands.
-- ============================================================================

CREATE TABLE idempotency_keys (
    operation_id     TEXT NOT NULL,
    actor_scope      TEXT NOT NULL,
    request_id       TEXT NOT NULL,
    response_status  INTEGER NOT NULL,
    response_body    JSONB NOT NULL,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at       TIMESTAMPTZ NOT NULL DEFAULT (NOW() + INTERVAL '24 hours'),
    PRIMARY KEY (operation_id, actor_scope, request_id)
);
CREATE INDEX idempotency_keys_op_actor_idx ON idempotency_keys (operation_id, actor_scope);
CREATE INDEX idempotency_keys_expires_idx ON idempotency_keys (expires_at);

-- ============================================================================
-- Migration ledger entry
-- ============================================================================

INSERT INTO schema_migrations (version) VALUES ('0001_v2_skeleton');

COMMIT;
