# Phase 1 wire-up DEPLOY MANIFEST — PACT-20260506-001

**Authored:** 2026-05-08 ~09:55 IST · Session 7 (DMP Standing Rule)
**Branch:** `feat/pact-001-phase-1-wireup` HEAD `8043f6b3`
**Audit base:** `26677e42` (PrivilegedAction enum substrate + NF-bono-1 absorption — last green main-merge target)
**Scope:** PR-open readiness checklist for Wave 0 PACT-20260506-001 Phase 1 (CIRS lookup HTTP surface + POS .130 UI + idle-timeout middleware + ManagerPill UI)

**MMA pre-ship signal:** DIAGNOSE 5 models / VERIFY 3 adversarial models (mean 4.53, ≥4.0 PASS gate). Path A executed at `8043f6b3` (TS wire-format alignment to Rust canonical). 3 P1 findings (CSRF / phone-leak / rate-limit) deferred to security-debt-ledger rows 7-9 per Captain "Open-by-Default Flagged-to-Close" doctrine.

---

## §1 — Deploy actions checklist (per DMP rule)

### 1.1 Rust binary — racecontrol

**Action:** rebuild + redeploy `racecontrol.exe` on every target.

**Why:** new HTTP handler at `POST /api/v1/cirs/lookup`; new auth middleware extension (idle-timeout via `is_idle_expired`); new `AuthConfig.idle_timeout_secs` field; new `PrivilegedAction` enum at `crates/racecontrol/src/auth/privileged_actions.rs`.

**Files (Phase 1 wire-up scope, excluding F25 inherited via merge `984774a6`):**
- `crates/racecontrol/Cargo.toml` (added `v2-db = { path = "../v2-db" }` dep)
- `crates/racecontrol/src/api/cirs_lookup.rs` (new — 865 LOC)
- `crates/racecontrol/src/api/mod.rs` (added `pub mod cirs_lookup`)
- `crates/racecontrol/src/api/routes.rs` (registered `/api/v1/cirs/lookup` POST under staff-JWT-protected sub-router)
- `crates/racecontrol/src/auth/admin.rs` (PrivilegedAction enum exports)
- `crates/racecontrol/src/auth/middleware.rs` (`is_idle_expired` helper + idle check in `extract_staff_claims`)
- `crates/racecontrol/src/auth/middleware_tests.rs` (+6 tests)
- `crates/racecontrol/src/auth/privileged_actions.rs` (new)
- `crates/racecontrol/src/config/services.rs` (`AuthConfig.idle_timeout_secs: u64` with `default_idle_timeout()=1800`)
- `crates/racecontrol/src/state.rs` (added `pub v2db: v2_db::DbPool` field)
- `crates/racecontrol/src/main.rs` (v2-db pool open + migrate after `db::init_pool`)

**Test gates (from this session — to re-run on Linux at PR-open):**
- `cargo test -p racecontrol-crate --lib` → 1070 PASS / 0 FAIL / 2 ignored on James .27 debug
- `cargo test -p v2-db` → 30 PASS baseline preserved (v2-db crate untouched this PR)

### 1.2 Frontend rebuild — web-v2 (Next.js v2 host)

**Action:** rebuild `web-v2` Next.js bundle + redeploy on every target serving the v2 surface.

**Why:** new POS lookup page at `/v2/pos/lookup`; 5 new components + ManagerPill UI; new TS types files; Playwright E2E config + specs.

**Files:**
- `web-v2/src/app/pos/lookup/page.tsx` (new — 172 LOC + Path A adjusted handleWalkInSelect)
- `web-v2/src/app/pos/lookup/page.module.css` (new)
- `web-v2/src/components/v2/pos/PhoneLookupInput.tsx` + .module.css
- `web-v2/src/components/v2/pos/ProfilePreviewCard.tsx` + .module.css
- `web-v2/src/components/v2/pos/WalkInGuestDropdown.tsx` + .module.css
- `web-v2/src/components/v2/pos/NotFoundCTA.tsx` + .module.css
- `web-v2/src/components/v2/pos/LookupErrorBanner.tsx` + .module.css
- `web-v2/src/components/v2/auth/ManagerPill.tsx` + .module.css
- `web-v2/src/lib/types/cirs-lookup.ts` (Path A wire-format aligned)
- `web-v2/src/lib/types/privileged-action.ts` (TS mirror of Rust enum)
- `web-v2/playwright.config.ts` (new — Lane A scaffolding)
- `web-v2/tests/e2e/cirs-lookup.spec.ts` (5 specs)
- `web-v2/vitest.config.ts` + `web-v2/vitest.setup.ts`
- `web-v2/src/components/v2/auth/ManagerPill.test.tsx`
- `web-v2/src/lib/types/privileged-action.test.ts`
- `web-v2/.gitignore` (added Playwright artifacts)
- `web-v2/package.json` + `web-v2/package-lock.json` (devDependencies: @playwright/test 1.59.1, vitest 4.1.5, etc.)

