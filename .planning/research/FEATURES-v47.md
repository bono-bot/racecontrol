---
milestone: v47.0
dimension: features
date: 2026-04-09
author: James (direct, research agents API-overloaded)
---

# v47.0 Features Research — Admin Dashboard Venue-Ready Hardening

## Scope note

Research agents hit 529 overload. Written directly. Categorizations are opinionated based on "will this matter in the first 30 days of customer operation?"

**Categories:**
- **Table stakes** — MUST have before venue opens. Ship-blocker.
- **Differentiators** — Add real value if time permits. Not blocking.
- **Anti-feature** — Tempting but EXCLUDED from v47.0. Deferred or explicitly never.

---

## Theme 1 — Unbreakable deploys

### Table stakes
- Single `admin-deploy.sh` script that works on both venue (Git Bash on Windows) + cloud (bash on Linux)
- Post-deploy verify gate: static assets present, login round-trip returns 200, better-sqlite3 loads, `/api/health` reports all subsystems green
- **Rollback:** previous binary/build preserved for 72h, `admin-deploy.sh --rollback` reverts
- Pinned Node version via `.nvmrc` + `"engines"` in package.json
- Archive/delete the 6 stale `deploy-staging/set-pin.*` scripts (Phase 343 C3)

### Differentiators
- CI green gate: admin-deploy.sh runs in GitHub Actions on every PR merged to main, blocks deploy if any step fails
- Deploy log persisted to `/root/backups/deploy-history/` for audit
- Slack/WhatsApp notification on deploy start + end with build ID

### Anti-features (OUT)
- ❌ Blue-green deployment — overkill for single-node admin
- ❌ Canary deploy with traffic shifting — no load balancer
- ❌ Kubernetes rolling deploy — bare metal
- ❌ Helm charts — not using k8s
- ❌ Ansible/Puppet playbooks — script is fine

---

## Theme 2 — Backend resilience

### Table stakes
- Module-load errors NEVER crash a route — env validation moved inside handlers, returns structured JSON 503 with `error_code` like `RC_URL_MISSING`
- admin.db lazy-load with auto-rebuild retry on ABI failure
- Every route returns JSON errors, not HTML (current state: several return plain-text "Internal Server Error")
- Split circuit breakers already exist (gatewayBreaker, rcBreaker from 2026-04-07 `7d9d3d4`) — add 3rd breaker for admin.db
- Remove hardcoded JWT secret default literal from `crates/racecontrol/src/config.rs:1152+` (Phase 343 C5)
- Halt startup if `payment_webhook_secret` unset (Phase 343 C6) — no silent unsigned webhook acceptance

### Differentiators
- Graceful degradation banner per page: "Cafe backend unavailable — kitchen ops unaffected" with retry button
- Request-level correlation IDs (UUID) in every log + response header for cross-system tracing
- Server-sent events (SSE) push for real-time health status to already-open admin tabs

### Anti-features (OUT)
- ❌ Full service mesh (Istio, Linkerd) — overkill
- ❌ Kubernetes liveness/readiness probes — no k8s
- ❌ Hystrix-style dashboard — circuit breakers are fine without UI
- ❌ Chaos engineering tests — manual failure testing during drill is enough for v47.0

---

## Theme 3 — Single source of truth enforcement

### Table stakes
- **Cafe menu/inventory rewrite** — admin `/api/cafe/menu` and `/api/cafe/inventory` proxy to racecontrol `crates/racecontrol/src/cafe.rs` CRUD. Delete `admin.db.menu_items` and `admin.db.inventory` tables in a migration.
- Drop empty `admin.db.employees` table (dead code, `/api/hr/employees` already proxies to rc `/staff`) — Phase 343 D4
- Identity source consolidation — decide `staff_members` vs `drivers.is_employee` vs `employees` single source (Phase 343 C8); `auth/mod.rs:1815-1820` currently reads from `drivers.is_employee`, inconsistent
- Schema guard: startup check refuses to boot admin if dead tables re-appear
- Delete hardcoded `terminal_pin = "261121"` + `terminal_secret = "rp-terminal-2026"` from `deploy-staging/racecontrol.toml` + `racecontrol-server.toml` (Phase 343 D6)

### Differentiators
- Contract test: Playwright adds a cafe item in admin → verifies it appears on POS billing `/billing` page within 10s (venue). Same test on cloud after sync lag.
- Dup drivers cleanup cron (Phase 343 D5) — 4 `l1-parent-*`, 19 `test_only audit driver`, 21 `unknown` get flagged for manual cleanup
- Data dictionary auto-generated from DB schema, linked from `/settings`

