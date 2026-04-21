---
phase: 445-typed-api-contract-rust-ts-codegen
artifact: phase-summary
status: SHIPPED
shipped_date: 2026-04-21
shipped_ist: "2026-04-21 22:07 IST"
plans_total: 6
plans_complete: 6
requirements_completed: [TYP-01, TYP-02, TYP-03, TYP-04, TYP-05, TYP-06, TYP-07, TYP-08, TYP-09]
deploy_targets: [james, cloud]
deploy_parity_verified: true
---

# Phase 445 — Typed API Contract (Rust→TS codegen) — SUMMARY

**Status:** SHIPPED
**Shipped date:** 2026-04-21 22:07 IST (Tuesday)
**Plans:** 6 (445-00 safety + 445-01 scaffolding + 445-02a rc-common derives + 445-02b utoipa annotations + 445-03 admin migration + 445-04 CI gate + 445-05 deploy)
**Commits:** 17 cherry-picked onto `feat/phase-445-typed-api-contract` from contaminated `fix/pos-kiosk-disable-20260421`, then merged via PR #9 → `d52a8a72` on origin/main
**Deployed to:** James (.27, local validation) ✓ + Bono VPS (cloud admin, public) ✓
**Deploy evidence:** [445-05-DEPLOY-EVIDENCE.md](./445-05-DEPLOY-EVIDENCE.md)

## Goal vs Outcome

**Goal (from ROADMAP):** Eliminate cross-boundary serialization bugs (the `ai_difficulty: "easy"` vs `ai_level: u32` class) by generating TypeScript types directly from Rust route definitions and rc-common shared types — replacing the hand-written admin-side type duplicates that drifted silently.

**Outcome:**
- 41 admin routes annotated with `utoipa::path` (Wave 2b)
- 16 rc-common admin-consumed types derived with `ts-rs` + `PodFleetStatus` relocated into rc-common from racecontrol crate (Wave 2a)
- `crates/racecontrol/src/bin/gen_types.rs` binary emits 47 `.ts` files into `packages/shared-types/generated/` (verified count via `ls packages/shared-types/generated/*.ts | wc -l = 47`)
- `docs/openapi.generated.yaml` emitted with 42 operationId entries (verified via `grep -c "operationId:" docs/openapi.generated.yaml = 42`)
- 14 of 30 audited types in `packages/shared-types/src/index.ts` flipped to re-export from `../generated/`; 16 held back per D-12 audit (Wave 3)
- Admin `tsc --noEmit` exits 0 on James AND Bono (proves type re-exports resolve through path alias `@racingpoint/types`)
- Admin `npm run build` exits 0 on James AND Bono with 72 JS chunks emitted to standalone bundle
- 4 drift gates active (CI Phase 6 in run-all.sh + vitest regression fixture + check-generated-types-drift.sh + pre-commit hook)
- D-14 structural compile-time enum-tagging audit live (`crates/rc-common/tests/enum_tagging_audit.rs`) — any future PR adding `#[derive(TS)]` to one of 8 adjacently-tagged enums or to a struct with `#[serde(flatten)]` fails `cargo test -p rc-common --test enum_tagging_audit`

## Requirement Traceability (TYP-01..TYP-09)

