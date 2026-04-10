# Requirements: v47.0 Admin Dashboard Venue-Ready Hardening

**Defined:** 2026-04-09
**Core Value:** Admin dashboard becomes a resilient, venue-ready single source of truth before customer opening. Cloud and venue admin panels stay in sync and every change propagates to the respective downstream apps (POS, kiosk, pods).

## v47.0 Requirements

Requirements grouped by feature theme. Each maps to one or more roadmap phases. REQ-ID format: `ADMIN-XX`, `AUTH-XX`, `SYNC-XX`, `OPS-XX`, `UI-XX`, `STAFF-XX`, `TEST-XX`.

### Theme 1 — Unbreakable Deploys

- [ ] **ADMIN-01**: Single `admin-deploy.sh` script works identically on venue (Git Bash + Windows) and cloud (bash + Linux)
- [ ] **ADMIN-02**: Post-deploy verify gate asserts: `.next/standalone/.next/static/` exists, login POST returns 200, `better-sqlite3` loads, `/api/health` reports all subsystems green
- [ ] **ADMIN-03**: `admin-deploy.sh --rollback` reverts to the previous build within 60 seconds
- [ ] **ADMIN-04**: Node version pinned via `.nvmrc` + `"engines"` in package.json + explicit `node --version` check in deploy script
- [ ] **ADMIN-05**: Previous build artifact (`prev/`) preserved for at least 72 hours
- [ ] **ADMIN-06**: Six stale `deploy-staging/set-*pin*.js/ps1/py` scripts archived or deleted (Phase 343 C3)
- [ ] **ADMIN-07**: Deploy script requires and validates `RC_URL`, `RC_JWT_SECRET`, `NEXT_PUBLIC_API_URL`, `NEXT_PUBLIC_GATEWAY_URL` environment variables before starting Node

### Theme 2 — Backend Resilience

- [ ] **ADMIN-08**: Module-load errors in `/api/rc/[...path]/route.ts` no longer crash routes — env validation moved inside handlers; returns structured JSON 503 with `error_code`
- [ ] **ADMIN-09**: `admin.db` access uses lazy-load pattern with auto-retry on ABI failure (single `npm rebuild better-sqlite3` attempt before giving up)
- [ ] **ADMIN-10**: Every admin API route returns JSON errors, never HTML plaintext (`Internal Server Error`)
- [ ] **ADMIN-11**: Third circuit breaker added for `admin.db` failures (in addition to existing `gatewayBreaker` + `rcBreaker` from `7d9d3d4`)
- [ ] **ADMIN-12**: Hardcoded JWT secret default literal `"racingpoint-jwt-change-me-in-production"` removed from `crates/racecontrol/src/config.rs:1152+` — startup halts if not explicitly set (Phase 343 C5)
- [ ] **ADMIN-13**: Racecontrol halts startup if `payment_webhook_secret` is unset — no silent acceptance of unsigned webhooks (Phase 343 C6)

### Theme 3 — Single Source of Truth

- [ ] **ADMIN-14**: Admin `/api/cafe/menu` CRUD rewritten to proxy to racecontrol `/api/v1/cafe/items`; no writes to `admin.db.menu_items`
- [ ] **ADMIN-15**: Admin `/api/cafe/inventory` CRUD rewritten to proxy to racecontrol `/api/v1/cafe/items` stock field; no writes to `admin.db.inventory`
- [ ] **ADMIN-16**: `admin.db.menu_items`, `admin.db.inventory`, `admin.db.employees` tables dropped in a migration with pre-cutover snapshot backup
- [ ] **ADMIN-17**: Startup schema-guard refuses to boot admin if dropped tables re-appear in admin.db
- [ ] **ADMIN-18**: Identity source consolidated — `auth/mod.rs` reads from single authoritative table; dead `admin.db.employees` reads removed (Phase 343 C8)
- [ ] **ADMIN-19**: Hardcoded `terminal_pin = "261121"` and `terminal_secret = "rp-terminal-2026"` removed from `deploy-staging/racecontrol.toml` and `racecontrol-server.toml` (Phase 343 D6)
- [ ] **ADMIN-20**: Schema-diff documented in Phase 346 PLAN.md before any cafe cutover — side-by-side field mapping `admin.menu_items` vs `racecontrol.cafe_items`

