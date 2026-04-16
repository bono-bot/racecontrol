# Phase 346: Cafe Menu Proxy Rewrite — SUMMARY

**Status:** PARTIAL (346-01 shipped, 346-02/346-03 deferred)
**Milestone:** v47.0 Admin Dashboard Venue-Ready Hardening
**346-01 Completed:** 2026-04-09

## What Was Built

### 346-01: Cafe proxy scaffolding (racingpoint-admin@613d1c4)
- `CAFE_PROXY_ENABLED` env flag (default `false`) for safe rollout
- Dual-path cafe/menu route: proxy mode reads from rc `/cafe/items`, legacy mode reads from admin.db
- Correct paise-to-rupees conversion on read path
- Write path returns explicit 503 in proxy mode (no silent data corruption)
- `withAdminDbError` helper adopted in legacy paths
- Schema diff documented in `ADMIN-SSOT-GAP-REPORT.md`

### 346-02: Cutover — DEFERRED
- Requires maintenance window, Playwright contract test (Phase 350), and live POS/kiosk smoke test
- Write path (POST/PUT/DELETE) not yet implemented

### 346-03: Identity consolidation — DEFERRED
- Depends on Phase 347 staff management

## Requirements Closed

- ADMIN-14 (partial -- GET proxy implemented, write path pending)
- ADMIN-20 (schema diff documented)

## Outstanding

- 346-02 cutover requires maintenance window
- 346-03 identity consolidation requires Phase 347

---

*Retroactive summary written 2026-04-16 to close phase gap artifact.*
