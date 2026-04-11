---
phase: 361-kiosk-preset-filtering-server-gate
verified: 2026-04-11T08:30:00+05:30
status: gaps_found
score: 3/4 success criteria verified
re_verification: false
gaps:
  - truth: "ROADMAP plan checkboxes 361-02-PLAN and 361-03-PLAN remain unchecked despite both plans being code-complete and deployed"
    status: partial
    reason: "Standing rule violation: ROADMAP checkbox sync on completion was not done. ROADMAP.md still shows '- [ ] 361-02-PLAN' and '- [ ] 361-03-PLAN'. REQUIREMENTS.md still shows GLD-A-02 and GLD-A-04 as [ ] unchecked."
    artifacts:
      - path: ".planning/ROADMAP.md"
        issue: "Lines 1307-1308: '- [ ] 361-02-PLAN' and '- [ ] 361-03-PLAN' not updated to [x] after completion"
      - path: ".planning/milestones/v46.0-REQUIREMENTS.md"
        issue: "GLD-A-02 (line 32) and GLD-A-04 (line 34) still show [ ] despite 361-02 and 361-03 being deployed"
    missing:
      - "Update ROADMAP.md: change '- [ ] 361-02-PLAN' to '- [x] 361-02-PLAN'"
      - "Update ROADMAP.md: change '- [ ] 361-03-PLAN' to '- [x] 361-03-PLAN'"
      - "Update REQUIREMENTS.md: change '[ ] GLD-A-02' to '[x] GLD-A-02' with deploy evidence note"
      - "Update REQUIREMENTS.md: change '[ ] GLD-A-04' to '[x] GLD-A-04' with deploy evidence note"
human_verification:
  - test: "Open kiosk staff wizard at :3300/kiosk/staff, select Pod 1, select Assetto Corsa. Verify car dropdown shows only TOML-declared cars (365 entries) and not unlisted cars."
    expected: "Car dropdown is filtered to pod-installed content. Unlisted car keys do not appear."
    why_human: "Requires a running kiosk instance and visual confirmation of dropdown content filtering in the browser."
  - test: "In the wizard, select an experience preset that is marked 'invalid' in combo_validation_flags for that pod. Verify 'Start Session' button is disabled with an inline red reason text."
    expected: "Button disabled, red reason text visible below: 'This experience is not available on this pod'"
    why_human: "Requires knowing which preset_ids have a 'CarMissing' or other non-Available status in the live DB for the selected pod, and visual confirmation of the disabled state."
  - test: "Disconnect or block the inventory API and open the wizard. Verify InventoryStatusBanner ('Pod inventory unreachable') appears with Retry button, and Start Session is disabled."
    expected: "Banner visible with role=alert, Start Session disabled, aria-describedby set on button pointing at banner id."
    why_human: "Requires network manipulation and browser inspection to confirm aria attribute presence."
  - test: "Navigate to admin /fleet/content-drift page. Verify 8-pod table renders with OK/DRIFT/UNREACHABLE statuses, nav entry is between Fleet Health and Metrics, and timestamps show HH:MM IST format."
    expected: "Page renders with 8 rows, Content Drift nav entry visible between Fleet Health and Metrics."
    why_human: "Requires staff-authenticated browser session and visual confirmation of page layout and nav placement."
---

# Phase 361: Kiosk Preset Filtering + Server Gate Verification Report

**Phase Goal:** Prevent invalid car/track combos at source. Wire unused `presetValidity`, filter by pod inventory, reject at API.
**Verified:** 2026-04-11T08:30:00 IST
**Status:** gaps_found (tracking gaps only — all code and functionality is deployed; ROADMAP/REQUIREMENTS.md checkboxes were not updated)
**Re-verification:** No — initial verification

---

## Goal Achievement

### Observable Truths (from ROADMAP Success Criteria)

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | Kiosk car/track dropdowns filter to installed-on-pod only | VERIFIED | `SetupWizard.tsx` lines 203-258: `inventoryAllowedCars`/`inventoryAllowedTracks` memos filter dropdowns from `podInventoryData.games[].cars/tracks`. Degrade-open when empty. |
| 2 | Invalid combos disable "Start Session" with `presetValidity` reason surfaced | VERIFIED | `canLaunch = inventoryFetchState === "ok" && presetIsValid` (line 266). `presetIsValid` reads from `presetValidity` prop (populated from `combo_validation_flags` DB via old Phase 320 endpoint). Inline red `launchBlockReason` text at line 1113-1115. |
| 3 | Server `/sessions/start` returns 422 with `{reason, suggestion}` on bypass attempt | VERIFIED | `validate_session_tuple()` in `session_validity.rs` called in `launch_game` handler at routes.rs:5803. Live test confirmed: `CAR_NOT_AVAILABLE` fired on server .23 with fake car. HTTP 200 wrapper with `body.status=422` for backward compat (DEV-1). |
| 4 | Admin `/admin/content-drift` lists pods with inventory drift | VERIFIED | Page at `/fleet/content-drift` exists (483 lines). Functional drift computation: `computeDrift()` diffs TOML `cars/tracks` vs `cars_on_disk/tracks_on_disk`. SWR 30s refresh. Semantic `<table>` + `<details open>` for drift rows. Nav entry between Fleet Health and Metrics in AdminLayout.tsx. |