### Theme 4 — Local↔Cloud Sync Contract

- [ ] **SYNC-01**: Litestream venue→cloud replication configured for `racecontrol.db` (Option A — venue writer, cloud read replica)
- [ ] **SYNC-02**: Backblaze B2 bucket `racingpoint-replica` stores WAL segments with 30-day (720h) retention
- [ ] **SYNC-03**: Litestream systemd service on Bono VPS continuously restores the replica
- [ ] **SYNC-04**: Cloud admin header shows "VENUE MIRROR — read only" badge + last-sync timestamp for replicated tables
- [ ] **SYNC-05**: Cloud racecontrol refuses writes to replicated tables (409 with hint) — mirrors Phase 343 venue-side guard
- [ ] **SYNC-06**: `/api/health` includes `litestream_lag_seconds` probe — WARN >300s, CRITICAL >900s
- [ ] **SYNC-07**: Monthly restore drill documented and executed on a scratch path
- [ ] **SYNC-08**: Break-glass "pause replication" command documented for maintenance windows

### Theme 5 — Live /api/health + Alerting

- [x] **OPS-01**: `/api/health` probes admin_db, rc_backend, gateway, static_assets, db_writable, litestream_lag, disk_free — each returns `{ok, latency_ms, error_code, detail}`
- [x] **OPS-02**: `/settings/health` page renders live per-subsystem tiles with 10s auto-refresh
- [x] **OPS-03**: Degraded subsystem triggers WhatsApp alert via POST to comms-link relay `/relay/alert` on James .27
- [x] **OPS-04**: Alert dedup — same subsystem + error_code within 10 minutes = single alert
- [x] **OPS-05**: Phase 343 Plan 02 `whatsapp_alerter.rs` TODO wired to the alert path
- [ ] **OPS-06**: Structured JSON log format for admin API requests (`{ts, level, route, status, latency, user, err, corr_id}`)
- [ ] **OPS-07**: Admin API logs rotated daily and rsync'd to Bono VPS `/root/backups/venue-logs/`

### Theme 6 — Downstream Propagation Tests

- [ ] **TEST-01**: Playwright contract test: admin cafe menu edit visible on POS `/billing` within 10 seconds
- [ ] **TEST-02**: Playwright contract test: admin pricing rule edit visible on kiosk wizard within 10 seconds
- [ ] **TEST-03**: Playwright contract test: admin coupon add visible on POS + kiosk within 10 seconds
- [ ] **TEST-04**: Playwright contract test: admin staff PIN change propagates to kiosk login within 70 seconds (cloud-authoritative table, reuses Phase 343-04 pattern)
- [ ] **TEST-05**: 46-page admin smoke test asserts no 500/502/error text in DOM on any nav link
- [ ] **TEST-06**: Contract tests + smoke test run as part of `admin-deploy.sh --verify` post-deploy gate

### Theme 7 — Auth Resilience

- [ ] **AUTH-01**: Per-staff-id lockout counter added in addition to per-IP (Phase 343 C4)
- [ ] **AUTH-02**: Admin lockout counter persisted to DB (SQLite `lockout_counters` table) — survives restart (Phase 343 C7)
- [ ] **AUTH-03**: JWT session length extended to 12 hours with sliding-window refresh on activity
- [ ] **AUTH-04**: Rate limiter scoped per-IP; venue IPs (.23, .20, pod subnet) whitelisted
- [ ] **AUTH-05**: Multi-device concurrent login supported — logins on POS + phone + iPad don't invalidate each other
- [ ] **AUTH-06**: Printed break-glass emergency admin token at cafe; any use audit-logged + WhatsApp alerted
- [ ] **AUTH-07**: Admin session list UI under `/settings` with "revoke other sessions" button

