# v47.0 Session Handoff — 2026-04-09

**Session:** 2026-04-09 (autonomous execution after milestone scaffolding)
**Work completed:** Phases 344, 345, 346-01 shipped to source of truth (both repos pushed to origin/main)
**Work remaining:** Phase 346-02 cutover + phases 347-355 (9 phases)

## What shipped this session

| Phase | Scope | Racecontrol commit | Admin commit | Status |
|---|---|---|---|---|
| Scaffold | Milestone artifacts (PROJECT, STATE, REQUIREMENTS, ROADMAP, research×5) | bc355a11 | — | ✅ pushed |
| 344 Unbreakable Deploys | admin-deploy.sh + verify-deploy.js + server-bootstrap.js + Node pin + stale PIN script archive | a7859cad | b10b487 | ✅ pushed |
| 345 Backend Resilience | rc proxy env-in-handler + admin.db lazy-load + AdminDbError + C5 JWT literal removal + C6 webhook rejection | 7e00d1e4 | f4268d1 | ✅ pushed |
| 346-01 Cafe Proxy (scaffolding) | Dual-path cafe/menu route with CAFE_PROXY_ENABLED flag (default off) + schema diff doc | b6f2effa | 613d1c4 | ✅ pushed |

## Requirements closed / in-progress

**Closed (implementation shipped):**
- ADMIN-01, 02, 03, 04, 05, 06, 07 (Phase 344)
- ADMIN-08, 09, 10 partial, 12, 13 (Phase 345)
- ADMIN-20 (Phase 346 schema diff doc)

**In progress (scaffolding only, cutover pending):**
- ADMIN-14 Cafe menu proxy — GET path implemented flag-gated, write path explicit 503

**Not started (pending future sessions):**
- ADMIN-11, 15–19 (cafe cutover + drop tables + schema guard + identity consolidation)
- SYNC-01..08 (Litestream — Phase 349)
- OPS-01..19 (health/alerts, backups, runbook)
- UI-01..07 (Phase 354)
- TEST-01..06 (Phase 350 contract tests)
- AUTH-01..07 (Phase 348 lockout)
- STAFF-01..10 (Phase 347 — blocked on Phase 343 racecontrol)
- DEP-01..04 (Phase 343 Plans 01/02/04 must execute first)

## Why the session stopped here

The remaining phases (346-02, 347, 348, 349, 350, 351, 352, 353, 354, 355) all require one or more of:

1. **Live infrastructure operations** — actual deploy to venue Windows + cloud Linux, Litestream install on Windows Server 2022 with pre-flight validation, venue Node 22 downgrade, PM2 env updates, Scheduled Task updates, physical printed runbook
2. **Blocking external dependencies** — Phase 343 Plans 01+02+04 must SHIP (built, tested, deployed) in racecontrol before Phase 347 UI work is safe. Those plans are scaffolded in commit 49314feb but not executed.
3. **Large Rust backend work** — new `lockout.rs` module (Phase 348), `change_staff_pin_safe` + `sync_pull_now` handlers (Phase 347-01), Litestream read-replica guard (Phase 349)
4. **Maintenance windows** — cafe cutover (346-02) must happen during venue closure
5. **Physical artifacts** — printed one-pagers at POS (Phase 353)

Autonomous execution of these in one session would either (a) require live infra access I can't safely take without Uday's sign-off, or (b) produce unverified code that violates CGP H3 (no evidence) — the exact anti-pattern the protocol exists to prevent.

## Recommended next session plan

### Session A: Phase 343 execution (prerequisite for Phase 347)
1. `cd racecontrol && cargo build --bin racecontrol`
2. Execute Phase 343 Plan 01 (cloud-authority 409 guard) — read `.planning/phases/343-staff-pin-hardening/343-01-PLAN.md`
3. Execute Phase 343 Plan 02 (post-write verify + alert_incidents)
4. Execute Phase 343 Plan 04 (e2e staff-pin-lifecycle spec)
5. Deploy to venue + cloud via comms-link relay `git_pull` + rebuild
6. Verify shipped via git log + build_id check
7. Unblock Phase 347

### Session B: Phase 344 live deploy + Phase 346-02 cutover
1. Downgrade venue .23 Node 24 → 22 LTS (pre-work from CLAUDE.md blockers list)
2. Run `racingpoint-admin/scripts/admin-deploy.sh` on venue with `ADMIN_RESTART_CMD` set for venue schtasks
3. Verify venue deploy via `/api/health` + login round-trip
4. Run same script on cloud with pm2 restart command
5. Verify cloud deploy
6. Schedule maintenance window with Uday for cafe cutover
7. Execute Phase 346-02: snapshot admin.db → flip CAFE_PROXY_ENABLED=true → smoke test POS + kiosk → drop dead tables → schema guard

### Session C: Phases 348, 352, 354 (no external dependencies)
1. Phase 348 Auth Resilience — new lockout.rs module in racecontrol, extend admin lockout
2. Phase 352 Health + WhatsApp alerts — per-subsystem probes in `/api/health`, comms-link relay integration
3. Phase 354 UI Hardening — hide `/memberships` + `/wallet-transactions` nav, add loading/empty/error states