**Score:** 4/4 success criteria code-verified

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/racecontrol/src/validation/session_validity.rs` | `fn validate_session_tuple` pure function | VERIFIED | 267 lines, `pub fn validate_session_tuple` at line 30, 11 unit tests |
| `crates/racecontrol/src/api/pods.rs` | `fn pod_inventory_handler` + `fn pod_content_dirs_proxy_handler` | VERIFIED | 434 lines, both handlers present |
| `crates/rc-common/src/inventory_types.rs` | `struct PodInventory` shared types | VERIFIED | 167 lines, `struct PodInventory` at line 21 |
| `deploy/configs/rc-agent-pod1.toml` | `[content.assetto_corsa]` with populated cars/tracks | VERIFIED | 565 lines, `[content.assetto_corsa]` at line 64 with 365+ car entries; all other games degrade-open |
| `crates/rc-agent/src/remote_ops.rs` | GET /debug/content-dirs handler | VERIFIED | Route at line 208, handler at line 1423 |
| `kiosk/src/components/InventoryStatusBanner.tsx` | Hard-block banner, min 50 lines | VERIFIED | 80 lines, `role="alert"` at line 35, Retry button with focus management |
| `kiosk/src/lib/api.ts` | `podInventoryFull()` with staff JWT | VERIFIED | `podInventoryFull` at line 576, uses `fetchApi()` which attaches `Authorization: Bearer` from `sessionStorage("kiosk_staff_token")` |
| `tests/e2e/playwright/kiosk/setup-wizard-inventory.spec.ts` | 3 Playwright tests (happy/invalid/unreachable) | VERIFIED | 592 lines, 3 `test()` blocks |
| `racingpoint-admin/src/app/(dashboard)/fleet/content-drift/page.tsx` | Content drift page min 180 lines | VERIFIED | 483 lines, `'use client'`, `useSWR`, `computeDrift()`, `<table>`, `<details open>` |
| `racingpoint-admin/src/components/AdminLayout.tsx` | "Content Drift" nav entry | VERIFIED | `{ href: '/fleet/content-drift', label: 'Content Drift' }` at line 43 |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `routes.rs` STAFF_ROUTES | `pod_inventory_handler` | `.route("/pods/{id}/inventory", get(...))` | VERIFIED | Line 351 in routes.rs |
| `routes.rs` STAFF_ROUTES | `pod_content_dirs_proxy_handler` | `.route("/debug/pod-content-dirs/{id}", get(...))` | VERIFIED | Line 355 in routes.rs |
| `routes.rs` `launch_game` handler | `validate_session_tuple` | Early-return on `Err` before state lock | VERIFIED | Line 5803 in routes.rs, returns `{status:422, code, reason, suggestion}` JSON |
| `SetupWizard.tsx` | `/api/v1/pods/{id}/inventory` | `useEffect` on `selectedPodId` calling `api.podInventoryFull(podIdNum)` | VERIFIED | `fetchInventory()` at line 87, `podInventoryFull` call at line 99 |
| `SetupWizard.tsx` | `InventoryStatusBanner` | Conditional render when `inventoryFetchState === "error"` | VERIFIED | Lines 347-354: `{inventoryFetchState === "error" && <InventoryStatusBanner />}` |
| Start Session button | `canLaunch` state | `disabled={isLaunching || !canLaunch}` + conditional `aria-describedby` | VERIFIED | Lines 1101-1103, conditional spread `{...(inventoryFetchState === "error" && {"aria-describedby": "inventory-status-banner"})}` |
| `content-drift/page.tsx` | `/pods/{id}/inventory` + `/debug/pod-content-dirs/{id}` | SWR fan-out via `fleetApi.podInventory(i)` + `fleetApi.podContentDirs(i)` | VERIFIED | Lines 116-117 in page.tsx |
| `AdminLayout.tsx` Fleet section | `/fleet/content-drift` route | `{ href: '/fleet/content-drift', label: 'Content Drift' }` in items array | VERIFIED | Line 43 in AdminLayout.tsx |

### Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
|----------|---------------|--------|--------------------|--------|
| `SetupWizard.tsx` car dropdown | `inventoryAllowedCars` | `api.podInventoryFull()` → `/pods/{id}/inventory` → `load_pod_inventory()` → TOML `rc-agent-pod{N}.toml` | YES — all 8 TOMLs populated with 365-436 AC cars per pod via SSH `dir /B` enumeration | FLOWING |
| `SetupWizard.tsx` `canLaunch` | `presetValidity` prop | `api.podInventory()` (Phase 320) → `/fleet/pod-inventory/{id}` → `combo_validation_flags` DB table → populated by `game_inventory.rs` on pod report-back | YES — DB populated at runtime from pod inventory reports (pre-existing Phase 320 mechanism) | FLOWING |
| `SetupWizard.tsx` hard-block | `inventoryFetchState` | `api.podInventoryFull()` → fetch result | YES — real HTTP fetch with error state on failure | FLOWING |
| `content-drift/page.tsx` drift table | `computeDrift()` result | `fleetApi.podInventory()` (TOML) + `fleetApi.podContentDirs()` (live disk) → diff | YES — both sources return real data (TOML populated, rc-agent disk scan live) | FLOWING |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| `/pods/{id}/inventory` returns 401 without JWT | `curl -s -o /dev/null -w '%{http_code}' http://192.168.31.23:8080/api/v1/pods/1/inventory` | 401 (per SUMMARY evidence) | PASS (SUMMARY-verified) |
| `launch_game` with fake car returns `CAR_NOT_AVAILABLE` | Live test on server .23 with `nonexistent_ferrari_9999_fake` | `{"code":"CAR_NOT_AVAILABLE","status":422}` (per NYQUIST-AUDIT.md) | PASS (live-verified) |
| `/debug/pod-content-dirs/{id}` returns 401 without JWT | `curl -s -o /dev/null -w '%{http_code}' http://192.168.31.23:8080/api/v1/debug/pod-content-dirs/1` | 401 (per SUMMARY evidence) | PASS (SUMMARY-verified) |
| Admin `/fleet/content-drift` returns 307 redirect (no JWT) | `curl -s -o /dev/null -w '%{http_code}' http://192.168.31.23:3201/fleet/content-drift` | 307 (per SUMMARY evidence) | PASS (redirect to login — correct) |
| Kiosk staff page serves new build with banner copy | `curl -s http://192.168.31.23:3300/kiosk/staff \| grep "Pod inventory unreachable"` | HTML contains banner copy (per SUMMARY: BUILD_ID `0ncViMD8v0EJ4rBBxzjFo`) | PASS (SUMMARY-verified) |