### Anti-features (OUT)
- ❌ Full CQRS / event sourcing rewrite — too much blast radius
- ❌ Replacing SQLite with Postgres — single-venue, no need
- ❌ Dual-write pattern during cafe migration — cutover + restore drill is simpler
- ❌ Read replicas within venue — overkill

---

## Theme 4 — Local↔cloud sync contract (Litestream read replica)

### Table stakes
- Litestream venue→cloud replication for `racecontrol.db` (Option A confirmed by user)
- Cloud admin header: "VENUE MIRROR — read only for synced tables" badge + last-sync timestamp
- Cloud racecontrol refuses writes to replicated tables (error 409 with hint to use venue endpoint) — extends Phase 343-01 cloud-authority guard inversely for replica mode
- Litestream lag detection in `/api/health` — WARN >5min, CRITICAL >15min
- Monthly restore drill documented and executed
- Cloud admin.db continues to be independent (it's admin-native anyway — nothing to replicate after theme 3 proxies cafe to rc)

### Differentiators
- Automatic failover: if venue racecontrol is down, cloud admin shows read-only view of last-known state (already possible with replica)
- "Pause replication" break-glass command for maintenance windows
- Litestream backups to off-site Backblaze B2 (DR beyond cloud VPS)

### Anti-features (OUT)
- ❌ Multi-master replication (rqlite) — single venue writer is sufficient
- ❌ Postgres with logical replication — see theme 3 anti-features
- ❌ Custom sync protocol — use Litestream, don't invent
- ❌ Real-time streaming to cloud admin UI (WebSocket tail) — 1s Litestream lag is fine

---

## Theme 5 — Live /api/health + alerting

### Table stakes
- Per-subsystem probes: admin_db, rc_backend, gateway, static_assets, db_writable, litestream_lag, disk_free
- Each returns `{ok: bool, latency_ms, error_code, detail}`
- WhatsApp alert on degradation → POST to comms-link relay `/relay/alert` on James .27
- Alert dedup: same subsystem + error_code within 10 minutes = single alert
- Phase 343 Plan 02 TODO (`whatsapp_alerter.rs`) wired to this system
- Admin settings/health page shows big red/yellow/green tile per subsystem, 10s auto-refresh

### Differentiators
- Alert escalation: if not acknowledged in 15min, escalate to second contact
- Health history chart (last 24h per subsystem) in settings/health
- Prometheus exposition format at `/metrics` (already have rc's version; add admin's)

### Anti-features (OUT)
- ❌ Grafana dashboards for admin — v34 has rc metrics, don't duplicate
- ❌ Alertmanager clustering — single venue, single alerter
- ❌ PagerDuty / Opsgenie integration — WhatsApp is sufficient
- ❌ Log-based alerting (Loki) — structured probe checks are enough
- ❌ Anomaly detection / ML alerts — too complex for v47.0

---

## Theme 6 — Downstream propagation tests

### Table stakes
- Playwright contract test for every admin→downstream data flow:
  - admin cafe menu edit → POS billing `/billing` page (10s)
  - admin pricing rule → kiosk experience selector (10s)
  - admin coupon → kiosk + POS (10s)
  - admin staff create/edit/delete → kiosk PIN login (70s for cloud-sync'd staff)
  - admin kiosk experience → kiosk wizard (10s)
- Tests run as part of `admin-deploy.sh` verify gate (post-deploy)
- Reuse Phase 343-04 `staff-pin-lifecycle.spec.ts` 70s-wait pattern for cloud-authoritative writes

### Differentiators
- Synthetic monitoring mode: same tests run every 5min via cron, alert on failure
- Multi-browser matrix (Chromium + Firefox + WebKit)

### Anti-features (OUT)
- ❌ Cypress — already standardized on Playwright
- ❌ Visual regression testing for every page — v43.0 handles visual; this theme is about data contracts
- ❌ Load testing / stress testing — single venue, low concurrency
- ❌ Fuzz testing admin forms — too broad

---

## Theme 7 — Auth resilience

### Table stakes
- Per-staff-id lockout tracking (Phase 343 C4) in addition to per-IP — attacker rotating IPs is blocked
- Persist admin lockout counter to DB (Phase 343 C7) — `persist_lockout_to_db()` stub already exists, wire it
- JWT session length: 12h (currently short, staff getting kicked mid-shift is P0 UX bug)
- Rate limiter scoped per-IP + whitelisted .23/.20 (venue trusted network)
- Multi-device login (concurrent sessions not invalidated — staff uses POS + phone + iPad)
- **Break-glass token:** physical printed emergency admin token at cafe, any use logged + alerted

### Differentiators
- JWT refresh on activity (sliding window)
- Role-based: superadmin, manager, cashier (already exists at DB level; enforce in admin UI)
- 2FA via TOTP for superadmin only (Uday uses it)
- Session list UI: show all active admin sessions, "revoke all other sessions" button

### Anti-features (OUT)
- ❌ Passkeys / WebAuthn — too complex for v47.0
- ❌ OAuth / SSO — no external IdP
- ❌ Biometric (fingerprint) — not needed
- ❌ SAML — enterprise, we're small
- ❌ LDAP integration — no directory service
- ❌ IP whitelist at firewall — admin needs to be reachable from phones at venue
- ❌ CAPTCHA — staff shouldn't face them

---

## Theme 8 — Data durability

### Table stakes
- Daily `sqlite3 .backup` of admin.db + racecontrol.db on both venue + cloud
- 30-day rolling retention + 12-month snapshots (1st of each month)
- Rsync/scp backups to Bono VPS (venue → cloud) daily
- WAL mode confirmed on both databases (crash-safe)
- Quarterly restore drill — execute script, assert row counts, documented in LOGBOOK.md
- Automated check: alert if backup file missing or size 0 after scheduled window

### Differentiators
- Off-site secondary backup to Backblaze B2 (Litestream gets us this for racecontrol.db — add manual for admin.db)
- Point-in-time restore via Litestream for racecontrol.db (WAL retention = 720h)
- Encrypted backups (age or gpg) for PII-containing tables

### Anti-features (OUT)
- ❌ Snapshot-based file system backups (ZFS, btrfs) — Windows venue precludes
- ❌ Tape / offline backups — over-engineered
- ❌ Immutable backups (WORM) — too complex for venue scale

---

## Theme 9 — UI hardening

### Table stakes
- Hide `/memberships` + `/wallet-transactions` from nav until racecontrol backends exist (currently 502 on both envs)
- Loading skeletons on every rcFetch call (currently blank for 2-3s on slow pages)
- Empty states: "No sessions today yet" instead of blank
- Error toasts on every mutation (create/update/delete) success + failure
- 46-page Playwright smoke test covering every nav link, asserts no `500`/`502`/`error` text in DOM, runs in deploy verify gate
- Dead-route removal / "COMING SOON" badges for pages whose backends are stubbed

### Differentiators
- `/settings/health` page upgrade with live per-subsystem tiles (ties to theme 5)
- Keyboard shortcuts for common actions (Cmd+K command palette)
- Page-level breadcrumbs for 2+ level routes
- Dark mode (Racing Point brand black `#1A1A1A`)

### Anti-features (OUT)
- ❌ Full design system rebuild — v43.0 did self-audit; this is hardening not redesign
- ❌ Internationalization (i18n) — English only, single venue in India
- ❌ Accessibility AAA compliance — aim for AA on new surfaces, don't backport
- ❌ Mobile-first redesign — responsive enough already
- ❌ Animation library (Framer Motion) — no new UI eye-candy
- ❌ 3D graphics / charts — no

---

## Theme 10 — Runbook + staff training

### Table stakes
- **Printed one-pager at POS** — "If admin is slow/broken: refresh → WhatsApp Bono → don't restart anything"
- **"How to change a staff PIN"** one-pager — use `/admin/staff`, do NOT use curl/sqlite3/deploy-staging (Phase 343 lesson)
- **Cafe menu change procedure** — open admin → Cafe → add → verify on POS within 10s → if not, menu didn't save
- Staff incident log (paper sheet or Google Sheet) — one-line per incident
- James + Bono review incident log each morning

### Differentiators
- Video walkthroughs for each procedure
- In-app tooltip tour for first-time users
- Staff onboarding checklist with signoff

### Anti-features (OUT)
- ❌ Full LMS / training platform — overkill
- ❌ Gamified training — no
- ❌ Multi-language training docs — English enough

---

## Theme 11 — Admin Staff Management (from superseded Phase 343-03)

### Table stakes
- `/admin/staff` page listing active staff: name, role, last_login_at, Change PIN button (role-gated superadmin + manager only)
- Change PIN modal: new PIN + confirm PIN, 4+ digit numeric validation
- Staged progress UI: "Writing cloud... ✓ Syncing venue... ✓ Verifying cloud... ✓ Verifying venue... ✓"
- New racecontrol endpoint `POST /api/v1/admin/staff/{id}/change-pin` — orchestrates cloud write + immediate verify + venue sync + venue verify. Returns `{cloud_verified, venue_verified, latency_ms}`.
- New racecontrol endpoint `POST /api/v1/sync/pull-now {tables:[...]}` — triggers immediate pull, bypassing 30s interval
- Admin page never displays PINs (even redacted) — only metadata
- **Hard dependency:** Phase 343 Plans 01 + 02 must ship first (409 guard + post-write verify). v47.0 Phase 344+ is blocked on this.

### Differentiators
- Bulk PIN reset (all staff force new PINs) — admin-only
- Staff login history dashboard (last 10 logins per staff, IP + user agent)
- "Impersonate staff" for debugging (superadmin only, audit-logged)
- Last_pin_change_at column + staff PIN age report (>90d old = warn)
- Soft-delete with undo window instead of hard DELETE

### Anti-features (OUT)
- ❌ Self-service PIN reset via email — staff don't have email accounts
- ❌ SMS PIN reset — too expensive per send
- ❌ Biometric PIN replacement — hardware not available
- ❌ Password (long) instead of PIN — POS pad is numeric-only
- ❌ PIN strength meter — 4-digit numeric by design

---

## Cross-cutting anti-features (NEVER in v47.0)

- ❌ **AI-powered insights** — tempting, adds nothing before customers use the system
- ❌ **Predictive alerting** — health probes are sufficient
- ❌ **Customer-facing features** in admin (kiosk modes, PWA) — out of scope
- ❌ **Multi-venue support** — one venue
- ❌ **Billing reconciliation automation with Tally/QuickBooks** — manual export is fine
- ❌ **Inventory predictions / auto-reorder** — cafe theme is "make admin edit reach POS", not "AI inventory"
- ❌ **Customer CRM features** — customer module already exists, don't expand
- ❌ **Loyalty program** — separate milestone
- ❌ **WhatsApp chatbot for admin queries** — no
- ❌ **Voice commands** — no
- ❌ **GraphQL federation** — no
- ❌ **Microservices split** — monolith is fine at our scale

---

## Venue Opening Readiness Checklist (proposed for v47.0 completion gate)

Admin dashboard is "venue-ready" when ALL of these are green:

| # | Criterion | Verification |
|---|---|---|
| 1 | `admin-deploy.sh` works fresh-VM in <3min on BOTH venue + cloud | Tested on a scratch VM |
| 2 | Cloud admin login returns 200 (currently 500) | `curl -X POST cloud/api/auth/login` |
| 3 | Cloud admin static assets serve (currently 404) | `curl cloud/_next/static/chunks/main.js` |
| 4 | Local admin `/api/cafe/menu` returns data (currently 500 ABI) | `curl venue/api/cafe/menu` |
| 5 | Cafe menu edit in admin reflects on POS `/billing` within 10s | Playwright contract test passes |
| 6 | Killing rc backend → admin UI stays up, shows degraded banner | Manual drill |
| 7 | Litestream replicating venue→cloud, lag < 60s | `/api/health` returns green |
| 8 | `/memberships` and `/wallet-transactions` hidden from nav | Playwright smoke test |
| 9 | Daily backup runs, size > 0, rsync to Bono succeeds | Restore drill once |
| 10 | Per-staff-id lockout persists across restart | Backend test |
| 11 | `/admin/staff` page works for Uday (manual smoke) | Manual walkthrough |
| 12 | Phase 343 Plans 01+02 shipped in racecontrol | `git log` + `/fleet/health` check |
| 13 | `/api/health` reports true ground truth per subsystem | Deliberate break + verify detection |
| 14 | All 46 admin pages pass Playwright smoke (no 500s) | CI job green |
| 15 | WhatsApp alert fires within 30s of subsystem degradation | Manual drill |
| 16 | Restore drill successfully reconstructs admin.db | Quarterly SOP executed |
| 17 | Printed runbook at POS, staff trained on cafe menu change | Physical check + Uday confirms |
| 18 | JWT session survives 6h shift without kicking staff | Manual shift simulation |

**Criteria 1-15 are P0 (blocker). 16-18 are P1 (within first week of operation).**

---

*Written directly due to agent API overload.*