**Test gates (from this session):**
- `tsc --noEmit` clean
- `npm test` (vitest) → 23/23 PASS in 1.75s
- `npx playwright test` → 5/5 PASS in 5.4s

**NOTE:** Production build path `npm run build` was NOT exercised this PR (dev mode only). Session 8 deploy verify must include `npm run build` + visual check from a non-James-machine browser per CLAUDE.md "Frontend: verify from the user's browser, not from the server."

### 1.3 Config change — `racecontrol.toml`

**Action:** add `idle_timeout_secs = 1800` to `[auth]` section on every target.

**Why:** new field in `AuthConfig`; default is 1800 (30-min sliding-window per Captain §S-82 Q3); explicit value preferred over relying on `default_idle_timeout()` for prod-clarity.

**Targets:**
- Server .23: `C:\RacingPoint\racecontrol.toml`
- Bono VPS: `/root/racecontrol/racecontrol.toml`

**Verification command:**
```
grep -A1 "^\[auth\]" racecontrol.toml | grep idle_timeout_secs
```
Expected: `idle_timeout_secs = 1800`

### 1.4 DB migration

**Action:** **NONE for this PR.** All schema (cirs_lookup_audit + staff_id FK + customers + wallets + sessions) already migrated via PACT-20260505-001 Phase 0 (`483562ac`) + PACT-20260503-018 staff_id FK (`3119da30`).

**Verify (read-only):** at deploy-time, run `sqlite3 v2.db ".schema cirs_lookup_audit"` and confirm staff_id FK + input_hash column present.

### 1.5 Infrastructure

**Action:** **NONE.** No new ports, services, or scheduled tasks. Existing racecontrol :8080 + web-v2 :3500 (or :3300 kiosk per existing wiring) reused.

### 1.6 Data files

**Action:** **NONE.**

### 1.7 .bat / startup script sync

**Action:** **NONE.** No changes to `start-racecontrol.bat` / `start-rcagent.bat` / `start-comms-link.bat`. Existing HKLM Run wiring covers the new binary.

### 1.8 Cloud parity (UNIVERSAL — NO EXCEPTIONS per CLAUDE.md DEPLOY PARITY rule)

**Action:** mirror every step on Bono VPS in the SAME deploy window.

**Sequence (post-PR-merge):**
1. `git push origin main` (after PR merge to main on bono-bot/racecontrol)
2. Bono relay `git_pull` for racecontrol repo: `curl -s -X POST http://localhost:8766/relay/exec/run -d '{"command":"git_pull"}'`
3. Bono relay rebuild racecontrol Rust binary (cargo build --release on Linux)
4. Bono relay rebuild web-v2 Next.js production bundle (`npm run build` on Linux)
5. PM2 restart `racecontrol` on Bono VPS
6. PM2 restart web-v2 host on Bono VPS (port 3500 if matching James .27)
7. Verify `curl https://srv1422716.hstgr.cloud:8080/api/v1/health` returns the new build_id
8. Verify `curl https://v2.racingpoint.cloud/v2/pos/lookup` HTTP 200 + Next.js _next/static/ asset 200

### 1.9 Per-target deploy targets enumeration (per CLAUDE.md H4)

| Target | What gets deployed | Verify command |
|---|---|---|
| **Server .23** racecontrol | new racecontrol.exe binary + racecontrol.toml idle_timeout_secs add | `curl http://192.168.31.23:8080/api/v1/health` → build_id matches `8043f6b3` (or PR-merge hash) |
| **POS .130** (Edge kiosk consuming web-v2) | Next.js v2 production bundle served from web-v2 host | open `/v2/pos/lookup` in Edge kiosk → screenshot of staff lookup flow per CLAUDE.md "Visual verification for display-affecting deploys" |
| **James .27** web-v2 dev host | rebuilt web-v2 production bundle on `npm run build` | `curl http://localhost:3500/v2/pos/lookup` → HTTP 200 + `_next/static/` 200 |
| **Bono VPS** racecontrol + web-v2 | mirrored binary + bundle per §1.8 | `curl https://srv1422716.hstgr.cloud:8080/api/v1/health` → matching build_id |
| **Cloud apps** (`v2.racingpoint.cloud` if pointing at Bono) | new web-v2 bundle | `curl https://v2.racingpoint.cloud/v2/pos/lookup` → HTTP 200 |
| **racecontrol-web kiosk + admin** | rebuild only if shared component dependency changed | `bash scripts/frontend-staleness-check.sh` clean |
| **Comms-link** | NO change this PR (no comms-link/ files in diff) | n/a |
| **Pods 1-8** | NO change this PR (no rc-agent / rc-sentry files in diff; Pod 1 still Captain-physical-blocked per §S-72) | n/a |