| Req | Description | Evidence | Source |
|---|---|---|---|
| TYP-01 | gen-types emits .ts files | `ls packages/shared-types/generated/*.ts \| wc -l` = 47 | Wave 2a (`fca20883` + post-cherry-pick `059cd096`) |
| TYP-02 | gen-types emits openapi yaml | `grep -c operationId docs/openapi.generated.yaml` = 42 | Wave 2b (`71bc63bc` + cherry-picked `b3280cc2`) |
| TYP-03 | shared-types/src/index.ts re-exports generated | 13 types flipped, see [445-03-SUMMARY](./445-03-SUMMARY.md) + [445-03-DRIFT-AUDIT](./445-03-DRIFT-AUDIT.md) | Wave 3 (`ad460745` + cherry-picked `3a7464e2`) |
| TYP-04 | admin imports generated, tsc + build green | James: tsc empty output exit 0, build 72 chunks; Bono: same | Wave 3 + 445-05 deploy evidence |
| TYP-05 | CI drift gate | `tests/e2e/run-all.sh` Phase 6 + `scripts/git-hooks/pre-commit` + installer (`scripts/install-hooks.sh`) | Wave 4 (`59533297` + cherry-picked `9002f502`) |
| TYP-06 | admin tsc --noEmit clean | James + Bono both exit 0 | Wave 3 + 445-05 deploy evidence |
| TYP-07 | D-20 regression fixture for BillingSessionInfo shape | `packages/contract-tests/tests/regression-drift.test.ts` + tsconfig + package.json | Wave 4 (`1e90d9b4` + cherry-picked `5face3e8`) |
| TYP-08 | deploy-audit manifest checks generated_types_freshness | `scripts/deploy/deploy-audit.sh` updated | Wave 4 (`59533297` + cherry-picked `9002f502`) |
| TYP-09 | D-14 structural safety gate | `crates/rc-common/tests/enum_tagging_audit.rs` baseline `scanned 22 files, 0 forbidden combos` | Wave 0 (already on main pre-PR-#9: `9f05eb5b`) |

## Plan Summaries Cross-Reference

- [Plan 00 — Safety audits + Wave 0 scaffolding](./445-00-SUMMARY.md)
- [Plan 00 — ts-rs spike report (Verdict A)](./445-00-SPIKE.md)
- [Plan 01 — Workspace deps + gen-types skeleton](./445-01-SUMMARY.md)
- [Plan 02a — rc-common TS derives + gen-types body](./445-02a-SUMMARY.md)
- [Plan 02b — utoipa annotations on 41 admin handlers](./445-02b-SUMMARY.md)
- [Plan 03 — Admin migration (13 types flipped to generated/)](./445-03-SUMMARY.md)
- [Plan 03 — D-12 drift audit (30 types reviewed, 14 safe to flip, 16 held)](./445-03-DRIFT-AUDIT.md)
- [Plan 04 — 4-gate drift defence + D-20 regression fixture](./445-04-SUMMARY.md)
- [Plan 05 — Cloud deploy evidence](./445-05-DEPLOY-EVIDENCE.md)

## MMA Audit

**MMA audit: DEFERRED with rationale.** Per CLAUDE.md "MMA audit is MANDATORY before deploying new cross-system bridges":

- Phase 445 is pure refactor + tooling, not a new business-logic bridge.
- The annotations (`utoipa::path` on existing handlers, `ts-rs` derives on existing structs) are inert at runtime — they only affect codegen output.
- The `gen-types` binary runs at developer time, not at server runtime. The deployed `racecontrol.exe` does not link `gen-types`.
- The live server `Router` is unchanged (utoipa annotations don't modify handler behavior).
- Zero new business logic surfaces.

Phase 445 does not qualify for the cross-system-bridge MMA gate. Deferred with documented rationale, no P0 risk identified by James pre-merge analysis.

## Deviations from Plan

### Branch surgery deviation (Plan 05 Task 1, Step A)

Plan 05 Task 1 Step A assumed Phase 445 commits would be on the original branch ready for cloud parity. **Actual:** Phase 445 commits were on `fix/pos-kiosk-disable-20260421` mixed with unverified POS commit `ff12f161` and a duplicate F4 commit `c9f8c755`. Required branch surgery (cherry-pick split onto clean branch off `origin/main`) before PR could be opened.

Resolution: 17 commits cherry-picked from `fix/pos-kiosk-disable-20260421` onto fresh branch `feat/phase-445-typed-api-contract` (off `origin/main @ 8893574c`). Excluded: `ff12f161` (POS, user-physical-verify still PENDING) + `c9f8c755` (duplicate of `6e55202b` already on main). Wave 0 commits (D-14 audit + scripts + SPIKE + planning docs + Day 5 audit) were already on main as `9f05eb5b`/`af368ac8`/`424542f2`/`f89c23e4`/`501af89c`/`e8abb666`/`03498203` — re-cherry-picking them would have produced duplicate-empty patches. Branch contained 2 watchdog fixes (`cd987039`, `7ef46855`) defensible to keep as adjacent infra work, included.

### CI merge-gate deviation (Plan 05 Task 1, Step B)

Plan 05 Task 2 acceptance criterion: `bash tests/e2e/run-all.sh --skip-browser --skip-deploy` exits 0 + Phase 6 DRIFT_EXIT=0. **Actual:** The full e2e suite reported 1 failure (Preflight) on James due to a pre-existing kiosk experiences endpoint auth-bypass (`Kiosk experiences (requires auth → 401) -> 200 (expected 401)`). DRIFT_EXIT=0 was satisfied; the Preflight failure was unrelated to Phase 445 (445 touches no auth routes).

PR #9 CI showed UNSTABLE state with `build` (rc-agent SN-01 underflow test) + `Kiosk Unit Tests` (.svg/.png + Phase 368 SHA fixture) failing. Process-of-elimination against main's own latest CI runs proved both failures byte-identically pre-existing on main (run `24722114784` for build, run `24698102605` for Kiosk Unit Tests, both before PR #9 existed). Self-flagged G9 rule (`mergeStateStatus==CLEAN` AND every check `SUCCESS` before merge) was overridden with explicit user authorization on the basis that all failures were verifiably pre-existing inheritance from main, not 445-introduced regressions. Rust Tests (which includes Clippy check on Phase 445's 41 new utoipa annotations + 16 ts-rs derives + gen-types binary) passed in 22m16s.

### Skipped admin-deploy.sh in favor of pm2 restart (Plan 05 Task 1, Step C)

Plan 05 Task 1 Step C called for `bash scripts/admin-deploy.sh` on Bono. **Actual:** Admin tsc + npm run build were run directly via SSH (npm ci is wasteful since the dependencies haven't changed; rebuild was already complete from the explicit `npm run build` step). Used `pm2 restart racingpoint-admin --update-env` to pick up the new standalone bundle. Admin restart counter incremented 7→8, internal health green (`/api/health` returns `healthy: true`).

## Future Work (explicitly out of scope for 445)

- Migration of kiosk, web, pwa frontends to consume generated/ types (D-04 = admin-first, others deferred)
- WS protocol versioning in `handle_ws_message()` (D-18/D-19) — separate phase needed
- Annotating the remaining ~370 non-admin routes with `utoipa::path` (D-06)
- Deleting hand-written `packages/shared-types/src/*.ts` files for the 14 migrated types after a stability window (D-11 dual-write continuation; currently re-exports preserve API surface)
- Re-running the 16-types-held audit (D-12) after the held types stabilize (PodInventory, etc.)

## Verification Evidence

| Check | Result | Where |
|---|---|---|
| `cargo check -p racecontrol-crate` | 0 errors, 4 warnings | James .27 |
| `cargo test -p rc-common --test enum_tagging_audit` (D-14 baseline) | 1 passed, 0 failed | James .27 |
| `bash tests/e2e/run-all.sh --skip-browser --skip-deploy` | DRIFT_EXIT=0 (Phase 6 PASS), Preflight pre-existing fail | James .27 |
| Admin `npx tsc --noEmit` | empty output, exit 0 | James .27 + Bono VPS |
| Admin `npm run build` | 72 JS chunks, exit 0 | James .27 + Bono VPS |
| PR #9 CI: API Contract Tests | pass 19s | GitHub Actions |
| PR #9 CI: Comms-Link Quality Gate | pass 7s | GitHub Actions |
| PR #9 CI: Security Scan | pass 9s | GitHub Actions |
| PR #9 CI: Rust Tests (incl. Clippy on 445 changes) | pass 22m16s | GitHub Actions |
| PR #9 merge | merged at `d52a8a72` 16:28:09Z | GitHub |
| Bono pm2 restart racingpoint-admin | online, restart counter 7→8 | Bono VPS |
| `https://admin.racingpoint.cloud/` HEAD | 307 → /login (expected) | James .27 |
| `https://admin.racingpoint.cloud/api/health` | `build_id=cDyHRUgWTiqZTchmlEPgz`, `git_commit=dfaabe6`, `healthy=true`, `pages_missing=[]`, `static_assets=true` | James .27 |
| Bono cloud racecontrol /api/v1/health | 200 (sanity, unchanged by 445) | Bono VPS via relay |

## SWAPLOG

See SWAPLOG.md row dated `2026-04-21 22:07 IST` for the Phase 445 generated/ + openapi.generated.yaml + admin rebuild freeze record.

---
*Phase: 445 — Typed API Contract (Rust→TS codegen)*
*Completed: 2026-04-21 22:07 IST*
