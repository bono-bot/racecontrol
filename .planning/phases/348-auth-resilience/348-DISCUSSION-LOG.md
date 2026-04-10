# Phase 348: Auth Resilience - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-04-11
**Phase:** 348-auth-resilience
**Areas discussed:** Lockout thresholds, Break-glass access model, Audit trail, Lockout recovery
**Mode:** --auto (all decisions auto-selected from recommended defaults)

---

## Lockout Thresholds

| Option | Description | Selected |
|--------|-------------|----------|
| 10 failures in 5min, DB-backed | Matches kiosk pattern, survives restart | ✓ |
| 5 failures in 3min, stricter | More aggressive, higher false-positive risk | |
| 20 failures in 10min, lenient | Less intrusive, longer attack window | |

**User's choice:** [auto] 10 failures in 5 minutes, DB-backed (recommended default — matches existing kiosk_redeem_pin pattern)
**Notes:** Dual lockout: per-IP in-memory + per-staff-id DB-backed. Already implemented in `da0fb590`.

---

## Break-Glass Access Model

| Option | Description | Selected |
|--------|-------------|----------|
| Pre-shared secret + 1h JWT + WhatsApp alert | Simple, auditable, alerting built-in | ✓ |
| TOTP-based emergency code | More secure but requires setup overhead | |
| Admin email recovery link | Requires email infra not yet available | |

**User's choice:** [auto] Pre-shared secret + 1h JWT + WhatsApp alert (recommended default — minimal infra, maximum visibility)
**Notes:** Endpoint returns 404 if not configured. Requires reason field. Already implemented in `a051c5d7`.

---

## Audit Trail

| Option | Description | Selected |
|--------|-------------|----------|
| Full DB audit trail | Every attempt recorded with IP, staff_id, success, timestamp | ✓ |
| Log-only audit | Write to tracing logs, no DB persistence | |

**User's choice:** [auto] Full DB audit trail (recommended default — queryable, persistent)
**Notes:** Uses existing `accounting::log_admin_action` for break-glass events.

---

## Lockout Recovery

| Option | Description | Selected |
|--------|-------------|----------|
| Time-based auto-recovery | 5-minute window resets automatically | ✓ |
| Admin manual unlock | Requires another admin to unlock | |
| Both (time + manual) | Auto-recovery with optional manual override | |

**User's choice:** [auto] Time-based auto-recovery (recommended default — no admin intervention needed for false positives)
**Notes:** Sliding window means old attempts age out naturally.

---

## Claude's Discretion

- Per-IP lockout threshold aligned with kiosk_redeem_pin (10 attempts) for consistency
- DB index strategy: composite indexes on (staff_id, attempted_at) and (source_ip, attempted_at)

## Deferred Ideas

None — discussion stayed within phase scope.
