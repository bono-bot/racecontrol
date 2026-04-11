---
phase: 361-kiosk-preset-filtering-server-gate
plan: 03
subsystem: ui, api
tags: [next.js, swr, axum, reqwest, drift-detection, content-inventory, admin-dashboard]

requires:
  - phase: 361-01
    provides: "PodInventory + ContentDirsResponse Rust types, /pods/{id}/inventory endpoint, rc-agent /debug/content-dirs probe"
provides:
  - "Admin Content Drift page at /fleet/content-drift with functional TOML-vs-disk drift detection"
  - "Server proxy GET /api/v1/debug/pod-content-dirs/{id} with service key injection"
  - "Fleet API methods podInventory() + podContentDirs() in admin dashboard"
  - "Content Drift nav entry in AdminLayout Fleet section"
  - "OpenAPI spec for /debug/pod-content-dirs/{id} with ContentDirsResponse + GameDirs schemas"
affects: [361-admin-deploy, 366-content-scanners, ui-auditor]

tech-stack:
  added: []
  patterns:
    - "Server-side service key injection proxy (same as /events/recent in v27.0)"
    - "SWR dual-fetch fan-out with Promise.allSettled per pod"
    - "Client-side drift computation: TOML expected - disk actual = missing/extra"
    - "Degrade-open skip for non-enumerable games (cars_enumerable === false)"

key-files:
  created:
    - "racingpoint-admin/src/app/(dashboard)/fleet/content-drift/page.tsx (483 lines)"
    - "racingpoint-admin/src/lib/types.ts (106 lines)"
  modified:
    - "crates/racecontrol/src/api/pods.rs (434 lines, +167 for proxy handler + tests)"
    - "crates/racecontrol/src/api/routes.rs (+4 lines, route registration)"
    - "racingpoint-admin/src/lib/api/fleet.ts (84 lines, +podInventory/podContentDirs)"
    - "racingpoint-admin/src/components/AdminLayout.tsx (254 lines, +nav entry)"
    - "docs/openapi.yaml (+85 lines, endpoint + schema definitions)"

key-decisions:
  - "Proxy handler placed in pods.rs alongside inventory handler rather than creating a new debug.rs module"
  - "Client-side drift computation (not server-side) -- keeps server stateless, computation is trivial"
  - "Used Promise.allSettled for both outer (per-pod) and inner (inventory+dirs) fetches for maximum resilience"

patterns-established:
  - "Admin fleet diagnostic pages: SWR with 30s refreshInterval, refreshWhenHidden: false"
  - "Drift detection: TOML - disk = missing (P0), disk - TOML = extra (informational)"

requirements-completed: [GLD-A-04]

duration: 16min
completed: 2026-04-11
---

# Phase 361 Plan 03: Admin Content Drift Page + Server Proxy Summary

**Admin Content Drift page with functional TOML-vs-disk drift detection, server proxy with service key injection, and OpenAPI spec**

## Performance

- **Duration:** 16 min
- **Started:** 2026-04-11T01:40:31Z
- **Completed:** 2026-04-11T01:56:57Z
- **Tasks:** 3 code tasks (pre-committed) + 1 OpenAPI/fix task
- **Files modified:** 7 (across racecontrol + racingpoint-admin repos)

## Accomplishments

- Server proxy endpoint `/api/v1/debug/pod-content-dirs/{id}` proxies rc-agent disk scan with service key injection (3s timeout, 404/503 error handling)
- Admin Content Drift page at `/fleet/content-drift` with dual-fetch fan-out (inventory + content-dirs per pod), client-side drift computation, DriftStatusBadge (OK/DRIFT/UNREACHABLE), auto-expanding drift rows via `<details open>`, 30s SWR refresh
- Nav entry "Content Drift" correctly placed between "Fleet Health" and "Metrics" in AdminLayout Fleet section
- OpenAPI spec updated with endpoint + ContentDirsResponse + GameDirs schemas
- All TypeScript field names match Rust serde output exactly (snake_case cross-boundary rule)
- Zero `any` types, zero hardcoded hex values, semantic `<table>` + `<details>` HTML

## Task Commits

Code was pre-committed in a prior session. This execution verified correctness and added missing artifacts:

1. **Task 0: Server proxy endpoint** - `e180f3c2` (feat) -- racecontrol repo
2. **Task 1: Fleet API + types + nav entry** - `c4f244f` (feat) -- racingpoint-admin repo
3. **Task 2: Content Drift page** - `b4d4112` (feat) -- racingpoint-admin repo
4. **OpenAPI spec + compile fix** - `6e250706` (fix) -- racecontrol repo

