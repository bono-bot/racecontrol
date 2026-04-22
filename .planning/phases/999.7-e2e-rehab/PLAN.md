# Phase 999.7 — Kiosk E2E Rehabilitation

**Created:** 2026-04-22
**Status:** IN PROGRESS
**Blocks:** PR #14 (`fix/kiosk-audit-batch-20260422`) merge; server + kiosk deploy

## Why

Kiosk E2E (`.github/workflows/e2e-tests.yml`) has **0/20 green runs** on recent
history (main 2026-04-18 onwards). PR #14 surfaced the decay when its build-env
fix unblocked the previously-blocked build step. Failing tests cluster into two
patterns:

1. **No-backend failures (~7 tests across 2 files):** `game-launch.spec.ts` and
   `wizard.spec.ts` hit `/kiosk/staff` with no request mocks, expect the real
   racecontrol backend to answer `/api/v1/staff/validate-pin` + `/api/v1/pods`
   etc. CI doesn't start racecontrol. Tests time out 30s on wizard visibility,
   `afterEach` throws on the resulting JS errors, test.skip() is overridden.

2. **Stale visual baselines (4 tests):** `visual.spec.ts` compares pixel-perfect
   PNG snapshots captured against an older kiosk build. Recent kiosk commits
   (customer landing redesign, game logo SVG migration, HUD changes) invalidate
   the baselines.

3. **One genuine assertion failure:** `setup-wizard-inventory.spec.ts:462 C:
   inventory unreachable` — already has mocks (tests A+B pass with identical
   setup); failure is in the banner + aria-describedby assertions. Already
   `test.skip`'d in commit `35dc9137` pending local Playwright repro.

## Scope

In scope:
- Shared mocks helper (`tests/e2e/playwright/kiosk/helpers/mocks.ts`) with
  login + pods + experiences + inventory fixtures.
- Refactor `game-launch.spec.ts` to use shared mocks.
- Refactor `wizard.spec.ts` to use shared mocks.
- Harden `afterEach` to tolerate expected network errors or to suppress JS
  errors on tests that called `test.skip()`.
- Refresh 4 visual baselines via `npx playwright test --update-snapshots`.
- Diagnose + re-enable `setup-wizard-inventory.spec.ts:462 C`.

Out of scope:
- Rewriting tests against a live backend (separate project/workflow, future).
- `staff-launch.spec.ts` remains venue-only (already skipped in `35dc9137`).
- Any Rust-side change.

## Success Criteria

- Kiosk E2E Tests completes green on CI for HEAD of
  `fix/kiosk-audit-batch-20260422` branch.
- PR #14 mergeStateStatus = CLEAN.
- No new `test.skip` added beyond the already-documented ones.

## Execution Order

| Step | Action | Deliverable | Status |
|------|--------|-------------|--------|
| 1 | Extract `loginAndOpenWizard` + mock fixtures into shared helper | `kiosk/helpers/mocks.ts` | pending |
| 2 | Refactor `setup-wizard-inventory.spec.ts` to import from shared helper (dogfood) | updated spec, still green | pending |
| 3 | Refactor `game-launch.spec.ts` to use shared mocks | updated spec, 4 tests green | pending |
| 4 | Refactor `wizard.spec.ts` to use shared mocks | updated spec, ~3 tests green | pending |
| 5 | Harden `afterEach` JS-error filter (skipped → no throw) | 1 helper change | pending |
| 6 | Run `npx playwright test --update-snapshots` for visual baselines | 4 updated PNGs | pending |
| 7 | Local run of full kiosk project — confirm all green | terminal output | pending |
| 8 | Push + verify CI green | PR #14 CLEAN | pending |
| 9 | Diagnose + un-skip setup-wizard-inventory test C | 1 more green test | deferred-if-hard |

## Plan Iteration Notes

- Each step is an incremental commit to `fix/kiosk-audit-batch-20260422`.
- If any step reveals deeper decay, it becomes its own sub-step — don't
  force green by widening skips.
- Visual baseline refresh (Step 6) must happen on Linux (baselines are
  OS-specific by font rendering) — run via CI workflow_dispatch or inside
  Docker if local Windows differs.

## Risk Register

- R1: Linux font rendering differs from Windows → local `--update-snapshots`
  produces baselines CI still rejects. Mitigation: regenerate via a CI
  workflow_dispatch run with `--update-snapshots` and commit the artifacts.
- R2: Mocks don't cover every fetch the kiosk makes (e.g. WS, leaderboard,
  health) → tests still fail on unmocked paths. Mitigation: start from the
  failure log per spec and expand mocks iteratively.
- R3: Unblocking E2E surfaces yet more failing tests hidden behind the
  current 11. Mitigation: re-triage after step 8.

## Out-of-Scope Deferrals (track separately)

- `setup-wizard-inventory.spec.ts:462 C` — deferred to Step 9 or follow-up.
- `staff-launch.spec.ts` venue-only smoke — move to a `@venue` project in a
  future phase.

## Commits So Far (PR #14)

- `15ebe26c` — kiosk unit test cascade + CRLF SHA guard
- `49897b7d` — build env validator scoping
- `35dc9137` — workflow timeout 15→45min, venue-only skip, wizard-C skip

Rehab work on top of these, targeting PR #14 green and subsequent deploy.