### Theme 8 — Data Durability

- [ ] **OPS-08**: Daily `sqlite3 .backup` of admin.db on venue and cloud at 03:00 IST
- [ ] **OPS-09**: Daily `sqlite3 .backup` of racecontrol.db on venue at 03:00 IST (cloud uses Litestream history as backup)
- [ ] **OPS-10**: 30-day rolling retention + first-of-month snapshots retained for 12 months
- [ ] **OPS-11**: Venue backups rsync'd to Bono VPS `/root/backups/venue/` same night
- [ ] **OPS-12**: Both databases run in WAL mode (`PRAGMA journal_mode=WAL`) — verified at startup
- [ ] **OPS-13**: Restore drill SOP documented; executed quarterly with LOGBOOK.md entry
- [ ] **OPS-14**: Automated check alerts if backup file missing or size 0 after scheduled window

### Theme 9 — UI Hardening

- [ ] **UI-01**: `/memberships` + `/wallet-transactions` hidden from nav until racecontrol backends exist (`/customer/drivers`, `/customer/membership/active`, `/customer/membership/tiers`)
- [ ] **UI-02**: Loading skeletons on every rcFetch call — no blank screens during fetch
- [ ] **UI-03**: Empty states on every list page ("No sessions today yet" instead of blank)
- [ ] **UI-04**: Error toasts on every mutation (create/update/delete) success and failure
- [ ] **UI-05**: `/settings/health` page with live per-subsystem tiles (ties to OPS-02)
- [ ] **UI-06**: Degraded banner component per page — shows which subsystem is down + retry button
- [ ] **UI-07**: 46-page Playwright smoke test (shared with TEST-05)

### Theme 10 — Runbook + Staff Training

- [ ] **OPS-15**: Printed one-pager at POS — "If admin is slow/broken: refresh → WhatsApp Bono → don't restart anything"
- [ ] **OPS-16**: Printed one-pager "How to change a staff PIN" — use `/admin/staff`, do NOT use curl/sqlite3/deploy-staging scripts
- [ ] **OPS-17**: Printed one-pager "How to change a cafe menu item" — open admin → Cafe → edit → verify on POS within 10s
- [ ] **OPS-18**: Staff incident log (paper or shared Google Sheet) — one-line per incident
- [ ] **OPS-19**: Morning review ritual — James + Bono read incident log each day before opening

### Theme 11 — Admin Staff Management (from superseded Phase 343-03)

- [ ] **STAFF-01**: `/admin/staff` page renders list of active staff (name, role, last_login_at) with per-row Change PIN button
- [ ] **STAFF-02**: Page is role-gated superadmin + manager only
- [ ] **STAFF-03**: Existing PINs are never displayed (not even redacted) — only metadata
- [ ] **STAFF-04**: Change PIN modal validates: 4+ digit numeric, both inputs match
- [ ] **STAFF-05**: New racecontrol endpoint `POST /api/v1/admin/staff/{id}/change-pin` orchestrates cloud write → immediate verify → venue sync → venue verify
- [ ] **STAFF-06**: `change_staff_pin_safe` response includes `cloud_verified: bool`, `venue_verified: bool`, `latency_ms`, `correlation_id`
- [ ] **STAFF-07**: New racecontrol endpoint `POST /api/v1/sync/pull-now {tables:[...]}` triggers immediate cloud→venue pull, bypassing 30s interval
- [ ] **STAFF-08**: Admin UI shows staged progress: "Writing cloud... ✓ Syncing venue... ✓ Verifying cloud... ✓ Verifying venue... ✓"
- [ ] **STAFF-09**: Error banner on partial success ("PIN changed on cloud but venue sync failed — contact James")
- [ ] **STAFF-10**: Feature-flag `FEATURE_STAFF_PIN_UI=off` by default; deploy gate checks Phase 343 Plans 01+02 shipped in racecontrol before enabling