## Files Created/Modified

- `crates/racecontrol/src/api/pods.rs` - proxy handler + 2 unit tests (wire format + serialization)
- `crates/racecontrol/src/api/routes.rs` - route registration in STAFF_ROUTES
- `racingpoint-admin/src/app/(dashboard)/fleet/content-drift/page.tsx` - Full drift detection page (483 lines)
- `racingpoint-admin/src/lib/types.ts` - PodInventory + ContentDirsResponse + GameDirs + ContentDriftRow TS types
- `racingpoint-admin/src/lib/api/fleet.ts` - podInventory() + podContentDirs() methods
- `racingpoint-admin/src/components/AdminLayout.tsx` - Content Drift nav entry
- `docs/openapi.yaml` - /debug/pod-content-dirs/{id} endpoint + schema definitions

## Verification Evidence

### Rust (racecontrol repo)
- `cargo build -p racecontrol-crate --lib` -- compiles (20 warnings, 0 errors)
- `cargo test -p rc-common --lib inventory_types` -- 4/4 pass
- `cargo test -p racecontrol-crate --lib api::pods` -- 6/6 pass
- Route registered: `grep "pod-content-dirs" routes.rs` returns line 352

### TypeScript (racingpoint-admin repo)
- `npx tsc --noEmit` -- clean (0 errors)
- `npx eslint` on all 4 files -- clean (0 warnings)
- `grep "Content Drift" AdminLayout.tsx` -- 1 hit at line 43
- `grep "<table" page.tsx` -- 1 hit (semantic HTML)
- `grep "<details" page.tsx` -- 2 hits (drift row expansion)
- `grep "refreshInterval: 30000" page.tsx` -- 1 hit
- `grep ": any" page.tsx` -- 0 hits
- `grep "bg-\[#" page.tsx` -- 0 hits (token compliance)

### NOT TESTED (deferred per user directive)
- Deploy to server .23 + cloud (Task 3 skipped)
- Live curl verification of proxy endpoint
- Real drift test (intentional rename on pod)
- Visual verification screenshots
- Admin rebuild + deploy
- NYQUIST-AUDIT.md precondition (skipped per user directive)

## Decisions Made

- Proxy handler in pods.rs (not debug.rs) -- consolidates all pod-related handlers
- Client-side drift computation keeps server stateless; computation is set difference, trivial for 8 pods
- Promise.allSettled at both layers ensures one pod failure never blocks the other 7

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Removed duplicate append_suspect_reason function in bot_coordinator.rs**
- **Found during:** Task 0 verification (cargo build)
- **Issue:** Pre-existing E0428 compile error from Phase 364 -- two definitions of `append_suspect_reason` at lines 159 and 516
- **Fix:** Removed the duplicate at line 516 (kept the one at line 159 which has the empty session_id guard)
- **Files modified:** `crates/racecontrol/src/bot_coordinator.rs`
- **Verification:** `cargo build -p racecontrol-crate --lib` compiles successfully
- **Committed in:** `6e250706`

**2. [Rule 2 - Missing Critical] Added OpenAPI spec for /debug/pod-content-dirs/{id}**
- **Found during:** Task 0 done criteria check
- **Issue:** Plan action item 6 requires OpenAPI spec entry, but it was missing from docs/openapi.yaml
- **Fix:** Added endpoint definition + ContentDirsResponse + GameDirs schemas
- **Files modified:** `docs/openapi.yaml`
- **Verification:** Schema $ref resolves correctly
- **Committed in:** `6e250706`

---

**Total deviations:** 2 auto-fixed (1 blocking compile error, 1 missing spec)
**Impact on plan:** Both fixes necessary for correctness. No scope creep.

## Known Stubs

None. All data sources are wired:
- podInventory() calls `/api/v1/pods/{id}/inventory` (live server endpoint from 361-01)
- podContentDirs() calls `/api/v1/debug/pod-content-dirs/{id}` (proxy endpoint from this plan)
- Drift computation is functional (not placeholder)
- Degrade-open games intentionally skipped (design, not stub)

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- Task 3 (deploy) is deferred -- racecontrol binary + admin rebuild needed before live verification
- gsd-ui-auditor must produce UI-REVIEW.md post-deploy
- Phase 366 will add proper per-game content scanners for ACR/LMU/AC EVO (currently degrade-open)
- URL clarification: /fleet/content-drift satisfies ROADMAP's /admin/content-drift criterion

---
*Phase: 361-kiosk-preset-filtering-server-gate*
*Completed: 2026-04-11*
