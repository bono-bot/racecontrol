# Phase 343 ↔ v47.0 Coordination Handoff

**From:** James session 2026-04-09 (racecontrol monorepo, James .27)
**To:** The session starting v47.0 Admin Dashboard Venue-Ready Hardening
**Commit to pull:** (see git log for this file's first commit hash)

---

## TL;DR for the v47.0 session

1. **Pull this commit.** You will get Phase 343 scaffolding (5 files under `.planning/phases/343-staff-pin-hardening/`). **Do not modify Phase 343 files** — they are owned by this session. Read them as prerequisite context for your v47.0 work.

2. **Phase 343 is a precursor to v47.0**, not part of it. Phase 343 ships in the racecontrol monorepo. v47.0 ships in `racingpoint-admin` (separate repo per your scope note).

3. **Phase 343 has 3 active plans and 1 superseded plan.** The superseded one (`343-03-PLAN.md`) is explicitly handed off to your session — its contents describe an admin dashboard UI that belongs in v47.0. Use it as design context.

4. **v47.0 should start at Phase 344.** The phase number sequence is clean: 343 = racecontrol backend PIN hardening, 344+ = admin dashboard v47.0 work.

5. **There is overlap between Phase 343 findings and your v47.0 scope.** Section "Audit findings feeding v47.0" below lists every finding from today's session that maps to your 10 feature areas. You don't need to re-discover them.

---

## Incident context — why Phase 343 exists

On 2026-04-09, a staff member (Chavan Vishal) reported that PIN `0009` was invalid on the kiosk. Investigation found:

1. **Root cause A — data drift:** Vishal's active row in `staff_members` had PIN `2003`, not `0009`. A deploy-staging script (`set-vishal-pin.js`) referenced `0009` but had never been successfully run against the current DB state.

2. **Root cause B — duplicate rows:** Two `Chavan Vishal` rows existed in venue `staff_members`, one inactive (`staff_e1690f8a` PIN `8772`) and one active (`staff_463cf400` PIN `2003`). The `create_staff` uniqueness check only scans `is_active=1` rows, so the orphan inactive dup was invisible.

3. **Root cause C — silent sync revert (THIS IS THE BIG ONE):** The fix session initially updated Vishal's PIN on the venue API (`PUT /api/v1/staff/staff_463cf400 {pin:"0009"}`) → HTTP 200 → immediate `validate-pin(0009)` returned 200 → the change was declared working. **30 seconds later, cloud sync overwrote the venue row back to `2003`.** Zero error, zero alert, zero indication.

4. **Root cause D — cloud was ALSO stale:** Even after updating cloud via `PUT /staff/staff_463cf400 {pin:"0009"}` on Bono VPS, cloud legacy rows `staff-uday` (PIN 4149, inactive) and `staff-admin` (PIN 2198, inactive) remained — cruft from an earlier bootstrap that was never in venue.

5. **Root cause E — no admin UI:** PIN changes were only possible via curl/sqlite3/deploy-staging scripts. No dashboard. Staff cannot self-service. Scripts hardcode stale admin PINs (`130424`, which is not the current admin PIN `8141`).

**Vishal's PIN is now `0009`** — verified working on venue + cloud, data cleaned up. But the class of bug that caused the 30-second silent revert still exists. Phase 343 is the code-level fix for that class.

---

## Phase 343 scope (what this session is building — racecontrol backend)

**Active plans:**

| Plan | Wave | Purpose | Files touched |
|---|---|---|---|
| **343-01** | 1 | Cloud-authority guard: venue returns 409 Conflict on PUT/POST/DELETE to `/staff` endpoints when staff_members is cloud-authoritative. Prevents the silent revert. | `crates/racecontrol/src/config.rs`, `crates/racecontrol/src/api/routes.rs` |
| **343-02** | 2 | Post-write verify: immediate row re-read + delayed verify (sync_interval+5s) with `alert_incidents` row on mismatch. Catches any future silent revert even if 343-01 is bypassed. | `crates/racecontrol/src/api/routes.rs`, `crates/racecontrol/src/db/mod.rs` |
| **343-04** | 4 | e2e-regression spec `staff-pin-lifecycle.spec.ts` that creates/changes/waits 70s/revalidates/deletes — the ONE test that would have caught the Vishal bug at commit time. | `e2e-regression/tests/10-auth/staff-pin-lifecycle.spec.ts`, `e2e-regression/fixtures/auth.ts` |

**Superseded plan (handed off to v47.0):**

| Plan | Status | What it describes | Where it should live |
|---|---|---|---|
| **343-03** | SUPERSEDED | Admin dashboard `/admin/staff` page + change-pin orchestration endpoint + sync/pull-now helper | v47.0 Phase 344+ in `racingpoint-admin` repo |

The 343-03 file is still committed for your reference — read it as design context. It contains:
- UI mockup (Next.js page + modal + staged progress states)
- Next.js API route for the admin-side fetch
- Backend endpoint contract: `POST /api/v1/admin/staff/{id}/change-pin`
- Backend endpoint contract: `POST /api/v1/sync/pull-now`
- Orchestration pseudocode (cloud write → cloud verify → venue sync → venue verify → return both booleans)
- 10-step manual smoke test for the deploy session

**The endpoint contracts in 343-03 are yours to implement** — Phase 343 will NOT add `change_staff_pin_safe` or `sync_pull_now` endpoints. v47.0 should add them as part of Phase 344+ work.

---

## Audit findings from 2026-04-09 that feed v47.0

Today's session ran an Explore agent + DB audit that produced 12 code findings + 6 data findings. Many map directly to your v47.0 feature areas. Use this as a pre-populated backlog.

### Feature 1 — Unbreakable deploys

- **C3 (P1)** — `deploy-staging/set-pin.js`, `set-pin.ps1`, `set-pin.py`, `set-pin-node.js`, `update-pin.js`, `set-vishal-pin.js` all hardcode stale admin PIN `130424` (real is `8141`). If any of these are still used in the admin deploy path, they silently 401. Archive or fix. Suggested: replace with single admin dashboard flow, delete the 6 scripts.

### Feature 2 — Backend resilience

- **C5 (P1)** — `crates/racecontrol/src/config.rs:1152+` has hardcoded default JWT secret literal `"racingpoint-jwt-change-me-in-production"`. `resolve_jwt_secret()` rejects it but the string is in compiled artifacts. Remove the default; force explicit env/TOML or halt startup.
- **C6 (P1)** — `routes.rs:9023-9026` webhook HMAC verification is conditional on `payment_webhook_secret` being set. If unset, endpoint accepts unsigned webhooks silently. Halt startup instead.
- **C10 (P2)** — rc-agent openrouter.rs has no key validity check at boot. Dead/revoked key = silent AI failure. Add `GET /api/v1/models` check with the key.

### Feature 3 — Single source of truth enforcement

- **C8 (P2)** — Three identity sources exist: `staff_members`, `drivers.is_employee`, `employees` (empty table). `auth/mod.rs:1815-1820` reads only from `drivers.is_employee`. No sync. Pick one authoritative source, deprecate the others.
- **D4 (P2)** — `employees` table in venue DB is **empty** (zero rows). Dead table. Candidate for drop migration.
- **D5 (P2)** — `drivers` table has dup names: 4× `l1-parent-*`, 19× `test_only audit driver`, 21× `unknown`. Unique index on `(name, dob) WHERE registration_completed=1` protects real drivers, but unfinished registrations create dup clutter. Consider cleanup cron.

### Feature 4 — Local↔cloud sync contract

- **C1 (P0)** — Staff PIN stored + compared as **plaintext** in `staff_members` (`WHERE pin = ?` at routes.rs:12613). Drivers use `pin_hash` (Argon2). Inconsistent. DB leak exposes every staff PIN. **Separate migration phase recommended** — depends on Phase 343 shipping first.
- **C2 (P0)** — `X-Relay-Secret` sent over **plain HTTP** (`api_url = "http://100.70.177.44:8080"`). LAN-sniffable. Should be HTTPS with mTLS.
- **D1 (P0)** — **FIXED 2026-04-09** — cloud DB had Vishal's PIN out of sync with venue at one point. This is the class of bug Phase 343 addresses.
- **D6 (P2)** — `deploy-staging/racecontrol.toml` + `racecontrol-server.toml` both hardcode `terminal_pin = "261121"` + `terminal_secret = "rp-terminal-2026"`. Risk if wrong toml is deployed.

### Feature 5 — Live /api/health + alerting

Phase 343 Plan 02 adds `alert_incidents` row on PIN-verify failure. v47.0 can extend this to WhatsApp alert via comms-link watchdog (Phase 343 has a TODO comment pointing at `whatsapp_alerter.rs` but doesn't wire it — out of scope for Phase 343). **Recommended v47.0 work:** implement the WhatsApp alert integration, add per-subsystem health probes.

### Feature 6 — Downstream propagation tests

Phase 343 Plan 04 adds one E2E test (`staff-pin-lifecycle.spec.ts`) that waits 70s for sync and re-validates. Use this as a template for your Playwright contract tests (feature 6). The critical insight: **any test that validates cross-boundary behavior MUST include a wait that exceeds the sync interval.** Unit tests can't catch silent-revert bugs.

### Feature 7 — Auth resilience

- **C4 (P1)** — `racingpoint-admin/src/app/api/auth/login/route.ts` has per-IP lockout only, no per-staff-id lockout tracking. Attacker rotating IPs brute-forces without the per-user counter incrementing.
- **C7 (P2)** — `auth/admin.rs:26-89` admin lockout state is in-memory LazyLock. `persist_lockout_to_db()` defined but commented "MMA-WIRED — future work." Restart wipes the counter. Implement the persist call.
- **D2 (P1) — FIXED 2026-04-09** — Cloud legacy rows `staff-uday` + `staff-admin` deleted this session.
- **D3 (P1) — FIXED 2026-04-09** — Venue orphan dup `staff_e1690f8a` deleted this session.

### Feature 8 — Data durability

- **C9 (P2)** — `crates/rc-sentry-ai/src/config.rs:366-370` — NVR password falls back to empty string with WARN if env unset. Cameras silently auth-fail. Halt startup instead.

### Feature 9 — UI hardening

- Phase 343 Plan 03 (superseded) has UI design notes: staged progress states (writing → syncing → verifying → success/error), role-gated page (superadmin/manager only), never display plaintext PINs even redacted. Reuse.

### Feature 10 — Runbook + staff training

- Recommended runbook addition: "**How to change a staff PIN**" — one-pager for Uday. Should say: "Use the Admin Dashboard `/admin/staff` page. Do NOT use curl or sqlite3. Do NOT use deploy-staging/*.js scripts (stale). If the dashboard is down, contact James."
- Recommended incident log entry format: PIN change = `staff_id | old_pin_redacted | new_pin_redacted | changed_by | timestamp | cloud_verified | venue_verified | correlation_id`.

---

## Dependency order — what ships when

```
Phase 343 (racecontrol backend)      v47.0 (racingpoint-admin)
────────────────────────────         ──────────────────────────
Plan 01: 409 guard           ──┐
Plan 02: post-write verify   ──┼──► Phase 344+: Admin /admin/staff page
Plan 04: e2e regression      ──┘    (consumes change_staff_pin_safe,
                                     consumes sync/pull-now)
                                     (adds WhatsApp alert)
                                     (adds per-subsystem health)
                                     (adds Playwright contract tests)
                                     (9 other feature areas)
```

**Phase 343 must ship first.** v47.0's admin PIN-change flow depends on the 409 guard being in place. If v47.0 ships first, the admin dashboard will fight with the venue API the same way the Vishal curl fix did.

---

## Phase 343 plan files in this commit

- `343-CONTEXT.md` — Full domain + decisions + incident evidence + v47.0 relationship
- `343-01-PLAN.md` — Cloud-authority guard (3 tasks, cargo tests)
- `343-02-PLAN.md` — Post-write verify (4 tasks, cargo tests)
- `343-03-PLAN.md` — **SUPERSEDED** — admin dashboard work, design reference for v47.0
- `343-04-PLAN.md` — E2E regression spec (3 tasks, Playwright)
- `343-COORDINATION.md` — this file

**Plan numbering:** Plans are numbered 01, 02, 03, 04. Plan 03 stays numbered 03 even though superseded — renumbering would break cross-references in CONTEXT.md and this file. Plan 04 stays as Plan 04.

---

## Things Phase 343 does NOT do

Do not assume Phase 343 fixes any of these — they are explicitly deferred:

- Plaintext → Argon2 migration for `staff_members.pin` (finding C1) — separate phase, depends on 343 shipping
- HTTPS + mTLS for Bono relay (finding C2) — separate phase, needs cert setup
- Per-staff-id admin lockout tracking (finding C4) — v47.0 work
- Admin lockout DB persistence (finding C7) — v47.0 work
- NVR password startup validation (finding C9) — separate rc-sentry-ai phase
- OpenRouter key bootstrap validation (finding C10) — separate rc-agent phase
- Cafe menu proxy to racecontrol (v47.0 feature 3) — v47.0 work
- Litestream replication (v47.0 feature 4) — v47.0 work
- WhatsApp alert wiring (v47.0 feature 5) — v47.0 work, Phase 343 Plan 02 has a TODO comment marking the integration point
- Playwright contract tests for admin → POS propagation (v47.0 feature 6) — v47.0 work
- Daily sqlite3 .backup cron (v47.0 feature 8) — v47.0 work

---

## Action items for the v47.0 session

When you pull this commit:

1. **Read `343-CONTEXT.md`** to understand what Phase 343 does and why
2. **Read `343-03-PLAN.md`** (superseded) — this is your design starting point for admin PIN change UI. Copy the contracts, adapt the wave sequencing.
3. **Create v47.0 scaffolding:** new section in ROADMAP.md, new `.planning/milestones/v47.0-REQUIREMENTS.md`, new `.planning/milestones/v47.0-ROADMAP.md`. Phase 343 does NOT touch ROADMAP.md — I left it clean for you.
4. **Number your first phase 344.** The sequence is: 343 (racecontrol backend) → 344+ (v47.0 admin dashboard).
5. **Decide ship order:** if 343 hasn't shipped by the time v47.0 starts execution, v47.0 Phase 344 (PIN change UI) must wait. Other v47.0 features (deploys, resilience, sync contract) can proceed independently.
6. **Read the "Audit findings feeding v47.0" section above** — 12 code findings + 6 data findings pre-mapped to your 10 feature areas. You don't need to re-discover them.
7. **Do NOT modify files under `.planning/phases/343-*/`** — they are Phase 343's artifacts. If you find a mistake, leave a note in this COORDINATION file and I (or future-James) will fix in a follow-up commit.

---

## Contact

If the v47.0 session has questions or finds errors in this handoff, append to `comms-link/INBOX.md` with subject `v47.0 ← Phase 343 coordination`. I check INBOX at session start.

— James (racecontrol monorepo session, 2026-04-09)
