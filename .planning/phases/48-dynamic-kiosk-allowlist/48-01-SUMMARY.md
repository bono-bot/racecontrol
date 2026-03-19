---
phase: 48-dynamic-kiosk-allowlist
plan: "01"
subsystem: racecontrol + kiosk
tags: [allowlist, kiosk, admin-panel, crud, sqlite]
dependency_graph:
  requires: []
  provides: [kiosk-allowlist-api, kiosk-allowlist-ui]
  affects: [racecontrol-server, kiosk-admin-panel]
tech_stack:
  added: []
  patterns: [sqlite-migration, axum-handler, nextjs-use-state]
key_files:
  created: []
  modified:
    - crates/racecontrol/src/db/mod.rs
    - crates/racecontrol/src/api/routes.rs
    - kiosk/src/lib/api.ts
    - kiosk/src/app/settings/page.tsx
decisions:
  - "BASELINE_PROCESSES const slice of 14 common system process names as a UX guard (not security boundary) — authoritative 70+ baseline lives in rc-agent"
  - "INSERT OR IGNORE on process_name UNIQUE constraint handles duplicates; separate already_exists response distinguishes from baseline guard"
  - "DELETE returns 204 on both found and not-found — no 404 needed for idempotent delete"
  - "hardcoded_count: 70 is informational only; admin UI shows it to help staff understand scope"
  - "listKioskAllowlist refetched after successful add (not local mutation) — ensures server-assigned id and created_at are in local state"
metrics:
  duration: 18
  completed_date: "2026-03-19"
  tasks_completed: 2
  files_modified: 4
---

# Phase 48 Plan 01: Kiosk Allowlist API + Admin Panel Summary

**One-liner:** SQLite kiosk_allowlist table with GET/POST/DELETE API endpoints and admin panel management section, enabling staff to add allowed processes without code changes or pod redeployment.

## What Was Built

### Task 1: DB migration + API route handlers (commit f3576ba)

Added `kiosk_allowlist` table to `migrate()` in `db/mod.rs`:
- Schema: `id TEXT PRIMARY KEY, process_name TEXT UNIQUE, added_by TEXT, notes TEXT, created_at TEXT`
- `idx_kiosk_allowlist_name` index for fast process name lookups
- Placed after all existing migrations (idempotent, `CREATE TABLE IF NOT EXISTS`)

Three route handlers in `routes.rs`:
- `GET /api/v1/config/kiosk-allowlist` — returns `{ allowlist: [...], hardcoded_count: 70 }`
- `POST /api/v1/config/kiosk-allowlist` — adds entry; returns `already_in_baseline` for 14 common system processes as a UX guard; `INSERT OR IGNORE` handles duplicates
- `DELETE /api/v1/config/kiosk-allowlist/:name` — removes by process name (case-insensitive `LOWER()` match)

Routes registered after `/kiosk/pod-launch-experience` in `api_routes()`.

Verification: `cargo build -p racecontrol-crate` and `cargo test -p racecontrol-crate` — 346 tests pass (280 unit + 66 integration).

### Task 2: API client methods + admin panel UI section (commit 3bbd618)

`kiosk/src/lib/api.ts`:
- `listKioskAllowlist()` — typed return with `allowlist[]` and `hardcoded_count`
- `addKioskAllowlistEntry(processName, notes?)` — POST with status/message in response
- `deleteKioskAllowlistEntry(processName)` — DELETE with URL encoding

`kiosk/src/app/settings/page.tsx`:
- Added `allowlist` and `hardcodedCount` state variables
- `listKioskAllowlist()` added to `Promise.all` in `loadData()`
- `handleAddAllowlistEntry()`: calls API, shows `alert()` for already-in-baseline/already-exists, reloads list on success
- `handleDeleteAllowlistEntry(processName)`: calls API + removes from local state
- Kiosk Allowlist section inserted between Experiences and Agent Deploy sections
- UI: count badge header, explanatory text, add input + Enter key support + Add button, per-entry Remove buttons
- Visual style matches existing Experiences section: `bg-rp-card border border-rp-border`, `hover:text-rp-red hover:border-rp-red/50` delete buttons

Verification: `npx next build` passes in kiosk/ directory.

## Must-Have Verification

- `kiosk_allowlist` in `db/mod.rs`: confirmed (CREATE TABLE IF NOT EXISTS)
- `list_kiosk_allowlist` handler in `routes.rs`: confirmed
- `add_kiosk_allowlist_entry` handler in `routes.rs`: confirmed
- `delete_kiosk_allowlist_entry` handler in `routes.rs`: confirmed
- `/config/kiosk-allowlist` route registered: confirmed
- `already_in_baseline` guard present: confirmed
- `idx_kiosk_allowlist_name` index: confirmed
- `cargo build -p racecontrol-crate`: exit 0
- `cargo test -p racecontrol-crate`: 346 tests pass
- `listKioskAllowlist` in `api.ts`: confirmed
- `addKioskAllowlistEntry` in `api.ts`: confirmed
- `deleteKioskAllowlistEntry` in `api.ts`: confirmed
- "Kiosk Allowlist" section heading in `settings/page.tsx`: confirmed
- `handleAddAllowlistEntry` and `handleDeleteAllowlistEntry` in `settings/page.tsx`: confirmed
- `already_in_baseline` handling in `settings/page.tsx`: confirmed
- `npx next build`: exit 0

## Deviations from Plan

None — plan executed exactly as written.

## Commits

| Task | Commit | Description |
|------|--------|-------------|
| 1 | f3576ba | feat(48-01): kiosk allowlist DB table + CRUD API endpoints |
| 2 | 3bbd618 | feat(48-01): admin panel Kiosk Allowlist section + API client methods |

## Self-Check: PASSED