### 1.10 Real-wire integration verify (Session 8 mandatory — NOT done in this PR's mocked tests)

The Path A fix is verified at the JSON wire-format level via TS compiler + Rust serde tests, but the END-TO-END integration (web-v2 fetch → Next.js proxy → racecontrol /api/v1/cirs/lookup → v2-db → ProfilePreview render) has NOT been exercised this PR. Session 8 deploy-time must include:

1. From POS .130 Edge kiosk browser: type `9876543210` → click Lookup → confirm ProfilePreview renders OR NotFound CTA shows
2. From POS .130 browser: select Walk-In Guest 1 → confirm short-circuit Found preview with `discount_ineligible: true` (THIS is the path that would have 400'd pre-Path-A)
3. From racecontrol logs: confirm `cirs_lookup_audit` row written for each lookup
4. From cloud (Bono VPS) browser: same checks against `https://v2.racingpoint.cloud/v2/pos/lookup`

---

## §2 — Pre-PR-open gate (per CLAUDE.md Pre-Ship Gate)

| Gate | State | Note |
|---|---|---|
| Quality Gate (`bash test/run-all.sh` in comms-link) | **NOT YET RUN** this session | run pre-PR-merge per CLAUDE.md Ultimate Rule layer 1 |
| E2E live exec + chain + health round-trip | **NOT YET RUN** | run from a non-James machine pre-merge |
| Standing Rules check (auto-push, partner notified, watchdog running) | partial — bono notify queued at `.inbox-drafts/` (Axis-A blocked); auto-push GREEN | bono notify needs `BYPASS_AXIS_CLASSIFICATION=1` or responsive context |
| MMA Audit (Cross-System Bridge mandatory) | **DONE** — DIAGNOSE 5 models + VERIFY 3 adversarial; mean 4.53 ≥ 4.0 PASS | $0.09 of $5 budget spent |
| Subagent gates (gsd-ui-researcher / gsd-ui-auditor / gsd-integration-checker / gsd-nyquist-auditor) | **NOT YET RUN** | gsd-ui-auditor is mandatory per CLAUDE.md "No frontend phase ships without UI-SPEC.md AND UI-REVIEW.md" — open question whether Wave 0 PR-open inherits Layer 1 design handoff `b5774aac` UI-SPEC or needs fresh review |
| Visual verification (POS .130 browser) | **NOT YET RUN** | Session 8 deploy-time gate |
| **web-v2 production build (`npm run build`)** | **PASS** — closes §1.2 NOTE | James .27 Node v22.22.0 Next 16.1.6 Turbopack — Compiled successfully in 3.5s; static pages 4/4 in 393.4ms; routes: `/`, `/_not-found`, `ƒ /api/v1/health`, `/pos/lookup`. Captured 2026-05-08 ~13:00 IST post-PR-open |
| **PR #64 CI rollup** (`gh pr checks 64` post-CI-completion) | **5/5 PASS · mergeStateStatus CLEAN** | API Contract Tests 15s · Comms-Link Quality Gate 9s · Security Scan 6s · Rust Tests 24m59s · build 41m3s. Captured 2026-05-08 ~13:10 IST. Re-runs on every push to branch |
| Captain per-PR auth (PROMOTED-N=1) | **PR #64 OPEN-DRAFT, CI CLEAN, awaiting Captain promote-from-draft + merge** | https://github.com/bono-bot/racecontrol/pull/64 — gates merge (NOT PR-open; PR-open authorized by Captain "Proceed autonomously" 2026-05-08 ~10:52 IST + ~13:10 IST §S-N) |

---

## §3 — Captain decisions still queued at PR-open time

| # | Decision | Source |
|---|---|---|
| **K1** | Per-PR MERGE auth — **DISPOSED** Captain explicit "merge PR #64 as fixed-window" 2026-05-08 ~13:50 IST. PR #64 MERGED `991b5411` 2026-05-08 13:54 IST (squash on main; branch deleted) | this manifest §2 + GitHub PR #64 |
| **K2** | Cross-layer naming-drift — **CLOSED** via `ba17088f` (TS `ProfilePreview`/`ProfileSummary` fields aligned to Rust canonical: `name`, `primary_phone`, `last_visit_ts`, `discount_ineligible` mirrored) | flagged Session 6d Lane A; resolved Session 7 fix-inline |
| **K3** | RsX refund-threshold for `ApproveRefundOverThreshold` enum entry (Wave 1 scope) | PACT-20260506-001 §AMEND-1.I |
| **K4** | PIN-LOCKOUT sub-questions 1.e-h (cadence/delivery/reset/fallback) (Wave 1 scope) | §S-82 Q1 |
| **K5** | Idle-timeout fixed-window-from-iat vs §S-82 Q3 sliding-window — **DISPOSED** Captain Path A 2026-05-08 ~13:50 IST verbatim "merge PR #64 as fixed-window; file V2.1 sliding-window PACT post-launch". V2.0 ships as-shipped; V2.1 sliding-window PACT pin planted at memory `project_v2_1_sliding_window_idle_timeout_pact_pin.md` | discovered during MMA prompt authoring; surfaced via INBOX `f7d00b04` 2026-05-08 13:44 IST |

---

## §4 — Stale-at conditions

This MANIFEST durable until any of:
- (a) PR-open + post-merge SUMMARY.md replaces it
- (b) Captain disposes a Wave 0 scope change that materially adds/removes deploy targets
- (c) Verify-by 2026-05-19 — if Session 8 not deploy-complete by then, escalate

---

## §5 — Audit script known issue (orthogonal)

`scripts/deploy/deploy-audit.sh` exited early after the header banner on this run (likely `set -euo pipefail` + a grep return-1 on no-match). Surfaced for orthogonal fix; this manifest authored manually from `git diff --name-only 26677e42..8043f6b3` output (64 files). Bug NOT in PACT-20260506-001 scope.

---

— james / 2026-05-08 ~09:55 IST · Phase 1 wire-up DEPLOY MANIFEST · branch `feat/pact-001-phase-1-wireup` HEAD `8043f6b3` · MMA mean 4.53 / 3 P1 findings deferred to security-debt-ledger rows 7-9 · per-PR Captain auth gate STANDS for PR-open · 5 Captain-reserve K-decisions queued

---

## §6 — Post-authoring update (2026-05-08 ~13:15 IST)

Manifest authored at `3ea5dc89` (HEAD `8043f6b3`). Following commits + events have landed since:

| When | Event | Reference |
|---|---|---|
| 2026-05-08 ~10:30 IST | `ba17088f` — K2 close (TS `ProfilePreview`/`ProfileSummary` aligned to Rust canonical) | branch commit |
| 2026-05-08 ~10:50 IST | `41ca8a29` — UI-REVIEW FLAG-2 close (Montserrat brand font via `next/font/google`) | branch commit |
| 2026-05-08 ~10:55 IST | **PR #64 opened DRAFT** against `main`; 55 files / +7176 / −144 | https://github.com/bono-bot/racecontrol/pull/64 |
| 2026-05-08 ~13:00 IST | `npm run build` (web-v2 production) **PASS** on James .27 | manifest §2 row added |
| 2026-05-08 ~13:10 IST | PR #64 CI **5/5 PASS · mergeStateStatus CLEAN** | manifest §2 row added |
| 2026-05-08 ~13:15 IST | This manifest refresh commit (§2 + §3 + §6) | docs-only; CI re-runs on push |

**Forward state:**
- PR #64 fully ready for Captain merge-auth click (K1) — awaiting promote-from-draft + merge
- K5 (sliding vs fixed-window idle-timeout) is the remaining substantive decision; Captain ratify-or-fix needed before merge
- Quality Gate / E2E live / visual-verification gates still apply at Session 8 deploy-time (post-merge)

---

## §7 — Merge + K5 disposition addendum (2026-05-08 ~13:55 IST)

| When | Event | Reference |
|---|---|---|
| 2026-05-08 ~13:44 IST | K5 surfaced to Captain via INBOX | comms-link `f7d00b04` |
| 2026-05-08 ~13:50 IST | Captain Path A disposition: "merge PR #64 as fixed-window; file V2.1 sliding-window PACT post-launch" | chat session verbatim |
| 2026-05-08 ~13:54 IST | PR #64 promoted from draft → ready + squash-merged to main | merge commit `991b5411` |
| 2026-05-08 ~13:55 IST | V2.1 sliding-window PACT pin planted | memory file `project_v2_1_sliding_window_idle_timeout_pact_pin.md` |
| 2026-05-08 ~13:55 IST | `middleware.rs:103-110` inline comment updated to cite V2.1 PACT pin (post-merge docs commit) | this commit |

**Wave 0 status:** SHIPPED to main. Session 8 deploy-time gates (Quality Gate / E2E live / visual verification on POS .130 / Bono VPS parity per §1.8) remain — those are deploy disposition, separate from PR-merge.

**V2.1 trigger conditions for sliding-window PACT FILE:** V2.0 launch readiness Wave 6 / staff burn-in data >10% mid-active force-re-PIN / Captain explicit request / 2026-06-30 calendar reminder — whichever first.

**Manifest now stale-at:** §4 condition (a) PR-open + post-merge SUMMARY.md replaces it — SUMMARY.md not yet authored; this manifest stays durable until SUMMARY.md or until Session 8 deploy-time replaces it.
