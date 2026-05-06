-- PACT-20260505-001 V2 Identity Primitive Phase 0 — cirs_lookup_audit table.
-- Sequencing: gates on PACT-20260503-018 Phase-0.5c-SUBSTRATE staff(id) FK
-- target (migration 20260503000003). cirs_lookup_audit.staff_id FKs to
-- staff(id), so the staff migration must apply first. sqlx-migrate runs
-- migrations in lexicographic filename order, which preserves this ordering.
--
-- PACT-20260505-001 §3.2 NEW-FINDING-2 absorbed: PII audit-log REQUIRED v2.0.
-- Records every CIRS lookup invocation (M1 phone / M3 qr / M4 nfc / walk-in)
-- for DPDP audit + 90-day retention reconciliation.
--
-- Spec divergence note: PACT-001 §3.2 spec uses singular `customer(id)`;
-- actual V2-DB schema is plural `customers(id)` (PACT-003 initial_schema.sql
-- line 13). This migration uses the actual table name to compile correctly.
-- The spec-vs-substrate divergence is non-blocking documentation kaizen for
-- a PACT-001 v0.1 amendment; substrate is canonical.
--
-- AMPLIFIER-absorbed CAVEATs from msg=35126:
--   §3.1 phone canonicalization — implemented in cirs.rs, not the schema
--   §2.2 PWA-QR component-boundary guard — frontend, not the schema
--   §2.3 nfc_tag_id NULL-default + no premature partial UNIQUE — covered by
--        absence of nfc_tag_id column in customers table at v2.0 (Phase 0
--        scope is the audit table only; nfc_tag_id customer column lands
--        with Phase 3 M3/M4 plumbing per §5)

CREATE TABLE cirs_lookup_audit (
    id                INTEGER PRIMARY KEY AUTOINCREMENT,
    ts                TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    -- staff_id MUST FK to staff(id). PACT-018 staff table is the FK target.
    -- ON DELETE RESTRICT preserves the audit anchor for forensic retention.
    staff_id          TEXT NOT NULL
                      REFERENCES staff(id) ON DELETE RESTRICT,
    -- customer_id NULL when lookup misses (phone not registered yet) OR
    -- when input_method = 'walk_in_guest_id' (no customer record).
    -- ON DELETE RESTRICT mirrors other tables — DPDP erasure is anonymization
    -- on the customer row, not deletion (per Phase 0.2.1 + customer_legal.rs).
    customer_id       TEXT
                      REFERENCES customers(id) ON DELETE RESTRICT,
    input_method      TEXT NOT NULL
                      CHECK (input_method IN ('phone','qr_payload','nfc_tag_id','walk_in_guest_id')),
    -- DPDP minimization (§3.2): SHA256 hex of raw input value, NEVER plaintext
    -- phone. 64 hex chars when present. NULL allowed for walk_in_guest_id
    -- (no PII to hash).
    input_hash        TEXT,
    result            TEXT NOT NULL
                      CHECK (result IN ('found','not_found','error'))
);

-- Indexes per §3.2 + audit query patterns.
-- staff_id index supports per-staff audit queries (DPDP regulator request).
CREATE INDEX idx_cirs_audit_staff ON cirs_lookup_audit(staff_id);
-- customer_id partial index (skip NULL rows) supports per-customer audit
-- queries (DPDP subject access request). NULL is the common case for misses
-- + walk-ins, so partial keeps the index small.
CREATE INDEX idx_cirs_audit_customer ON cirs_lookup_audit(customer_id) WHERE customer_id IS NOT NULL;
-- ts index supports retention sweeps (90-day default per §3.2).
CREATE INDEX idx_cirs_audit_ts ON cirs_lookup_audit(ts);
