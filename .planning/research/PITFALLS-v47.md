# v47.0 Pitfalls Research

**Scope:** Ruthless enumeration of what can go wrong when ADDING 11 hardening features to an existing Next.js 15 admin dashboard deployed to both a Windows venue (`.23:3201`) AND a Linux cloud VPS (Bono).

**Source material:**
- Phase 343 coordination handoff (`.planning/phases/343-staff-pin-hardening/343-COORDINATION.md`)
- `feedback_deploy_all_lessons.md` (10 server + 5 fleet failure modes)
- `feedback_permanence_gate_data_file_fixes.md`
- `CLAUDE.md` deploy, security, debugging rules (50+ lessons)
- Web search 2025: Litestream Windows, better-sqlite3 Node 24 prebuild, Next.js 15 standalone env
- Known facts: Server .23 runs Node v24.14.0, James .27 runs Node v22.22.0, venue opening imminent

**Severity rubric:**
- **P0** = ship-blocker. Venue-breaking if hit during/after v47.0 deploy. Must have prevention before execution starts.
- **P1** = high-impact. Causes staff/customer-visible regression, but recoverable within a shift.
- **P2** = nice-to-have guard. Degraded state, no customer harm.

---

## P0 Pitfalls (ship-blockers)

### PITFALL 1: Phase 343 ships AFTER v47.0 → silent PIN revert returns

- **Scenario:** v47.0's `/admin/staff` change-pin UI ships before Phase 343 Plans 01+02 are deployed on venue+cloud racecontrol. Admin writes PIN to venue → venue sync pulls cloud row → overwrites → admin UI shows success, kiosk rejects PIN 30 seconds later. Exact Vishal incident, now with a "fixed" claim in the LOGBOOK.
- **Warning sign:** `curl -s http://192.168.31.23:8080/api/v1/health | jq .build_id` does NOT include the Phase 343 merge commit. Or: v47.0 phase plan references `change_staff_pin_safe` / `sync/pull-now` endpoints that return 404 on venue.
- **Prevention:**
  1. **Feature flag the admin PIN UI.** Wrap the `/admin/staff` page and `POST /api/admin/staff/:id/change-pin` route behind `FEATURE_STAFF_PIN_UI` env var, default OFF.
  2. **Pre-flight deploy gate:** Phase 344 (PIN UI) `PLAN.md` executor MUST run `curl -s http://.23:8080/api/v1/admin/staff/test/change-pin -X POST -d '{}' | grep -v 404` and hard-fail if the endpoint is missing.
  3. **v47.0 ROADMAP.md must pin Phase 344 to `depends_on: phase-343-01, phase-343-02`** (document + enforced by a pre-deploy script that greps git log for those phase SUMMARY hashes).
- **Phase to address:** v47.0 Phase 344 (PIN UI — MUST be last non-trivial phase) + pre-work flag wiring.

### PITFALL 2: Next.js standalone `.env.local` NOT loaded → auth/DB URLs silently fall back to localhost