Note: Spot-checks marked "SUMMARY-verified" use evidence from the SUMMARY.md files. Live re-verification would require staff JWT and running services.

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|----------|
| GLD-A-01 | 361-01 | Kiosk reads pod inventory from `/pods/{id}/inventory`, filters dropdowns | SATISFIED | `SetupWizard.tsx` `inventoryAllowedCars/Tracks` memos. All 8 TOMLs populated. `podInventoryFull()` in api.ts. DEPLOYED to .23 and cloud. |
| GLD-A-02 | 361-02 | Kiosk surfaces `presetValidity` — invalid combos disable Start Session with inline reason | SATISFIED | `canLaunch = inventoryFetchState === "ok" && presetIsValid` (line 266). `presetValidity` prop from Phase 320 `combo_validation_flags` DB. Inline `launchBlockReason` at lines 1113-1115. DEPLOYED. **REQUIREMENTS.md checkbox still `[ ]` — needs update.** |
| GLD-A-03 | 361-01 | Server returns 422 on invalid `(pod_id, game, car, track, ai_count)` tuple | SATISFIED | `validate_session_tuple()` in `launch_game` handler. Live test: `CAR_NOT_AVAILABLE` on server .23. HTTP 200 wrapper with `body.status=422` for backward compat. DEPLOYED. |
| GLD-A-04 | 361-03 | Admin `/admin/content-drift` lists pods with inventory drift | SATISFIED | `/fleet/content-drift` page (483 lines). Functional `computeDrift()`. 8-pod table. Nav entry. DEPLOYED to .23:3201 and cloud. **REQUIREMENTS.md checkbox still `[ ]` — needs update.** |

**Orphaned requirements:** None — all 4 GLD-A-01..04 requirements claimed and implemented.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| `.planning/ROADMAP.md` | 1307-1308 | `- [ ] 361-02-PLAN` and `- [ ] 361-03-PLAN` still unchecked after deployment | Warning | Tracking divergence — future audits would incorrectly report Phase 361 as 1/3 complete |
| `.planning/milestones/v46.0-REQUIREMENTS.md` | 32, 34 | `[ ] GLD-A-02` and `[ ] GLD-A-04` still unchecked despite deployment | Warning | Same tracking divergence — status report for v46.0 milestone would show 2/4 Phase A requirements incomplete |

