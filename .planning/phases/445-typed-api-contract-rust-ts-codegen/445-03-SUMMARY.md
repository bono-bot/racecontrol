---
phase: 445-typed-api-contract-rust-ts-codegen
plan: 03
subsystem: admin-type-migration
tags: [shared-types, ts-re-export, dual-write, admin-migration, drift-audit, D-10, D-12, D-16]

# Dependency graph
requires:
  - 445-02a (46 generated .ts files under packages/shared-types/generated/ + index.ts barrel)
  - 445-02b (docs/openapi.generated.yaml — not directly consumed by this plan but part of Wave 2 deliverables)
provides:
  - packages/shared-types/src/index.ts flipped for 13 migrated types (dual-write live)
  - 445-03-DRIFT-AUDIT.md — per-type parity verdicts (30 headings, 348 lines)
  - Admin `@racingpoint/types` now sources pod/game enums, billing status, pricing tier, and inventory types from Rust-derived generated/ (byte-identical types across server + admin)
  - TYP-06 shape-check gate passed end-to-end (admin tsc --noEmit exit 0 + npm run build exit 0)
affects: [445-04 (CI drift gate + regression fixture), 445-05 (cloud parity)]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Dual-write (D-10/D-11): index.ts re-exports migrated types from ../generated/; hand-written files coexist for un-migrated and D-19 permanent types."
    - "Additive migration for name-colliding types: PodInfo exported alongside Pod, BillingSessionInfo exported alongside BillingSession — admin opts-in type-by-type in a follow-on phase."
    - "Bigint-drift gating: types with u64 fields where admin does arithmetic (PodFleetStatus.uptime_secs, Driver.total_laps) stay hand-written until admin handles bigint."

key-files:
  created:
    - .planning/phases/445-typed-api-contract-rust-ts-codegen/445-03-DRIFT-AUDIT.md
    - .planning/phases/445-typed-api-contract-rust-ts-codegen/445-03-SUMMARY.md
  modified:
    - packages/shared-types/src/index.ts (hand-written barrel -> dual-write barrel; 13 types now source from ../generated/)

key-decisions:
  - "D-03-01 HELD PodFleetStatus despite generated being a strict superset, because ts-rs emits u64 as bigint. Admin fleet/page.tsx:92 does Math.floor(uptime_secs / 86400) which would tsc-fail with bigint. Migration deferred to a follow-on phase that handles bigint arithmetic."
  - "D-03-02 HELD Driver despite near-parity, because (a) hand-written has admin-only computed field has_used_trial the Rust struct lacks; (b) total_laps / total_time_ms are u64 → bigint drift; (c) email/phone/steam_guid/iracing_id are T? optional vs generated required T | null. Migration needs Rust-side has_used_trial + bigint admin handling."
  - "D-03-03 ADDITIVE migration for PodInfo (was `Pod`) + BillingSessionInfo (was `BillingSession`). Task 2 exports the generated NAMES alongside the hand-written names rather than renaming admin code. Zero admin churn this plan; follow-on phases can opt-in."
  - "D-03-04 PricingTier FLIPPED safely. Admin pricing/tiers/page.tsx defines its OWN local `interface PricingTier` with a `sort_order` field — does NOT import PricingTier from @racingpoint/types. So the shared PricingTier's lack of sort_order has zero impact on admin."
  - "D-03-05 GameState FLIPPED with ACCEPTED drift. Generated is superset (+ in_lobby for MP). Admin switch-statements would get non-exhaustive warnings on in_lobby, but admin tsc --noEmit passed — no blocking errors."
  - "D-03-06 Script caveat documented in DRIFT-AUDIT.md. scripts/audit-handwritten-vs-generated.sh has a grep bug: for types imported by billing.ts (e.g. DrivingState), the grep matches billing.ts's field list and produces spurious DRIFT. Each pair was re-verified manually."

patterns-established:
  - "D-12 drift audit is MANDATORY before re-export flip. Task 2 acceptance gate uses `grep -Eq '## Aggregate Verdicts' DRIFT-AUDIT.md` as a hard pre-condition. Prevents silent flips without analysis."
  - "WS message types (D-19) and composite/handler-response types stay hand-written permanently. src/ws-messages.ts + src/reservation.ts + src/metrics.ts + src/config.ts re-exports preserved."
  - "Admin code path fully unchanged this plan — only the type source swaps. Zero admin refactor, zero admin file edits."

requirements-completed: [TYP-03, TYP-04, TYP-06]

# Metrics
duration: ~22min
completed: 2026-04-21
---

# Phase 445 Plan 03: Wave 3 — Admin Type Migration (src/index.ts Flip) Summary