### Theme 12 — Pre-Flight (Phase 343 Dependency Management)

- [ ] **DEP-01**: Phase 343 Plans 01 + 02 + 04 executed and deployed to venue + cloud racecontrol before Phase 347 ships
- [ ] **DEP-02**: Venue .23 Node version downgraded to 22 LTS (or deploy script forces explicit Node 22 path)
- [ ] **DEP-03**: Racecontrol `/api/v1/admin/staff/{id}/change-pin` endpoint returns something other than 404 before Phase 347 deploys
- [ ] **DEP-04**: Pre-deploy script greps git log for Phase 343 merge commits and hard-fails Phase 347 deploy if missing

### Theme 13 — Business Rules SSOT (added 2026-04-09 after gap audit)

Added after the 2026-04-09 SSOT gap audit found 38 hardcoded business rules + table CRUD gaps across all apps. See `.planning/phases/346-cafe-menu-proxy/ADMIN-SSOT-GAP-REPORT.md`.

- [ ] **BIZRULE-01**: New `business_rules` SQLite table (key/value/type/description/updated_by/updated_at)
- [ ] **BIZRULE-02**: `referral_reward_referrer_paise` + `referral_reward_referee_paise` migrated from billing.rs:4615-4630 hardcode to business_rules
- [ ] **BIZRULE-03**: `discount_approval_threshold_paise` migrated from billing.rs:121 hardcode to business_rules
- [ ] **BIZRULE-04**: `discount_floor_paise` migrated from billing.rs:126 hardcode to business_rules
- [ ] **BIZRULE-05**: `max_manual_refund_paise` migrated from routes.rs:9367 hardcode to business_rules
- [ ] **BIZRULE-06**: `max_linked_racers` migrated from routes.rs:7950 hardcode to business_rules
- [ ] **BIZRULE-07**: `max_ai_single_player` migrated from catalog.rs:11 hardcode to business_rules
- [ ] **BIZRULE-08**: `nudge_ttl_days` + `streak_grace_days` migrated from psychology.rs:136-139 hardcode to business_rules
- [ ] **BIZRULE-09**: `whatsapp_daily_budget_per_customer` migrated from psychology.rs:127 hardcode to business_rules
- [ ] **BIZRULE-10**: `refund_policy_text` + `pricing_policy_text` + `gst_note_text` migrated from routes.rs:2822-2829 literals to business_rules (legal compliance — Consumer Protection Act 2019)
- [ ] **BIZRULE-11**: `trial_session_minutes` migrated to business_rules
- [ ] **BIZRULE-12**: Admin `/settings/business-rules` page with read/edit UI (role-gated superadmin)
- [ ] **BIZRULE-13**: Audit trail in `audit_log` for every business_rules update
- [ ] **BIZRULE-14**: All consumers read from business_rules at runtime (not baked at startup)
- [ ] **BIZRULE-15**: Rollback window — old hardcoded values kept as fallback constants for 30 days

### Theme 14 — Pricing Tiers CRUD

- [ ] **TIER-01**: Admin `/pricing/tiers` page — list, create, edit, delete, reorder
- [ ] **TIER-02**: Racecontrol `POST /pricing/tiers` + `PUT /pricing/tiers/{id}` + `DELETE /pricing/tiers/{id}` admin-gated endpoints
- [ ] **TIER-03**: Add `is_popular` column to `pricing_tiers` table — replaces hardcoded index heuristic in PricingDisplay.tsx:51
- [ ] **TIER-04**: Remove hardcoded "save 7%" / "save 40%" strings from SetupWizard.tsx:297-298 — compute dynamically from tier `base_price_paise` vs `price_paise`
- [ ] **TIER-05**: Schema-guard migration prevents new hardcoded tier references