No code-level stubs or anti-patterns found:
- `bg-[#E10600]` in `SetupWizard.tsx`: 0 hits (migrated to `bg-rp-red`)
- `bg-[#E10600]` in `content-drift/page.tsx`: 0 hits (rp-* tokens only)
- `: any` in `content-drift/page.tsx`: 0 hits
- TODO/FIXME in production code: not found in key files
- All return values in API handlers use real data sources (TOMLs, DB, live disk scan)

### Human Verification Required

#### 1. Kiosk dropdown filtering in browser

**Test:** Open kiosk staff wizard at `http://192.168.31.23:3300/kiosk/staff`, log in as staff, select Pod 1, select Assetto Corsa as game, navigate to the car selection step.
**Expected:** Car dropdown shows only the 365 cars listed in `rc-agent-pod1.toml [content.assetto_corsa]`. Any car NOT in the TOML (e.g., a modded car not on that pod) should not appear.
**Why human:** Requires live kiosk session and visual/functional confirmation of dropdown filtering. The code is correctly wired but filtering depends on runtime data flow from server.

#### 2. presetValidity disabling Start Session

**Test:** Using the staff wizard, select a pod and experience preset that is known to have `status != "Available"` in `combo_validation_flags` DB. Observe the Start Session button state.
**Expected:** Button is disabled with inline red text "This experience is not available on this pod".
**Why human:** Requires knowing which preset_id has a non-Available status in the live production DB, and visual confirmation of the disabled state and reason text display.

#### 3. InventoryStatusBanner error state and retry

**Test:** Block the `/api/v1/pods/1/inventory` endpoint (e.g., via a test flag or by temporarily making it return 500). Open wizard, select Pod 1.
**Expected:** InventoryStatusBanner appears with title "Pod inventory unreachable", Retry button auto-focused, Start Session disabled. Clicking Retry and restoring the endpoint should unmount the banner and re-enable Start Session.
**Why human:** Requires network manipulation to trigger the error state, and browser devtools inspection to confirm `aria-describedby` presence/absence on the Start button.

#### 4. Admin /fleet/content-drift page visual and functional check

**Test:** Log into admin portal at `http://192.168.31.23:3201` with staff credentials, navigate to Fleet > Content Drift.
**Expected:** 8-pod table renders. "Content Drift" entry visible in Fleet section between "Fleet Health" and "Metrics". Timestamps show HH:MM IST format. OK rows compact, drift rows expanded (if any pods have drift).
**Why human:** Requires staff-authenticated browser session. The 307 redirect to login (confirmed in SUMMARY) means the page requires auth — cannot verify page content via curl.

#### 5. gsd-ui-auditor gate (both plans)

**Test:** Run `gsd-ui-auditor` to produce `UI-REVIEW.md` for both 361-02 (kiosk wizard) and 361-03 (admin drift page).
**Expected:** All 6 UI audit dimensions PASS for both pages.
**Why human:** Both PLAN.md files specify `gate: ui-auditor-required-post-exec`. Neither SUMMARY documents a completed `UI-REVIEW.md`. This gate is blocking milestone ship per CLAUDE.md subagent gates rule.

---

## Gaps Summary

**The phase goal is functionally achieved** — all four success criteria are implemented, deployed to both server .23 and cloud, and data flows are wired to real sources (not stubs). The code passes all programmatic checks.

**The only gaps are tracking/process gaps:**

1. **ROADMAP plan checkboxes not updated** — ROADMAP.md lines 1307-1308 still show `[ ]` for 361-02-PLAN and 361-03-PLAN, despite both being deployed. This is a standing rule violation ("ROADMAP plan checkbox sync on completion"). Future phase audit tools reading the ROADMAP would incorrectly see Phase 361 as 1/3 complete.

2. **REQUIREMENTS.md checkboxes not updated** — GLD-A-02 and GLD-A-04 still show `[ ]` in `v46.0-REQUIREMENTS.md` despite both being satisfied. This makes the v46.0 milestone progress report show 2/4 Phase A requirements incomplete when all 4 are in fact deployed.

3. **UI-REVIEW.md not produced** — Both plans specify `gate: ui-auditor-required-post-exec`. The `gsd-ui-auditor` subagent was not invoked. Per CLAUDE.md standing rules, "No frontend phase ships without UI-SPEC.md AND UI-REVIEW.md." This is a blocking gate for milestone ship (not for phase code completeness).

**Fixing gaps 1 and 2** is a one-commit ROADMAP/REQUIREMENTS update.
**Fixing gap 3** requires invoking `gsd-ui-auditor` as a separate agent step.

No code changes are needed — all implementations are substantive, wired, and deployed.

---

_Verified: 2026-04-11T08:30:00 IST_
_Verifier: Claude (gsd-verifier)_
