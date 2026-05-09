-- PR-A W1-S6 PIN-LOCKOUT auto-rotate (Phase 1 of PR-A code authoring)
-- Captain Option C hybrid (2026-05-09 ~20:28 IST) → cascade #8 PR-A authoring
-- Authored per W1-S6-PLAN.md §3.1 + Wave A.2.1 §10 Q-DECISION compliance map
-- Composes-with: §S-146 V1↔V2 RCA · §S-158 V2 Audit-Log Doctrine · NEW-Q-2 ratification
--
-- Adds:
-- 1. staff.lockout_count + staff.lockout_until columns (V1 inheritance — V2 schema track)
-- 2. pin_rotations table (PIN history audit trail)
-- 3. pin_lockout_events table (V2-bounded action_type CHECK per V2 Audit-Log Doctrine)
-- 4. lockout_alerts table (in-memory primary in V2.0; DB persistence forward-compat hook)
--
-- Per PR-A scope: PUBLISHER ONLY — no W1-S5 refresh-path consumer code (gated PR-C)
-- Per Q-W1-CROSS-2-a sequencing: W1-S6 → W1-S5 → W3
--
-- Rollback: 20260510000001_pin_lockout_state_v2.down.sql (separate file)

-- ─── staff lockout state columns (V1-inheritance preserved) ────────────────
-- AMEND-W1-S6: V1 lockout state was implicit (jwt_secret rotation only); V2
-- formalizes lockout state in DB columns. lockout_count = N failed attempts
-- in window; lockout_until = NULL when not locked, ISO-8601 UTC otherwise.
ALTER TABLE staff ADD COLUMN lockout_count INTEGER NOT NULL DEFAULT 0;
ALTER TABLE staff ADD COLUMN lockout_until TEXT;

-- Index for lockout-active predicate query (W1-S6 publisher reads this fast)
CREATE INDEX idx_staff_lockout_active ON staff(id, lockout_until) WHERE lockout_until IS NOT NULL;

-- ─── pin_rotations (audit trail; NEVER-DELETE candidate Phase 0.2.1) ───────
-- Captures every PIN rotation event for audit trail. Sibling table to
-- staff_actions; isolated per V2-Audit-Log-Doctrine bounded-CHECK approach.
CREATE TABLE pin_rotations (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    staff_id        TEXT NOT NULL REFERENCES staff(id) ON DELETE RESTRICT,
    rotation_reason TEXT NOT NULL
                    CHECK (rotation_reason IN (
                        'lockout_threshold_breach',
                        'admin_manual_rotation',
                        'periodic_security_rotation',
                        'staff_self_initiated'
                    )),
    -- old_pin_hint is the FIRST 2 CHARS of pre-rotation PIN; for forensic
    -- correlation only. Full PIN hash NOT stored per V2 security minimality.
    old_pin_hint    TEXT,
    actor_staff_id  TEXT REFERENCES staff(id) ON DELETE RESTRICT,
    created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX idx_pin_rotations_staff_id ON pin_rotations(staff_id);
CREATE INDEX idx_pin_rotations_created_at ON pin_rotations(created_at);

-- ─── pin_lockout_events (V2-bounded action_type CHECK; NEW-Q-2 compliance) ─
-- Per §S-158 V2 Audit-Log Doctrine: action_type is bounded by CHECK constraint
-- to prevent V1-class drift (where audit_log.action accepted only CRUD verbs
-- and PACT-091 had to add a sibling action_type column for MI markers).
-- This V2 table avoids that anti-pattern by bounding action_type from the
-- start.
CREATE TABLE pin_lockout_events (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    staff_id        TEXT NOT NULL REFERENCES staff(id) ON DELETE RESTRICT,
    action_type     TEXT NOT NULL
                    CHECK (action_type IN (
                        'lockout_threshold_breach',
                        'lockout_pin_rotated',
                        'lockout_alert_dispatched_email',
                        'lockout_alert_dispatched_whatsapp',
                        'lockout_alert_dispatch_failed'
                    )),
    payload         TEXT,  -- JSON: event-specific context (recipient, attempt#, error msg)
    created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX idx_pin_lockout_events_staff_id ON pin_lockout_events(staff_id);
CREATE INDEX idx_pin_lockout_events_action_type ON pin_lockout_events(action_type);
CREATE INDEX idx_pin_lockout_events_created_at ON pin_lockout_events(created_at);

-- ─── lockout_alerts (forward-compat hook; in-memory primary in V2.0) ───────
-- Per Q-W1-S6-NEW-2 (Captain G33 v5 #6 verbatim): in-memory queue is
-- kaizen-min for V2.0; restart-loses-queue acceptable per CR-3 customer-
-- service-priority bounded blast radius.
--
-- This DB table is FORWARD-COMPAT ONLY for V2.1+ if persistence becomes
-- necessary. V2.0 retry-queue lives in tokio::sync::mpsc channel.
-- Comment documents the kaizen-min decision; do not write rows in V2.0.
CREATE TABLE lockout_alerts (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    staff_id        TEXT NOT NULL REFERENCES staff(id) ON DELETE RESTRICT,
    -- channel: 'email' (retry queue) | 'whatsapp' (fire-and-forget; never enters here)
    channel         TEXT NOT NULL CHECK (channel = 'email'),
    -- attempt number; max 3 per Q-W1-S6-NEW-2 (10s/60s/300s exponential)
    attempt         INTEGER NOT NULL DEFAULT 1 CHECK (attempt BETWEEN 1 AND 3),
    -- enqueued_at is when entry was queued; retry_after is next-attempt-time
    enqueued_at     TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    retry_after     TEXT NOT NULL,
    -- outcome: NULL while pending; 'ok' / 'timeout' / 'error' on terminal
    outcome         TEXT CHECK (outcome IS NULL OR outcome IN ('ok','timeout','error')),
    last_error      TEXT
);
CREATE INDEX idx_lockout_alerts_pending ON lockout_alerts(retry_after) WHERE outcome IS NULL;
