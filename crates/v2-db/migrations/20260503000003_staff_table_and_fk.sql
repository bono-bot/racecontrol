-- PACT-20260503-018 Phase-0.5c-SUBSTRATE staff table + staff_id FK migration.
-- Applies to: SQLite (PACT-003 substrate). Postgres-pivot path documented in
-- POSTGRES-PIVOT.md §Schema-portability (FK ALTER simpler in Postgres).
--
-- Sequencing: gates on PACT-20260503-003 (V2-DB Foundation) merge to main per
-- PACT-018 AMEND-1 Q5-A AGREE (sequential, NOT parallel rebase). Composes-with
-- PACT-20260505-001 V2 Identity Primitive (CIRS) §3.2 cirs_lookup_audit FK
-- target = staff(id) (lands as separate sub-PACT post this migration).
--
-- Bono AMPLIFIER msg=34855 verdict (per PACTS row 318):
--   Q1 FK-add strategy: AGREE-A (recreate-table pattern; canonical SQLite)
--   Q2 staff_actions.action_type set: AGREE + CAVEAT-1 'permission_revoke'
--   Q3 staff.id format: AGREE-A + CAVEAT-2 (V2.1 ULID as future tightening)
--   Q4 backfill: AGREE-A role='inactive', active=0
--   Q5 sub-PACT merge order: AGREE-A sequential after PACT-003
-- AMEND-1 RATIFY (per PACTS row 331):
--   Q5/Q6/Q7 = AGREE-A (sequential / schema absorbs UI / DB-level UNIQUE)
--   NEW-FINDING-1 absorbed: Uuid->String field-type change lands in same
--     commit as this migration (sessions.rs + wallets.rs)
--   CAVEAT-3 absorbed: pin TEXT NOT NULL inline schema comment (security
--     debt closure_phase=Phase-0.5c-AUTH per security-debt-ledger.jsonl)