- **Scenario:** Admin is built via `next build` with `output: 'standalone'`. The standalone `server.js` does NOT source `.env.local` at runtime (confirmed Next.js issue #46296, still unresolved in 15.x). Secrets/URLs come from `process.env` at Node start time. On venue, pm2/schtask launches `node server.js` without env → JWT secret defaults, DB path defaults, `NEXT_PUBLIC_API_URL` bakes in localhost at build time. Result: admin loads unstyled, auth uses dev secret, `/api/health` returns healthy while every DB-backed API 500s.
- **Warning sign:**
  - `curl .23:3201/api/health` returns 200 but `curl .23:3201/api/staff` returns 500.
  - pm2 `env` for `racingpoint-admin` process shows missing `DATABASE_URL` / `JWT_SECRET`.
  - Static files load but WS/data hooks show localhost in browser devtools network tab.
- **Prevention:**
  1. **Env loader wrapper script** — `start-admin.bat` (Windows) and pm2 ecosystem config (Linux) MUST `set`/`export` every required var explicitly from a sourced `.env.production` BEFORE `node server.js`. Never rely on Next.js loading it.
  2. **Startup fail-fast:** add a 20-line `preflight.js` that runs before `server.js`, asserts `DATABASE_URL`, `JWT_SECRET`, `ADMIN_API_URL`, `COMMS_PSK`, `RACECONTROL_URL` all exist, process.exit(1) otherwise. Rejects the "dev secret fallback" class outright.
  3. **Grep rule:** `grep -rn "process.env" src/` list in CGP G4 — every var must have a preflight assert.
  4. **Deploy verification:** after restart, curl ONE authenticated DB-backed endpoint, not `/api/health`. Health probe that only pings the HTTP listener is the exact lie pattern from PITFALL 11.
- **Phase to address:** v47.0 health-probe phase + deploy-script phase. Both must land before anything else touches the admin.

### PITFALL 3: better-sqlite3 ABI break on Node 24 (server .23) during v47.0 rebuild

- **Scenario:** Server .23 runs Node v24.14.0. `better-sqlite3@11.x` has NO prebuilt N-API 137 binaries as of 2025-2026 (GH #1384, #1376). `npm install` falls back to node-gyp → requires C++20 toolchain + Python + VS Build Tools. On a fresh clone or CI path, this fails with `'CopyablePersistentTraits' is not a member of 'v8'`. Admin deploy bricks mid-shift with "Error: The module was compiled against a different Node.js version / NODE_MODULE_VERSION 127 vs 137."
- **Warning sign:**
  - `node -e "require('better-sqlite3')"` on .23 throws ABI mismatch after `git pull` + `npm ci`.
  - `npm ls better-sqlite3` shows version change across commits in the deploy range.
  - `ls node_modules/better-sqlite3/build/Release/` empty or has a `.node` built for wrong ABI.
- **Prevention:**
  1. **Pin Node to v22 LTS on server .23 BEFORE v47.0 execution.** Already a known-blocker in CLAUDE.md "Current Blockers". Schedule the downgrade as a pre-work task, not as part of v47.0 — don't bundle infra change with feature ship.
  2. **Pin better-sqlite3** to a version with prebuilt Node 22 N-API binaries (11.3.x+ confirmed, verify with `npm view better-sqlite3 dist-tags`).
  3. **Commit `node_modules/better-sqlite3/build/Release/*.node`** to deploy-staging as a pre-built artifact. On deploy, unpack — never run `npm rebuild` inside the deploy window.
  4. **Deploy script asserts Node version:** `node -v | grep -q '^v22\.' || exit 1`. Fail before any git pull.
  5. **Cache `node_modules` with a lockfile+node-version hash key** — rebuild only when either changes.
- **Phase to address:** Pre-work (Node downgrade on .23) + v47.0 deploy-script phase.

### PITFALL 4: Admin deploy tar/extract path mismatch (admin/ vs admin/.next/standalone/)

- **Scenario:** `feedback_admin_deploy_path.md` already documents this: admin tar extracts to `admin/` root, NOT `admin/.next/standalone/`. v47.0 adds a new deploy-script; if the script writer assumes standalone-layout, files land in the wrong directory, old admin keeps running, "deployed" hash is a lie.
- **Warning sign:** post-deploy curl to `.23:3201/api/build-info` returns the OLD build hash while `ls` on .23 shows new files.
- **Prevention:** Deploy script copies ENTIRE `admin/` repo root (or uses git pull + build-in-place), not just `.next/standalone/`. Post-deploy verification hits a `/api/build-info` endpoint (new in v47.0) that reads git commit from a baked file — not from `process.env` (which would regress to PITFALL 2).
- **Phase to address:** v47.0 deploy-script phase. MUST cite `feedback_admin_deploy_path.md` in PLAN.md.

### PITFALL 5: Cafe menu cutover causes data loss mid-shift

- **Scenario:** v47.0 Feature 3 rewrites cafe menu from `admin.db.menu_items` → proxy to `racecontrol.db.cafe_items`. Field names drift (`price_paise` vs `price`, `name` vs `item_name`, `is_available` vs `enabled`). During cutover, kiosk+POS still read old paths, admin writes new paths, or vice versa. Customer orders a ₹50 drink, charged ₹5000 because a paise/rupee field was misinterpreted.
- **Warning sign:**
  - Any kiosk cafe order shows price ≠ price on admin UI.
  - Contract test for `/api/cafe/items` shape differs between admin and racecontrol.
  - Grep for `menu_items` in kiosk+POS+web returns hits after admin cutover.
- **Prevention:**
  1. **Schema diff doc in PLAN.md:** side-by-side field table `admin.menu_items` vs `racecontrol.cafe_items` — NO migration allowed until every field is mapped.
  2. **Dual-read, NEVER dual-write.** Admin reads from racecontrol (new source of truth) via proxy; writes ONLY to racecontrol. Kill admin.db.menu_items table writes at cutover — do not leave dual-write ambiguity.
  3. **Zod/contract-test schema assertion** on the proxy response — test passes on CI before deploy.
  4. **Cutover only during explicit maintenance window** (venue closed, announced to Uday).
  5. **Pre-cutover snapshot:** `sqlite3 admin.db ".backup admin-pre-cafe-migration.db"` stored off-machine.
- **Phase to address:** v47.0 cafe-proxy phase. Hard dependency: racecontrol `cafe_items` table schema frozen BEFORE phase starts.

### PITFALL 6: Plaintext PIN UI makes the plaintext problem WORSE

- **Scenario:** C1 (plaintext PINs in `staff_members.pin`) is deferred. v47.0 adds a NEW change-pin UI that reads the plaintext PIN out of the DB to display/edit, or logs it in Next.js access logs, or ships it to the browser in an API response, or leaves it in React state/devtools. The attack surface grows with every new caller of the plaintext column.
- **Warning sign:**
  - Grep `src/app/api/admin/staff/` for any handler returning a field named `pin` (not `pin_hash`) in the JSON response body.
  - Browser devtools → Network → any staff API response contains a 4-digit number.
  - Next.js server logs (`.next/server/...`) contain PIN values in query strings or body.
- **Prevention:**
  1. **Hard rule in 344 PLAN.md:** the admin change-pin API NEVER reads the plaintext PIN. It only WRITES a new one. No `GET /admin/staff/:id/pin` endpoint. Ever.
  2. **Redact middleware on the admin API layer:** any JSON response containing a key matching `/^pin$/` has the value replaced with `"****"`. One place, tested.
  3. **Log scrubber:** Next.js access log format string must exclude request bodies for `/admin/staff/*/change-pin` routes.
  4. **Add `(pin)` to pre-commit secret scanner** — any new code that reads `staff_members.pin` in a SELECT triggers a review gate.
- **Phase to address:** v47.0 Phase 344 (PIN UI) + cross-cutting log-redaction phase.

### PITFALL 7: Venue opening day deploy — no rollback possible

- **Scenario:** Venue opens. Customers arrive. Staff uses admin. v47.0 deploy introduces regression (any of 1-6 above, or 8-18 below). Rolling back requires rebuild from prior commit + restart + possibly DB schema rollback. Meanwhile Uday is fielding customer complaints in real-time.
- **Warning sign:** Deploy window inside business hours. No `admin-prev/` directory on .23. No pre-deploy DB backup. No tested rollback runbook.
- **Prevention:**
  1. **NO v47.0 deploys during business hours. Period.** Hook into existing `scripts/ist-now.sh check` deploy-window gate.
  2. **Two-stage rollout:** cloud (Bono VPS) first (non-customer-facing for staff), verify 24h, then venue.
  3. **Rollback artifact mandatory:** `admin-prev.tar.gz` staged before the new build extracts. Rollback = one command: `rm -rf admin/ && tar xzf admin-prev.tar.gz && schtasks /Run /TN StartAdmin`.
  4. **DB snapshot before every deploy:** `.backup admin-pre-v47-<phase>.db` + automated size check.
  5. **Pre-written rollback runbook** in `docs/RUNBOOKS/ROLLBACK-v47.md`, tested on cloud-staging once before venue deploy.
- **Phase to address:** v47.0 deploy-script phase + runbook phase.

### PITFALL 8: Litestream on Windows — NOT officially supported

- **Scenario:** v47.0 Feature 4 (Litestream replication) is deployed on venue .23 (Windows). Per Litestream project README: "Windows binaries are provided for convenience but Windows is NOT officially supported. Use at your own risk." Bugs in file locking, WAL handling, or path translation corrupt the venue DB at 2am during a sync. No official support channel.
- **Warning sign:**
  - Litestream Windows binary prints WARN or ERROR during first `litestream replicate` run.
  - `.db-wal` or `.db-shm` file grows unbounded.
  - First restore drill from S3 produces a corrupt DB or a DB out-of-sync with local.
- **Prevention:**
  1. **Do NOT run Litestream directly on Windows .23.** Two options:
     - **Option A (preferred):** Run Litestream on the CLOUD side (Linux), pulling from a shared network location or via a periodic `.backup` + scp/rsync push from .23 → Bono VPS → Litestream → S3.
     - **Option B:** Run Litestream inside WSL2 on .23. WSL2 file system interop with NTFS is slow and has known SQLite locking edge cases — NOT recommended for a DB under active writes.
     - **Option C (simplest for v47.0):** Skip Litestream on venue entirely for v47.0. Use scheduled `sqlite3 .backup` + scp to cloud + S3 from Linux. Add Litestream later when it becomes first-class on Windows.
  2. **If any Litestream variant is used: mandatory restore drill BEFORE venue opens** — pull last night's snapshot, restore to a scratch DB, run admin against it, verify staff/cafe/menu tables load. Document the drill in a runbook.
  3. **S3 cost guard:** set S3 bucket lifecycle policy (7-day transitions to IA, 30-day to Glacier) + CloudWatch budget alarm at $5/month. Litestream snapshot-every-10s on a write-heavy DB can surprise the bill.
  4. **Encryption at rest:** S3 SSE-KMS with customer key (`aws:kms`), not just SSE-S3. Litestream supports this via `AWS_SDK_LOAD_CONFIG` + `~/.aws/config`. Document in PLAN.md.
  5. **Schema migrations + Litestream:** Litestream replicates WAL frames. A schema migration is a normal SQL statement — it replicates fine. BUT: during a migration window, do NOT run `VACUUM` or `sqlite3_rekey` — both invalidate the shadow WAL chain and force a full snapshot. Document a migration checklist.
- **Phase to address:** v47.0 data-durability phase. MUST include an ADR (Architecture Decision Record) picking Option A/B/C before implementation.

---

## P1 Pitfalls (high-impact)

### PITFALL 9: Auth lockout cascades through shared POS IP

- **Scenario:** v47.0 Feature 7 adds per-IP AND per-staff-id lockout. POS (`100.95.211.1`) is a shared workstation — 5+ staff log into admin from it during a shift. Three bad attempts by one distracted staff → POS IP locked out → every other staff at POS now blocked. Uday himself gets locked out while walking around during peak because the SSO session on his phone shares a Tailscale egress.
- **Warning sign:**
  - Lockout table shows >1 staff_id under a single ip_address.
  - Admin login page returns 429 on a machine that never saw 3 failed attempts.
- **Prevention:**
  1. **Per-staff-id lockout PRIMARY, per-IP SECONDARY** — per-IP lockout threshold must be much higher (e.g., 20 failures in 5 min, not 3). Per-staff-id is the actual security guard.
  2. **Allowlist POS + Tailscale internal IPs** from per-IP lockout. Per-staff-id still applies.
  3. **Break-glass token** — a signed, single-use token (env var `ADMIN_BREAKGLASS_TOKEN`, rotated weekly, held by Uday in a text message from James) that bypasses lockout for ONE login. Logged prominently in `alert_incidents` with WhatsApp alert.
  4. **Lockout auto-clear after 15 min** — not permanent. User can retry without needing James.
  5. **Runbook card in physical venue:** "If locked out: call James / use break-glass token from the slip in the safe."
- **Phase to address:** v47.0 auth-hardening phase.

### PITFALL 10: Health probes that lie (the proxy-lie pattern)

- **Scenario:** v47.0 Feature 5 adds per-subsystem health. Executor copies the existing `/api/health` that returns `{ok: true}` regardless of DB state. Admin claims healthy while every DB-backed API 500s. This is EXACTLY the current bug. v47.0 enshrines it.
- **Warning sign:**
  - `curl /api/health` returns 200 but `curl /api/staff` returns 500.
  - The health handler's code does not `await` any DB query.
- **Prevention:**
  1. **Per-subsystem health response schema:** `{db: ok|fail, sync: ok|fail, racecontrol_proxy: ok|fail, comms_link: ok|fail, litestream: ok|fail|disabled, overall: ok|degraded|down}`. Overall = worst of components.
  2. **Each check MUST actually exercise the dependency:** db → `SELECT 1 FROM staff_members LIMIT 1`; sync → last_sync_at < 5min; proxy → `GET racecontrol/health` with 2s timeout; litestream → shadow WAL file mtime < 30s.
  3. **Health probes MUST NOT depend on the thing they probe.** The DB probe is a separate SQL call on a separate connection pool, not the shared pool that's already deadlocked if the app is broken.
  4. **Load guard:** health endpoint rate-limited to 1 req/sec per IP. Probes that cause load kill the service they're checking.
  5. **Contract test in Playwright:** Kill the DB, assert `/api/health` returns 503 + `db: fail`. Kill the sync, assert `sync: fail`. If either returns 200, CI fails.
- **Phase to address:** v47.0 health-probe phase (MUST land EARLY, other phases depend on it for deploy verification).

### PITFALL 11: WhatsApp alert storm during a cascading failure

- **Scenario:** v47.0 Feature 5 wires `alert_incidents` → WhatsApp via comms-link. Racecontrol dies → every admin endpoint that proxies to it fails → every failure writes an `alert_incidents` row → each row triggers a WhatsApp send → 500 messages in 60 seconds → comms-link rate limits → Meta throttles the number → real alerts are buried under duplicates → Uday mutes WhatsApp → next real alert missed.
- **Warning sign:**
  - comms-link relay logs show > 10 sends/min from admin source.
  - WhatsApp Business API returns 429.
  - Uday says "my phone is going crazy."
- **Prevention:**
  1. **Dedup window:** same `incident_key` (e.g., `admin.db.5xx`) alerts at most once per 10 minutes. Dedup key is `(source, severity, symptom_hash)`, NOT `(timestamp)`.
  2. **Rate cap:** max 5 WhatsApp messages per 5-minute window from admin. Overflow goes to a summary "+ N more alerts suppressed, see /admin/incidents".
  3. **Escalation tiers:**
     - Tier 1 (INFO, WARN): write to `alert_incidents`, no WhatsApp.
     - Tier 2 (ERROR): WhatsApp once per dedup window.
     - Tier 3 (CRITICAL, e.g., DB corruption, auth bypass): WhatsApp immediately, every time, bypass dedup.
  4. **Circuit breaker:** if > 50 alerts in 1 min → enter SILENT_MODE for 5 min, send ONE WhatsApp: "Admin alerter in circuit-breaker, check /admin/incidents."
  5. **Dry-run verification:** simulate a cascade in staging (kill racecontrol, watch admin's alerter behavior) BEFORE venue deploy.
- **Phase to address:** v47.0 WhatsApp-alerts phase. Tightly coupled with health-probe phase.

### PITFALL 12: Playwright contract tests with 70-second sync waits break parallelism

- **Scenario:** Phase 343 Plan 04 establishes a 70-second-wait template for cross-boundary tests. v47.0 adds 10+ contract tests following the same pattern. Running them in parallel (Playwright default workers=4) → 4 tests all writing to the same staff_id → flake. Running sequentially → 10 × 70s = 12 minutes per test run → CI timeout → dev disables tests → regression ships.
- **Warning sign:**
  - Test suite runtime > 10 min.
  - Flake rate > 5%.
  - Test output shows "Unique constraint failed" on staff_id or "row not found" after write.
- **Prevention:**
  1. **Per-test isolated staff IDs:** `staff_test_${crypto.randomUUID()}` — never a hardcoded `staff_test` name. Cleanup step at test end: DELETE by the UUID.
  2. **Dedicated test venue_id** (or test DB) — contract tests never touch production rows, even on a dev box. Use a Playwright global-setup that creates an isolated `admin.test.db`.
  3. **Parallelism-safe wait:** tests that must wait for sync use `test.describe.configure({ mode: 'serial' })` ONLY for the wait-heavy tests. Other tests run parallel.
  4. **Sync interval tuning for test mode:** expose sync interval as env var `SYNC_INTERVAL_SEC`. Tests set it to 2s, wait 5s. Production leaves default 30s.
  5. **CI timeout** raised to 20 min; per-test timeout 90s; flake retry = 1.
- **Phase to address:** v47.0 Playwright-contract-tests phase.

### PITFALL 13: Cross-subdomain cookie handling (venue vs cloud auth)

- **Scenario:** Venue admin at `.23:3201` (plain HTTP, no domain). Cloud admin at `admin.racingpoint.cloud`. Next.js auth cookies default to `HttpOnly; SameSite=Lax`. If the admin code is shared between environments but cookie config isn't environment-aware, cloud sets `Secure` flag (works) and venue sets `Secure` flag too (cookie rejected by browser on plain HTTP → perpetual login loop). OR: staff logs into cloud from a phone, cookie has `Domain=.racingpoint.cloud`, works. Same code on venue sets `Domain=.23` which is invalid → cookie rejected.
- **Warning sign:** DevTools → Application → Cookies shows the JWT cookie is missing after a 200 login response.
- **Prevention:**
  1. **Env-driven cookie config:** `COOKIE_SECURE=true` on cloud, `false` on venue. `COOKIE_DOMAIN` unset on venue (host-only cookie), set to `.racingpoint.cloud` on cloud.
  2. **Login flow test in Playwright runs against BOTH venue:3201 and cloud URL** in CI.
  3. **HTTPS on venue** — long-term fix. Self-signed cert + Tailscale MagicDNS. Add to v47.0 backlog but NOT as a v47.0 scope item (scope creep risk).
- **Phase to address:** v47.0 auth-hardening phase.

### PITFALL 14: Cloud-authority 409 UX disaster

- **Scenario:** Phase 343 Plan 01 makes venue return 409 on PUT to `/staff`. v47.0 admin UI, running on venue, hits this 409 when staff tries to change a PIN locally. UI shows "Conflict" in a red toast and bails. Staff retries 3×, gets locked out, calls James. OR: UI catches 409, silently retries on cloud without telling staff, cloud lags 30s, UI shows stale PIN, staff thinks change failed, changes again, creates a change/change loop.
- **Warning sign:** Admin UI error log shows 409 responses from `/api/admin/staff/*`. Staff support tickets mention "it said conflict then worked."
- **Prevention:**
  1. **Admin API layer catches 409 and routes the write to cloud-authority endpoint automatically.** UI never sees the 409 — it's transparent.
  2. **Progress states in UI** (per Plan 343-03 mockup): `writing → syncing → verifying → success`. Staff sees what's happening; no silent retries.
  3. **After write, UI polls venue read-endpoint every 2s for up to 90s** (> sync interval) and shows the verified state. Only then "success."
  4. **On 409 without admin-layer recovery:** UI shows actionable message: "Cloud sync pending. Change will apply in ~30s. Click to verify." Not "Conflict."
  5. **Playwright test:** trigger a 409, assert UI shows progress state, not error toast.
- **Phase to address:** v47.0 Phase 344 (PIN UI) + cross-cutting admin-API-layer phase.

### PITFALL 15: Deploy script written in bash works on Linux, breaks on Windows (or vice versa)

- **Scenario:** Deploy-script phase produces `deploy-admin.sh`. Dev tests it on WSL, ships. On venue .23 Git Bash it silently fails because `pm2` isn't installed (venue uses schtasks), `systemctl` doesn't exist, paths use forward slashes that .bat scripts break on, `node_modules` permissions differ. OR: dev writes `deploy-admin.ps1` for Windows, cloud Bono can't run it.
- **Warning sign:** Deploy script has ANY of: `systemctl`, `pm2 reload` (without "or schtasks"), `&&` chains with cmd.exe-incompatible syntax, `2>/dev/null`, `$(cmd)` subshells in a .bat file.
- **Prevention:**
  1. **TWO scripts, not one.** `deploy-admin-venue.bat` (Windows, uses schtasks + curl + cmd.exe-safe syntax) and `deploy-admin-cloud.sh` (Linux, uses pm2 + systemd). Shared manifest JSON file describes steps — each script interprets it for its OS.
  2. **Cross-reference `feedback_deploy_all_lessons.md` Part 4 #5** (`npm build scripts use bash-only syntax`) and Rule "Unix shell syntax in SSH remote commands to Windows" in CLAUDE.md. Both scripts must pass a linter that rejects the other OS's syntax.
  3. **Dry-run mode:** `deploy-admin-venue.bat --dry-run` and `--cloud.sh --dry-run` print every action without executing. Ran against a fresh VM before venue ship.
  4. **Deploy verification script is OS-neutral** — uses `curl` (present both places) and plain HTTP checks only.
- **Phase to address:** v47.0 deploy-script phase.

### PITFALL 16: Backup looks fine but restore fails (untested restore drill)

- **Scenario:** v47.0 Feature 8 adds daily `sqlite3 .backup admin-YYYY-MM-DD.db` to S3/cloud. Cron runs nightly. Files show up. Sizes look right. 3 months later a DB corruption event happens. Restore drill fails because: (a) the backup is of a locked DB (no WAL checkpoint first → restored file is partial), (b) schema version on the backup is older than the current code (migration required on restore), (c) file permissions prevent admin from opening the restored file, (d) the backup was of `admin.db` but not `admin.db-wal` — WAL mode requires both for a consistent restore.
- **Warning sign:** No evidence in LOGBOOK.md of a completed restore drill.
- **Prevention:**
  1. **Restore drill is a REQUIRED PHASE in v47.0.** Cannot ship v47.0 without a documented successful restore from S3/cloud backup to a scratch VM, admin booted against it, login + staff list + cafe menu all verified.
  2. **Backup procedure MUST use `sqlite3 .backup`**, not `cp admin.db`. The `.backup` command is WAL-safe and produces a consistent snapshot.
  3. **Backup includes schema version file alongside the DB:** `admin-2026-04-09.db` + `admin-2026-04-09.schema-version.txt` (reads from `PRAGMA user_version`). Restore script checks compat before extracting.
  4. **Quarterly restore drill** in LOGBOOK.md as a recurring item (add to gsd:plant-seed with a 90-day trigger).
  5. **Backup retention:** 7 daily + 4 weekly + 3 monthly. Old enough to recover from slow-creeping corruption, not so old S3 bill explodes.
- **Phase to address:** v47.0 data-durability phase. Restore drill is a GATE, not a checkbox.

### PITFALL 17: `outputFileTracingRoot` trap — static 404 fleet-wide

- **Scenario:** Already documented in CLAUDE.md as a historical bug. v47.0 touches `next.config.ts` for any reason (env var change, basePath fix, feature flag) → forgets to set `outputFileTracingRoot: path.join(__dirname)` → `required-server-files.json` has build-machine absolute path → deployed admin serves pages but all `/_next/static/*` → 404 → UI loads unstyled, no JS → every interaction broken. Health endpoint returns 200 the whole time.
- **Warning sign:** `curl -I .23:3201/_next/static/css/...` returns 404 while `curl .23:3201/` returns 200.
- **Prevention:**
  1. **`outputFileTracingRoot` assertion in CI:** build step runs, then a post-build script greps `.next/standalone/server.js` for `C:\Users\` or `/home/` absolute paths. If any found from a different machine than the CI runner → fail.
  2. **Deploy verification curl:** `curl -I -o /dev/null -w "%{http_code}" .23:3201/_next/static/css/<hashed>.css` — must return 200. Not just `/api/health`.
  3. **Lock `next.config.ts` behind a code-review gate** during v47.0: any PR touching it requires James review.
- **Phase to address:** Cross-cutting. Affects EVERY phase that rebuilds admin.

### PITFALL 18: pm2 `--update-env` forgotten after env var change

- **Scenario:** Cloud admin deploy: env var added to `ecosystem.config.js`, `pm2 reload racingpoint-admin` runs but without `--update-env` → process keeps the OLD env. New env var is unset inside the running process. Code path that depends on it silently fails. Admin half-works: old features OK, new v47.0 features 500.
- **Warning sign:** `pm2 env racingpoint-admin` doesn't show the new var despite it being in the config file on disk.
- **Prevention:**
  1. **Deploy script ALWAYS runs `pm2 reload racingpoint-admin --update-env`** — never just `pm2 reload`. Fail the script if `--update-env` is not present.
  2. **Post-deploy assertion:** `pm2 env racingpoint-admin | grep <new_var>` — fail if missing.
- **Phase to address:** v47.0 deploy-script phase (cloud script).

---

## P2 Pitfalls (nice-to-have guards)

### PITFALL 19: Existing Playwright tests assume stable schema

- **Scenario:** Existing e2e-regression tests (not v47.0's new contract tests) hardcode `staff_admin` PIN `8141` or assume specific columns. v47.0 adds new columns, migrates data, or changes seed fixtures → existing tests fail → dev adds `test.skip` → coverage erodes.
- **Warning sign:** Any `test.skip()` added in a v47.0 PR.
- **Prevention:** Feature-flag new admin features; keep old tests green until the flag flips. Forbid `test.skip` in PR review — require either fixing the test or marking it `test.fail` with a tracked issue.

### PITFALL 20: CGP H2 two-phase completion vs long deploys

- **Scenario:** H2 says fix in one message, verify behavior in the NEXT. v47.0 has 11 themes each with venue + cloud + Playwright verification. 11 × 2 = 22 message pairs minimum. Long deploys (Node downgrade, Litestream restore drill, S3 bucket setup) take hours — the "next message" verification is blocked by an external clock. Tempting shortcut: batch claims.
- **Warning sign:** Any "done" claim in the same message as the fix.
- **Prevention:**
  1. **Phase PLAN.md includes explicit wait-states:** "after committing, wait 15 min for pm2 restart + Playwright CI run, THEN verify in a new message."
  2. **If wait > 30 min, use `gsd:pause-work`** — create a handoff, resume in a fresh session. Don't try to "remember" across a long idle.
  3. **Evidence must match change domain** (CLAUDE.md feedback_verify_domain_match): venue change → curl venue; cloud change → curl cloud; BOTH for parity changes.

### PITFALL 21: Admin deploys break kiosk by proxy (menu, staff cache)

- **Scenario:** Kiosk (:3300) caches staff list + cafe menu at boot. Admin cutover to racecontrol-backed cafe items. Kiosk's cached menu is now stale relative to the new source of truth for 1 boot cycle. Customer orders from kiosk, item doesn't exist in racecontrol.cafe_items → POS 500 at charge time.
- **Warning sign:** `/api/cafe/items` returns different JSON from kiosk vs admin vs POS.
- **Prevention:** After cafe migration, force-restart kiosk + POS rc-agents so they re-fetch. Add a WS broadcast `CafeMenuInvalidate` that both consume.

### PITFALL 22: Fresh-CI clock / `Instant::now()` arithmetic

- **Scenario:** Referenced in memory (`feedback_instant_arithmetic_on_ci.md`). Not Node-specific but worth flagging for any Rust-side changes Phase 343 carries into v47.0 verification. Tests using `Instant::now() - Duration::from_secs(N)` panic on fresh VM boot.
- **Prevention:** Avoid in v47.0 Rust tests. Use `SystemTime` if a wallclock delta is needed.

### PITFALL 23: Data-file fix to dynamic-commands.json could re-poison during v47.0 deploy

- **Scenario:** v47.0 deploy script registers a new relay command (e.g., `admin_pull`) via `POST /relay/registry/register`. Per `feedback_permanence_gate_data_file_fixes.md`, the registry has no schema guard — if the args contain a Windows-absolute path from James's session, cloud restart reloads it and breaks.
- **Prevention:** Any `register` call in v47.0 deploy script MUST send platform-correct paths (no drive letters, no `C:\`), AND v47.0 should bundle a schema guard fix in `comms-link/shared/dynamic-registry.js` as pre-work.

### PITFALL 24: Venue ↔ cloud clock skew breaks sync-age health check

- **Scenario:** Venue .23 clock drifts 2 minutes ahead. Health probe compares `now - last_sync_at < 5min`. Sync is actually working (30s lag) but health fires false WARN because the timestamps are on different clocks.
- **Prevention:** `last_sync_at` comparison uses the server's own now, not wall-clock comparison across machines. OR: force NTP sync on .23 daily via schtasks.

---

## Cross-cutting: Pitfall Heatmap (by theme)

| v47.0 theme | P0 | P1 | P2 |
|---|---|---|---|
| **Litestream replication** | PITFALL 8 | — | — |
| **Cafe menu proxy** | PITFALL 5 | — | PITFALL 21 |
| **Per-subsystem health** | PITFALL 2 (env), 10 (proxy lie) | — | PITFALL 24 (clock skew) |
| **WhatsApp alerts** | — | PITFALL 11 | — |
| **Admin staff PIN UI** | PITFALL 1 (343 dep), 6 (plaintext) | PITFALL 14 (409 UX) | — |
| **Daily backups** | — | PITFALL 16 (restore drill) | — |
| **Playwright contract tests** | — | PITFALL 12 | PITFALL 19 |
| **Auth hardening** | — | PITFALL 9 (lockout), 13 (cookies) | — |
| **Deploy script** | PITFALL 3 (Node/ABI), 4 (tar path), 7 (rollback) | PITFALL 15 (dual-OS), 18 (pm2 env) | PITFALL 23 (relay registry) |
| **CGP/process** | — | PITFALL 20 | PITFALL 22 |
| **Next.js standalone gotchas** | PITFALL 2 (env) | PITFALL 17 (tracing root) | — |

---

## Pre-Flight Checklist — what MUST be true BEFORE v47.0 execution starts

Copy this into the v47.0 ROADMAP.md as a blocking gate. Every item must be CHECK before Phase 344 begins.

### Infrastructure pre-work
- [ ] **Server .23 Node downgraded from v24.14.0 to v22 LTS** (fixes PITFALL 3). Document in LOGBOOK.md with before/after `node -v`.
- [ ] **`better-sqlite3` pinned** in admin's `package.json` to a version with Node 22 N-API prebuilt binaries. `npm ci` on .23 succeeds without touching node-gyp.
- [ ] **Node version assertion** in deploy-admin-venue.bat — script exits 1 if `node -v` doesn't start `v22.`.
- [ ] **Phase 343 Plans 01 + 02 SHIPPED** (not just committed) to venue and cloud racecontrol. Verified via `curl .23:8080/api/v1/health | jq .build_id` matching the 343 merge hash. (Blocks PITFALL 1.)
- [ ] **Admin `admin-prev.tar.gz` rollback artifact** script exists and is tested. (Blocks PITFALL 7.)
- [ ] **Admin DB `.backup` script** exists, tested restore drill completed, documented in LOGBOOK. (Blocks PITFALL 16.)

### Schema + contract pre-work
- [ ] **Cafe schema mapping doc** committed: `admin.menu_items` ↔ `racecontrol.cafe_items` field-by-field table in `.planning/research/cafe-schema-diff.md`. (Blocks PITFALL 5.)
- [ ] **`racecontrol.cafe_items` schema frozen** — no more column changes until v47.0 cafe phase ships.
- [ ] **Admin `build-info` endpoint** defined with a baked-at-build git hash (NOT env var). Blocks PITFALL 4 verification drift.

### Security pre-work
- [ ] **Log redaction middleware** for `pin` field added to admin API layer. Tested with a mock response. (Blocks PITFALL 6.)
- [ ] **Break-glass token** env var defined, holder identified (Uday), rotation schedule documented. (Blocks PITFALL 9.)
- [ ] **Cookie config env-driven:** `COOKIE_SECURE`, `COOKIE_DOMAIN` set per environment. Login flow Playwright tested on venue + cloud URLs. (Blocks PITFALL 13.)

### Deploy + verification pre-work
- [ ] **Two deploy scripts exist:** `deploy-admin-venue.bat` + `deploy-admin-cloud.sh`. Both `--dry-run` tested. (Blocks PITFALL 15.)
- [ ] **Preflight env-asserter** (`preflight.js`) exists. Fails process start if any required env var missing. (Blocks PITFALL 2.)
- [ ] **`outputFileTracingRoot` set** in `next.config.ts`. CI grep for build-machine paths in `.next/standalone/server.js` passes. (Blocks PITFALL 17.)
- [ ] **Static-file 200 assertion** in deploy-verification script. (Blocks PITFALL 17.)
- [ ] **Health endpoint returns per-subsystem status** with real DB query, not `{ok:true}`. Contract test kills DB → asserts 503. (Blocks PITFALL 10.)
- [ ] **pm2 reload command includes `--update-env` flag** in cloud deploy script. (Blocks PITFALL 18.)

### Policy + process pre-work
- [ ] **Deploy window gate:** deploy-admin-venue.bat calls `scripts/ist-now.sh check` and refuses to run during business hours. (Blocks PITFALL 7.)
- [ ] **Litestream ADR** committed choosing Option A/B/C. If A or B: restore drill completed. If C: Litestream NOT in v47.0 scope, deferred to v48. (Blocks PITFALL 8.)
- [ ] **Alert dedup + rate cap** design doc committed. Circuit breaker threshold set. (Blocks PITFALL 11.)
- [ ] **Per-test UUID staff IDs** pattern established in contract test fixtures. (Blocks PITFALL 12.)
- [ ] **Rollback runbook** `docs/RUNBOOKS/ROLLBACK-v47.md` committed and tested on cloud-staging.
- [ ] **Feature flag `FEATURE_STAFF_PIN_UI`** defined and defaults OFF. (Blocks PITFALL 1.)

### CGP compliance pre-work
- [ ] **Every v47.0 phase PLAN.md includes a `deploy:` section** per DMP. H2 wait-state explicitly noted for long deploys. (Blocks PITFALL 20.)
- [ ] **Every phase PLAN.md includes the `feedback_*` references** it must read before executing (deploy lessons, admin deploy path, verify domain match).

---

## Out-of-scope callouts (explicitly NOT v47.0)

To prevent scope creep under venue-opening pressure:

- **HTTPS on venue** — deferred. Self-signed + Tailscale is a separate project.
- **Argon2 migration for `staff_members.pin`** — deferred (C1). Phase 343 must ship first; this is a separate post-v47.0 phase.
- **mTLS for Bono relay** — deferred (C2).
- **Full Litestream on Windows** — deferred (PITFALL 8 Option C). Daily `.backup` + scp is the v47.0 minimum.
- **Multi-venue support** — explicitly not v47.0.
- **Node 24 support** — blocked on better-sqlite3 upstream. Stay on Node 22 LTS.

Sources:
- [Litestream Windows support status](https://github.com/benbjohnson/litestream)
- [better-sqlite3 Node 24 prebuild issue #1384](https://github.com/WiseLibs/better-sqlite3/issues/1384)
- [better-sqlite3 Node 24 compat issue #1376](https://github.com/WiseLibs/better-sqlite3/issues/1376)
- [Next.js standalone .env.local not loaded #46296](https://github.com/vercel/next.js/issues/46296)
- [Next.js 15 env vars + Turbopack #75586](https://github.com/vercel/next.js/discussions/75586)
- [Next.js standalone NODE_ENV hardcoded #58294](https://github.com/vercel/next.js/issues/58294)
