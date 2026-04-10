# Phase 349: DB Sync via Google Drive - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-04-11
**Phase:** 349-db-sync-google-drive
**Areas discussed:** Venue-Authoritative Table List, Read-Replica Guard Pattern, Sync Lag Health Probe, Admin UI Badge, Restore Drill, Pause Replication
**Mode:** --auto (all decisions auto-selected as recommended defaults)
**Note:** Plans 349-01 and 349-02 already shipped (`b8c7726b`). Only Plan 349-03 remains.

---

## D1: Venue-Authoritative Table List

| Option | Description | Selected |
|--------|-------------|----------|
| Inverse of cloud-authoritative | All tables venue-authoritative EXCEPT those in `authoritative_tables` | ✓ |
| Explicit venue table list | Maintain a separate list of venue-owned tables | |
| No guard, trust scripts | Rely on operational discipline, no code enforcement | |

**User's choice:** [auto] Inverse of cloud-authoritative — single config source, no list drift
**Notes:** Currently `authoritative_tables = ["staff_members"]` — everything else is venue-owned.

---

## D2: Read-Replica Guard Pattern

| Option | Description | Selected |
|--------|-------------|----------|
| Symmetric `venue_authority_guard()` | Same pattern as `cloud_authority_guard()`, inverted logic | ✓ |
| Middleware-based guard | Axum middleware that intercepts all non-GET requests | |
| Database trigger guard | SQLite trigger that rejects writes on cloud | |

**User's choice:** [auto] Symmetric function — mirrors existing pattern, no new abstractions
**Notes:** Applied per-endpoint like the existing guard, returns 409 with consistent error format.

---

## D3: Sync Lag Health Probe

| Option | Description | Selected |
|--------|-------------|----------|
| File-mtime based | Check racecontrol.db mtime on cloud instance | ✓ |
| sync-status.json parsing | Parse timestamp from JSON written by download script | |
| Google Drive API check | Query Drive API for file metadata | |

**User's choice:** [auto] File-mtime — simplest, most reliable, no JSON parsing failure modes
**Notes:** WARN >300s, CRITICAL >900s. Cloud-only probe.

---

## D4: Admin UI Badge (SYNC-04)

| Option | Description | Selected |
|--------|-------------|----------|
| Defer to Phase 354 | Backend API only, UI in Phase 354 | ✓ |
| Include in this phase | Add frontend badge now | |

**User's choice:** [auto] Defer — consistent with Phase 352 D7 pattern

---

## D5: Monthly Restore Drill (SYNC-07)

| Option | Description | Selected |
|--------|-------------|----------|
| Documentation only | RESTORE-DRILL.md with step-by-step | ✓ |
| Automated script | restore-drill.sh with scratch path | |

**User's choice:** [auto] Documentation — operational procedure, not code

---

## D6: Pause Replication (SYNC-08)

| Option | Description | Selected |
|--------|-------------|----------|
| Sentinel file | `touch /tmp/DB_SYNC_PAUSED`, checked by download-db.sh | ✓ |
| Config flag | Add pause flag to db-sync.env | |
| Kill cron job | Remove cron entry to pause | |

**User's choice:** [auto] Sentinel file — Claude's discretion, minimal change

---

## Claude's Discretion

- Guard placement: which specific write endpoints to guard (researcher enumerates)
- Sync probe interval alignment
- sync-status.json format consumption

## Deferred Ideas

- Admin dashboard sync status badge (→ Phase 354)
- Bi-directional conflict resolution
- Automated restore drill