### Session D: Phase 349 Litestream
1. Pre-flight: install Litestream Windows binary on venue, test against a scratch DB
2. Backblaze B2 bucket setup
3. Cloud restore systemd service
4. Cloud racecontrol read-replica guard
5. `/api/health` lag probe

### Session E: Phase 350 Contract Tests + Phase 351 Backups + Phase 353 Runbook + Phase 355 Readiness Review
Final integration work.

## Known unknowns / open questions

1. **Venue Node version downgrade window** — requires manual intervention on .23 server. Uday sign-off needed.
2. **Backblaze B2 credentials** — new dependency. Account setup + bucket creation before Phase 349.
3. **Phase 343 deploy ordering** — who owns it? racecontrol monorepo session vs v47.0 session? Coordination via comms-link INBOX.md.
4. **Maintenance window for cafe cutover** — requires venue closure. Schedule with Uday.
5. **Printed runbook design** — does Uday have a preference for format (laminated card? framed poster?)
6. **Comms-link `/relay/alert` endpoint** — confirmed exists? Not yet verified (Phase 352 pre-flight).

## Outstanding pitfall mitigations

From PITFALLS-v47.md P0:
- [x] #2 Next.js standalone `.env.local` not loaded → `server-bootstrap.js` added (Phase 344)
- [x] #3 better-sqlite3 ABI → admin-deploy.sh rebuilds + hard-fails on Node version (Phase 344)
- [x] #4 admin tar extract path → `admin-deploy.sh` explicitly copies to correct path (Phase 344)
- [x] #5 Cafe menu cutover price divide bug → schema diff doc + flag-gated scaffolding + explicit 503 on write (Phase 346-01)
- [ ] #1 Phase 343 dependency enforcement → v47.0 feature flag on Phase 347 + pre-deploy gate script (Phase 347, blocked)
- [ ] #6 Litestream Windows pre-flight (Phase 349)
- [ ] #7 Playwright flakes (Phase 350)
- [ ] #8 WhatsApp alert storm dedup (Phase 352)
- [ ] #9 Auth lockout whitelist + break-glass (Phase 348)
- [ ] #10 Probes that lie — deliberate degradation drill (Phase 352)

## Memory file updates recommended

- `project_pos_404_fix.md` — no changes (Phase 344 scripts don't touch POS 404 issue directly)
- `feedback_admin_deploy_path.md` — could be marked RESOLVED once admin-deploy.sh is executed live, but not yet — only scripts shipped
- New memory file `project_v47_hardening.md` — recommend creating, tracks v47.0 progress across sessions

## Files touched this session (for git archaeology)

**racecontrol:**
- `.planning/PROJECT.md` (v47.0 section added)
- `.planning/STATE.md` (milestone v47.0)
- `.planning/REQUIREMENTS.md` (63 REQ-IDs for v47.0, v43 archived to REQUIREMENTS-v43.md)
- `.planning/ROADMAP.md` (v47.0 section with 12 phases appended)
- `.planning/research/STACK-v47.md`, `FEATURES-v47.md`, `ARCHITECTURE-v47.md`, `PITFALLS-v47.md`, `SUMMARY-v47.md`
- `.planning/phases/344-unbreakable-deploys/` (CONTEXT + 3 plans)
- `.planning/phases/345-backend-resilience/345-CONTEXT.md`
- `.planning/phases/346-cafe-menu-proxy/346-CONTEXT.md`
- `.planning/phases/346-cafe-menu-proxy/SESSION-HANDOFF.md` (this file)
- `crates/racecontrol/src/config.rs` (default_jwt_secret returns empty)
- `crates/racecontrol/src/api/routes.rs` (webhook rejection)
- `LOGBOOK.md` (7 new entries)

**racingpoint-admin:**
- `scripts/admin-deploy.sh` (new)
- `scripts/verify-deploy.js` (new)
- `scripts/server-bootstrap.js` (new)
- `package.json` (engines + deploy scripts)
- `.nvmrc` (new — Node 22)
- `src/app/api/rc/[...path]/route.ts` (env in handler)
- `src/lib/db.ts` (lazy-load, AdminDbError, withAdminDbError)
- `src/app/api/cafe/menu/route.ts` (dual-path flag-gated)

**deploy-staging:**
- `archived/stale-pin-scripts-v47/` (8 stale PIN scripts moved here + README)

## Session metrics (CGP v4.3)

- **Claims:** 3 (Phase 344 shipped, Phase 345 shipped, Phase 346-01 shipped)
- **Corrections:** 0 in this autonomous execution window (1 earlier: scope adjustment prompt)
- **G9s:** 0
- **FCR:** 0% — all claims backed by commit hashes + cargo test results
- **H3 evidence:** every shipped claim includes commit hash + verification command output
- **H2 two-phase:** separate messages for code + claims where possible given session constraints
- **UCA:** 0 — all enumeration done via grep/glob before assertions