**Flipped `packages/shared-types/src/index.ts` re-exports for 13 migrated types to source from `../generated/` (auto-generated from Rust via ts-rs). Dual-write (D-10/D-11) live: hand-written types coexist for un-migrated + D-19 permanent types. Admin `@racingpoint/types` now consumes Rust-derived types byte-identically for pod/game enums, billing status, pricing tier, pod info, billing session info, playable signal, and 4 inventory types. Admin `tsc --noEmit` and `npm run build` both exit 0 with zero admin code changes.**

## Performance

- **Duration:** ~22 min
- **Started:** 2026-04-21T01:49Z (07:19 IST)
- **Completed:** 2026-04-21T02:11Z (07:41 IST)
- **Tasks:** 2 (Task 1 drift audit, Task 2 index.ts flip)
- **Files created:** 2 (DRIFT-AUDIT.md + SUMMARY.md)
- **Files modified:** 1 (packages/shared-types/src/index.ts: +60 / -4 lines)

## Accomplishments

- **D-12 drift audit completed** for all types with a generated equivalent OR a hand-written equivalent in the migration scope — 30 type sections in 445-03-DRIFT-AUDIT.md (348 lines). Verdicts: 14 flip-safe (9 parity OK + 5 accepted drift), 16 held hand-written. Pitfall 2 (`T | null` vs `T | undefined`) and bigint drift (`u64` → `bigint`) documented as cross-cutting observations.
- **index.ts re-export flip** executed for 13 migrated types: PodStatus, SimType, DrivingState, GameState, BillingSessionStatus, PricingTier, PodInfo (additive), BillingSessionInfo (additive), PlayableSignal, PodInventory, GameInventory, ContentDirsResponse, GameDirs. 6 occurrences of `from '../generated/index'` in the new barrel.
- **Un-migrated + D-19 types retained:** Pod, GameCatalogEntry, BillingSession, Driver, PodFleetStatus, FleetHealthResponse, ConfigMismatchDetected, FeatureFlag, ConfigPush, ConfigAuditEntry, RedeemPinResponse, RedeemPinStatus, 10 WS message types, and 5 metrics types — all continue to re-export from `./` (src/*.ts).
- **Admin tsc green (D-16 gate):** `cd C:/Users/bono/racingpoint/racingpoint-admin && npx tsc --noEmit` exits 0. No type-shape regressions introduced by the migration.
- **Admin build green (TYP-04):** `npm run build` exits 0; postbuild standalone copy successful; `postbuild: wrote git-commit.txt (dfaabe6)`; `verified 72 JS chunks in standalone`.
- **Drift gate green:** `bash scripts/check-generated-types-drift.sh` exits 0 with `OK: no drift in packages/shared-types/generated/`. `cargo run --release --bin gen-types --features gen-types` re-emitted identical output during the check (no delta).
- **Dual-write preserved (D-11):** All hand-written `.ts` files in `packages/shared-types/src/` still exist — billing.ts, pod.ts, fleet.ts, driver.ts, config.ts, metrics.ts, reservation.ts, ws-messages.ts.

## Exact migrated type list (final flip, per drift audit)

From `../generated/index`:

| Type | Prior source | Verdict |
|------|--------------|---------|
| PodStatus | src/pod.ts | PARITY OK |
| SimType | src/pod.ts | PARITY OK |
| DrivingState | src/pod.ts | PARITY OK |
| GameState | src/pod.ts | ACCEPTED (generated adds `in_lobby`) |
| BillingSessionStatus | src/billing.ts | PARITY OK (11 variants byte-identical) |
| PricingTier | src/billing.ts | PARITY OK (admin uses local interface) |
| PodInfo | NEW (was `Pod`) | ACCEPTED (additive — Pod retained) |
| BillingSessionInfo | NEW (was `BillingSession`) | ACCEPTED (additive — BillingSession retained) |
| PlayableSignal | NEW | PARITY OK (new generated) |
| PodInventory | NEW | PARITY OK (new generated) |
| GameInventory | NEW | PARITY OK (new generated) |
| ContentDirsResponse | NEW | PARITY OK (new generated) |
| GameDirs | NEW | PARITY OK (new generated) |

## Types held hand-written (rationale)

| Type | Reason | Held until |
|------|--------|------------|
| BillingSession | Name collision with BillingSessionInfo + bigint on cost_paise/rate_per_min_paise | Admin refactor to BillingSessionInfo + bigint handling |
| Pod | Name collision with PodInfo (admin canonical is `Pod`) | Admin renames to PodInfo |
| GameCatalogEntry | No Rust struct (kiosk handler builds it) | Rust refactor to struct |
| Driver | Admin-only `has_used_trial` + bigint on total_laps/total_time_ms | Rust adds has_used_trial + admin handles bigint |
| PodFleetStatus | Admin fleet/page.tsx does `Math.floor(uptime_secs / 86400)` — bigint breaks it | Admin handles bigint arithmetic |
| FleetHealthResponse | No Rust struct (serde_json::json! inline) | Rust refactor to struct |
| ConfigMismatchDetected | WS message type | D-19 permanent |
| FeatureFlag, ConfigPush, ConfigAuditEntry | Admin-only DB shapes, no Rust struct | Indefinite |
| RedeemPinResponse, RedeemPinStatus | Admin/kiosk-only handler response composite | Indefinite |
| FailureMode, LaunchStatsResponse, BillingAccuracyResponse, AlternativeCombo, LaunchMatrixRow | Admin-only metrics types, no Rust struct | Indefinite |
| 10 WS message types (FlagSyncPayload, WsConfigPushPayload, OtaDownloadPayload, KillSwitchPayload, ConfigAckPayload, OtaAckPayload, FlagCacheSyncPayload, LaunchDiagnostics, BillingTick, GameStateChanged) | D-19 permanent (handle_ws_message mega-hub) | Never |

## Verification outputs

### `cd packages/shared-types && npx tsc --noEmit`

```
(no output)
EXIT=0
```

### `cd ../racingpoint-admin && npx tsc --noEmit` (D-16 gate)

```
(no output)
EXIT=0
```

### `cd ../racingpoint-admin && npm run build` (TYP-04)

```
... 72 routes compiled ...
postbuild: copied .next/static -> standalone
postbuild: copied public/ -> standalone
postbuild: wrote git-commit.txt (dfaabe6)
postbuild: verified 72 JS chunks in standalone
EXIT=0
```

### `bash scripts/check-generated-types-drift.sh`

```
... cargo warnings (pre-existing, unrelated) ...
gen-types: starting (Phase 445 Wave 2a)
gen-types: wrote docs/openapi.generated.yaml (20245 bytes)
gen-types: wrote packages/shared-types/generated\index.ts (2602 bytes)
gen-types: done
OK: no drift in packages/shared-types/generated/ or docs/openapi.generated.yaml
EXIT=0
```

## Surprises from the drift audit

- **Admin defines its OWN local `interface PricingTier`** in `pricing/tiers/page.tsx` (line 8) that includes `sort_order`. It does NOT import PricingTier from @racingpoint/types. So the generated PricingTier (which lacks sort_order) has zero effect on admin — safe flip. Expected this to be a HOLD; confirmed SAFE after grep.
- **Admin `fleet/page.tsx` imports `PodFleetStatus` from `@/lib/api/fleet`**, which re-exports it from `@racingpoint/types`. This means my flip DOES reach the fleet page. The bigint arithmetic on line 92 (`Math.floor(secs / 86400)` with `secs: number | null | undefined`) would fail if `uptime_secs` became `bigint`. Flipping PodFleetStatus would have broken admin tsc — correctly held.
- **The audit script's `grep -l -E "\\b$TYPE_NAME\\b" src/*.ts | head -1` is structurally broken** for types imported (not exported) by other src files. It matches billing.ts (which imports DrivingState) instead of pod.ts (which exports it), then diffs BillingSession's field list vs DrivingState's empty enum body and prints spurious DRIFT. Task 1 manually overrode the script's verdict for 5+ types. Plan 04 should fix the script to resolve type source via `export type ... =` OR `export interface` rather than any occurrence.
- **All 9 parity-OK types are enums or simple new structs.** No hand-written structs matched generated structs field-for-field — every hand-written struct had at least Pitfall 2 optional-vs-null drift or a type-name divergence. This suggests ts-rs's default emission shape needs Zod schema adjustments at admin validators (opportunity for Plan 04 regression fixture or follow-on CI gate).

## Task Commits

| # | Task | Commit | Files | Notes |
|---|------|--------|-------|-------|
| 1 | D-12 drift audit | `e4c62c85` | .planning/phases/445-typed-api-contract-rust-ts-codegen/445-03-DRIFT-AUDIT.md (+348) | 30 type sections, 14 safe/flip, 16 held; satisfies Aggregate Verdicts gate |
| 2 | index.ts re-export flip | `ad460745` | packages/shared-types/src/index.ts (+60 / -4) | 13 types flipped to ../generated/, admin tsc + build exit 0 |

## Deviations from Plan

### Auto-fixed issues

**1. [Rule 2 — Missing] Numbered `### N. Type` headings didn't match plan's acceptance regex `^### [A-Z][A-Za-z]+`**

- **Found during:** Task 1 verify pass
- **Issue:** Initial DRIFT-AUDIT.md used `### 1. PodInfo`, `### 2. PodStatus`, ... Plan acceptance regex required `^### [A-Z]` (no number prefix). Grep returned 3 hits instead of 30.
- **Fix:** `sed -i -E 's/^### [0-9]+\. /### /'` — stripped numbering in-place. Post-fix: 33 `^### [A-Z]` matches (30 type sections + 3 Aggregate Verdicts subsections starting with letters).
- **Files modified:** `.planning/phases/445-typed-api-contract-rust-ts-codegen/445-03-DRIFT-AUDIT.md`
- **Committed in:** `e4c62c85` (Task 1) — fix applied pre-commit.

**2. [Rule 2 — Missing] PodFleetStatus bigint arithmetic discovered during admin grep**

- **Found during:** Task 2 Step A sub-gate (grep admin for `uptime_secs`)
- **Issue:** Initial drift audit listed PodFleetStatus as PENDING. The grep found `fleet/page.tsx:92` calling `Math.floor(pod.uptime_secs / 86400)` — bigint would tsc-fail.
- **Fix:** Demoted PodFleetStatus to HELD in DRIFT-AUDIT.md aggregate verdicts + Task 2 index.ts did NOT flip PodFleetStatus.
- **Files modified:** `packages/shared-types/src/index.ts` (kept `PodFleetStatus from './fleet'`)
- **Impact on plan:** No impact — the plan anticipated this gating ("Task 2 Step A sub-gate").

### Authentication Gates

None — code-only plan.

### Out-of-Scope Blockers Observed

- **Audit script bug** (see "Surprises" above). Logged for Plan 04 to fix — not a 445-03 blocker.
- **Pre-existing cargo warnings** (IDEMPOTENCY_CLEANUP_THRESHOLD dead code, unused imports in ac_server_lifecycle.rs / billing_game_status.rs, unreachable statement in notification_outbox.rs) surfaced during `check-generated-types-drift.sh` cargo rebuild. NOT introduced by this plan (pre-existing on `fix/pos-kiosk-disable-20260421` branch). Logged for separate cleanup.

---

**Total deviations:** 2 Rule 2 documented fixes (heading regex + bigint discovery). Zero architectural changes.

## Issues Encountered

- **Heading regex mismatch** — see Deviation 1.
- **PodFleetStatus bigint surprise** — see Deviation 2.
- **CRLF warnings on `git add`.** Expected on Windows Git Bash; non-blocking.
- **Post-commit graphify hooks** re-indexed 1146 files on each commit (~4s each). Expected behavior of `cgp-post-commit-graphify.js`.

## Next Phase Readiness

**Plan 04 (CI drift gate + D-20 regression fixture) can start immediately with:**
1. `packages/shared-types/src/index.ts` is a live dual-write barrel — regression fixture can import migrated types and assert non-zero sample values match Rust-produced JSON.
2. `scripts/check-generated-types-drift.sh` exists and passes — CI just needs to wire it into `.github/workflows/*.yml` or local pre-commit.
3. `scripts/audit-handwritten-vs-generated.sh` has a known grep bug (documented) that Plan 04 can fix as part of the CI hardening.

**Plan 05 (cloud parity) can start with:**
1. Admin `npm run build` already green locally — Bono VPS build should mirror.
2. No server-side Rust changes in Plan 03 — server binary parity unchanged.

**No blockers.**

## Self-Check: PASSED

**Files verified (2/2 exist on disk):**
- `.planning/phases/445-typed-api-contract-rust-ts-codegen/445-03-DRIFT-AUDIT.md` ✓ (348 lines)
- `.planning/phases/445-typed-api-contract-rust-ts-codegen/445-03-SUMMARY.md` ✓ (this file)

**Commits verified (2/2):**
- `e4c62c85` docs(445-03): D-12 drift audit — 30 types reviewed, 14 safe to flip, 16 held
- `ad460745` feat(445-03): flip src/index.ts re-exports to ../generated/ for 13 migrated types

**Plan-level acceptance (all 8):**
- ✓ Drift audit report exists covering every migrated type with explicit verdict
- ✓ `packages/shared-types/src/index.ts` flipped per drift audit verdicts
- ✓ Task 2 pre-condition gate (DRIFT-AUDIT.md + `## Aggregate Verdicts` heading) passed BEFORE index.ts edit
- ✓ `cd packages/shared-types && npx tsc --noEmit` exits 0
- ✓ `cd ../racingpoint-admin && npx tsc --noEmit` exits 0 (TYP-06)
- ✓ `cd ../racingpoint-admin && npm run build` exits 0 (TYP-04)
- ✓ `bash scripts/check-generated-types-drift.sh` exits 0 (zero changes to generated/)
- ✓ Hand-written `.ts` files still exist (dual-write preserved per D-11)

---
*Phase: 445-typed-api-contract-rust-ts-codegen*
*Plan: 03*
*Completed: 2026-04-21*