-- ─── staff (master table) ──────────────────────────────────────────────────
-- AMEND-1 EXTENDED 2026-05-05 ~05:30 IST: schema absorbs racingpoint-admin
-- StaffMember UI contract verbatim (phone, pin, last_login_at, 5-role union)
-- per Q6-A AGREE-A.
CREATE TABLE staff (
    id              TEXT PRIMARY KEY,
    name            TEXT NOT NULL,
    -- AMEND-1: V1 staff_crud.rs:64 LOWER(name) duplicate-prevention; UI form
    -- requires phone. UNIQUE-WHERE-active=1 mirrors V1 behavior. No regex
    -- CHECK to avoid locking future locale shifts.
    phone           TEXT NOT NULL,
    -- AMEND-1 SECURITY-DEBT (CAVEAT-3 absorbed): V1 staff_crud.rs:77 stores
    -- raw `req.pin` and validates via validate_pin_format(); Phase-0.5c
    -- preserves V1 contract initially. Hash-via-bcrypt is sibling sub-PACT
    -- Phase-0.5c-AUTH (closure_phase per security-debt-ledger.jsonl entry
    -- class=credential-storage). TEXT NOT NULL is the V1 surface; any
    -- column-level transformation defers to that sub-PACT.
    pin             TEXT NOT NULL,
    role            TEXT NOT NULL
                    CHECK (role IN ('staff','cashier','manager','superadmin','inactive')),
    active          INTEGER NOT NULL DEFAULT 1 CHECK (active IN (0,1)),
    -- AMEND-1: written by backend on JWT issuance (not UI-write). Sub-PACT
    -- scope for write-trigger code; this migration adds the column only.
    last_login_at   TEXT,
    created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    -- staff_id of admin who onboarded this row; NULL = system seed (orphan
    -- backfill). Self-referential FK keeps audit anchor on hierarchy.
    created_by      TEXT REFERENCES staff(id) ON DELETE RESTRICT,
    updated_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX idx_staff_active ON staff(active) WHERE active = 1;
-- AMEND-1: V1 staff_crud.rs:64 duplicate-phone prevention via partial UNIQUE
CREATE UNIQUE INDEX idx_staff_phone_active ON staff(phone) WHERE active = 1;
-- AMEND-1: V1 staff_crud.rs:77 duplicate-pin prevention via partial UNIQUE
CREATE UNIQUE INDEX idx_staff_pin_active ON staff(pin) WHERE active = 1;

-- ─── staff_actions (audit table; NEVER-DELETE candidate Phase 0.2.1) ───────
-- AMEND-1: action_type set per bono AMPLIFIER CAVEAT-1 absorbed: added
-- 'permission_revoke' for explicit role-downgrade events (distinct from
-- 'deactivated' = active=1->0 toggle).
CREATE TABLE staff_actions (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    staff_id        TEXT NOT NULL REFERENCES staff(id) ON DELETE RESTRICT,
    action_type     TEXT NOT NULL
                    CHECK (action_type IN (
                        'created','activated','deactivated',
                        'role_changed','permission_revoke','renamed'
                    )),
    old_value       TEXT,  -- JSON: pre-change state (NULL on 'created')
    new_value       TEXT,  -- JSON: post-change state
    actor_staff_id  TEXT REFERENCES staff(id) ON DELETE RESTRICT,
    created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX idx_staff_actions_staff ON staff_actions(staff_id);
CREATE INDEX idx_staff_actions_actor ON staff_actions(actor_staff_id);
CREATE INDEX idx_staff_actions_type ON staff_actions(action_type);

-- ─── Backfill orphan staff_id values from sessions + wallet_topups ─────────
-- CAVEAT-3 verbatim: default backfill role = 'inactive' (NOT 'cashier' —
-- accidental privilege grant on historical orphans). active=0 + role='inactive'
-- excludes orphans from idx_staff_phone_active + idx_staff_pin_active UNIQUE
-- partial indexes (no collision risk).
-- AMEND-1: phone='0000000000' + pin='0000' placeholder per CLAUDE.md
-- "no fake data" rule (TEST_ONLY/0000000000/empty allowed).
-- Admin must explicitly re-activate via UI: flip role away from 'inactive'
-- AND replace placeholder phone+pin AND set active=1.
INSERT OR IGNORE INTO staff (id, name, phone, pin, role, active, created_at, created_by)
SELECT DISTINCT staff_id, '<unknown-orphan-' || staff_id || '>', '0000000000', '0000', 'inactive', 0,
       (strftime('%Y-%m-%dT%H:%M:%fZ','now')), NULL
FROM (
    SELECT staff_id FROM sessions WHERE staff_id IS NOT NULL
    UNION
    SELECT staff_id FROM wallet_topups WHERE staff_id IS NOT NULL
);

-- Audit-log the backfill event for forensic clarity (per §2.5 verbatim).
INSERT INTO staff_actions (staff_id, action_type, old_value, new_value, actor_staff_id, created_at)
SELECT id, 'created', NULL,
       json_object('source', 'phase-0.5c-orphan-backfill', 'role', 'inactive', 'active', 0),
       NULL, (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
FROM staff
WHERE created_by IS NULL AND role = 'inactive';

-- ─── FK ALTER on sessions.staff_id (recreate-table pattern Q1=A) ───────────
-- SQLite limitation: ALTER TABLE ADD CONSTRAINT FOREIGN KEY not supported.
-- Canonical pattern per https://sqlite.org/lang_altertable.html §7:
--   rename → create new with FK → copy data → drop old → recreate indexes.
-- PACT-003 ships empty DB; copy is a no-op for fresh installs. For migrations
-- applied to populated DBs (future schema evolutions), the backfill above
-- ensures FK targets exist before this recreate.
ALTER TABLE sessions RENAME TO sessions_old_pact018;

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
    -- PACT-018: FK to staff(id) ON DELETE RESTRICT (preserves audit anchor)
    staff_id            TEXT NOT NULL
                        REFERENCES staff(id) ON DELETE RESTRICT,
    created_at          TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at          TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

INSERT INTO sessions (id, customer_id, pod_id, lobby_id, session_type, started_at, ended_at,
                     billing_state, credits_consumed, staff_id, created_at, updated_at)
SELECT id, customer_id, pod_id, lobby_id, session_type, started_at, ended_at,
       billing_state, credits_consumed, staff_id, created_at, updated_at
FROM sessions_old_pact018;

DROP TABLE sessions_old_pact018;

-- Recreate sessions indexes (per 001 schema)
CREATE INDEX idx_sessions_customer ON sessions(customer_id);
CREATE INDEX idx_sessions_pod ON sessions(pod_id);
CREATE INDEX idx_sessions_lobby ON sessions(lobby_id) WHERE lobby_id IS NOT NULL;
CREATE INDEX idx_sessions_active ON sessions(billing_state) WHERE billing_state IN ('active','blanking_grace');

-- ─── FK ALTER on wallet_topups.staff_id (recreate-table pattern Q1=A) ──────
-- DROP TRIGGER trg_wallet_topups_no_delete first — the trigger is bound to
-- the OLD wallet_topups; renaming the table orphans it. Recreated post-swap
-- (verbatim from migration 002 to preserve MMA VERIFY P0 NEVER-DELETE
-- invariant + 8-year audit retention).
DROP TRIGGER trg_wallet_topups_no_delete;

ALTER TABLE wallet_topups RENAME TO wallet_topups_old_pact018;

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
    -- PACT-018: FK to staff(id) ON DELETE RESTRICT (preserves audit anchor)
    staff_id              TEXT NOT NULL
                          REFERENCES staff(id) ON DELETE RESTRICT,
    pos_id                TEXT NOT NULL,
    tax_invoice_no        TEXT NOT NULL UNIQUE,
    created_at            TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

INSERT INTO wallet_topups (id, wallet_id, credits_purchased, gst_collected_paise,
                          amount_paid_paise, gst_rate_bps, payment_method, payment_ref,
                          staff_id, pos_id, tax_invoice_no, created_at)
SELECT id, wallet_id, credits_purchased, gst_collected_paise,
       amount_paid_paise, gst_rate_bps, payment_method, payment_ref,
       staff_id, pos_id, tax_invoice_no, created_at
FROM wallet_topups_old_pact018;

DROP TABLE wallet_topups_old_pact018;

-- Recreate indexes (per 001 schema)
CREATE INDEX idx_topups_wallet ON wallet_topups(wallet_id);
CREATE INDEX idx_topups_staff ON wallet_topups(staff_id);

-- Recreate the BEFORE DELETE trigger from migration 002 (verbatim — preserve
-- MMA VERIFY P0 NEVER-DELETE invariant + CGST 8-year audit retention).
CREATE TRIGGER trg_wallet_topups_no_delete
BEFORE DELETE ON wallet_topups
BEGIN
    SELECT RAISE(ABORT,
        'wallet_topups is append-only audit trail (8-year retention per CGST + Wallet Framing C)');
END;