### Theme 15 — Cafe Promos Admin Page

- [ ] **PROMO-01**: Admin `/cafe/promos` page — list active/inactive promos with type badge (combo/happy_hour/gaming_bundle)
- [ ] **PROMO-02**: Create/edit modal with fields: name, promo_type, config (JSON), start_time, end_time, stacking_group, is_active
- [ ] **PROMO-03**: Toggle active/inactive without reopening modal
- [ ] **PROMO-04**: Delete with confirmation dialog
- [ ] **PROMO-05**: "Broadcast to WhatsApp" button calls existing `POST /cafe/marketing/broadcast` endpoint with dedup cooldown check

### Theme 16 — Bonus Tiers Admin Page

- [ ] **BONUS-01**: Admin `/wallet/bonus-tiers` page — list tiers with min amount + bonus % + sort order
- [ ] **BONUS-02**: Racecontrol `POST /wallet/bonus-tiers` + `PUT /wallet/bonus-tiers/{id}` + `DELETE /wallet/bonus-tiers/{id}` admin-gated endpoints
- [ ] **BONUS-03**: Create/edit modal: min_amount_paise, bonus_percent, sort_order, is_active
- [ ] **BONUS-04**: Preview widget shows "₹500 → 0% bonus → ₹500 credits" for current tiers
- [ ] **BONUS-05**: Role-gated to superadmin + manager

### Theme 17 — Topup Presets SSOT (shipped 2026-04-09 in commit 0c7a8d86)

- [x] **TOPUP-01**: New `system_settings.wallet_topup_presets_paise` key — JSON array of i64 paise values
- [x] **TOPUP-02**: Racecontrol `GET /wallet/topup-presets` public endpoint with safe 8-entry default fallback
- [x] **TOPUP-03**: PWA `api.topupPresets()` helper + `wallet/topup/page.tsx` dynamic state + 8-entry matching fallback
- [x] **TOPUP-04**: POS `WalletTopupModal.tsx` fetches from endpoint + 8-entry matching fallback
- [ ] **TOPUP-05**: Admin `/wallet/topup-presets` editor UI — currently requires direct SQL; deferred to Phase 360-02
- [ ] **TOPUP-06**: Integration test: change presets in system_settings, verify PWA and POS both reflect within 1 render cycle (deferred to Phase 350 contract tests)

## Future Requirements (deferred to v48+)

- Plaintext → Argon2 PIN hashing for `staff_members.pin` (Phase 343 C1) — depends on v47.0 shipping
- HTTPS + mTLS for Bono relay (Phase 343 C2) — needs cert setup
- NVR password startup validation (Phase 343 C9) — separate rc-sentry-ai phase
- OpenRouter key bootstrap validation (Phase 343 C10) — separate rc-agent phase
- Customer CRM expansion (out of scope)
- Loyalty program (out of scope)
- Mobile admin wrapper (out of scope)
- SSO/OAuth/Passkeys (out of scope for <50 staff)

## Out of Scope (explicitly excluded)

- **Sentry / GlitchTip** — new service, Uday can't babysit; use structured JSON logs instead
- **Redis** — lockout counter fits in SQLite at ~50 staff scale
- **Docker / Kubernetes** — bare metal works, single venue
- **Multi-tenancy** — one venue, one operator
- **Real-time WebSocket dashboards** — existing polling is sufficient
- **AI-powered insights / predictions** — adds nothing before customers use the system
- **SAML / LDAP integration** — no directory service
- **Full design system rebuild** — v43.0 handled visual; v47.0 is hardening
- **Internationalization** — English only
- **Page-level breadcrumbs / command palette** — differentiator only, not table stakes

## Traceability

Filled by roadmap phase. Target format: `REQ-ID → Phase NNN`.

---

*Milestone v47.0 defined 2026-04-09 after live Admin Dashboard audit + Phase 343 Vishal-PIN incident handoff.*
