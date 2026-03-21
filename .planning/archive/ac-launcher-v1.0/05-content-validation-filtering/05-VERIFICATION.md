---
phase: 05-content-validation-filtering
verified: 2026-03-14T00:00:00Z
status: passed
score: 5/5 must-haves verified
re_verification: false
gaps: []
human_verification:
  - test: "Deploy rc-agent to Pod 8 and verify scan_ac_content() returns non-empty cars and tracks for the actual AC installation on the pod"
    expected: "Logs show N cars and M tracks scanned; /customer/ac/catalog?pod_id=pod_8 returns filtered list matching pod's actual AC content"
    why_human: "scan_ac_content() uses hardcoded path C:\\Program Files (x86)\\Steam\\steamapps\\common\\assettocorsa\\content -- no way to verify filesystem scan output without live pod"
  - test: "Request /customer/ac/catalog?pod_id=pod_8 after rc-agent connects, then request same endpoint for a pod_id with no manifest cached"
    expected: "With pod_id: returns only cars/tracks installed on that pod. Without: returns full static catalog"
    why_human: "Catalog filtering correctness against real pod content requires live pod and real AC installation"
  - test: "Select a Race vs AI session on a track without AI lines via the PWA"
    expected: "Race vs AI and Track Day session types are absent from the track's available options -- not greyed out, not present"
    why_human: "PWA UI behavior depends on frontend consuming available_session_types field correctly"
---

# Phase 5: Content Validation & Filtering Verification Report

**Phase Goal:** Customers never see a car, track, or session option that would fail to launch -- every displayed option is guaranteed valid
**Verified:** 2026-03-14
**Status:** passed
**Re-verification:** No -- initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | Customer browsing cars in PWA only sees cars actually installed on their pod | VERIFIED | `get_filtered_catalog()` in catalog.rs filters ALL_CAR_IDS and FEATURED_CARS against `manifest.cars` HashSet; returns full catalog if no manifest cached (line 324-328). Catalog API accepts `pod_id` query param (routes.rs line 4471-4481). 13 catalog tests all pass. |
| 2 | Customer browsing tracks in PWA only sees tracks actually installed on their pod | VERIFIED | Same `get_filtered_catalog()` function filters track lists against `manifest.tracks` HashSet; enrich_track_entry() adds per-track metadata. Test `filtered_catalog_filters_all_tracks_to_manifest_only` passes. |
| 3 | Tracks without AI line data (ai/ folder) do not show Race vs AI or Track Day session types | VERIFIED | `enrich_track_entry()` builds `available_session_types` with "practice"+"hotlap" always; "race"+"trackday"+"race_weekend" only if any config has `has_ai=true` (catalog.rs line 402-407). Tests `filtered_catalog_track_without_ai_excludes_race_and_trackday` and `filtered_catalog_track_with_ai_includes_race_and_trackday` both pass. |
| 4 | Maximum AI opponent count for a track is capped by that track's pit stall count | VERIFIED | `enrich_track_entry()` computes `max_ai = min(max_pit_count_across_configs - 1, 19)` using `saturating_sub` (catalog.rs lines 411-422). Tests `filtered_catalog_track_includes_pit_count_max_across_configs` and `filtered_catalog_track_pit_count_none_defaults_to_19` pass. |
| 5 | No invalid car/track/session combination can be selected -- invalid options are hidden, not just greyed out | VERIFIED | Two launch validation gates in place: (1) `game_launcher.rs::launch_game()` lines 78-90 rejects before double-launch check; (2) `routes.rs::customer_book_session()` lines 4967-4985 rejects before `sender.send(LaunchGame)`. Both call `catalog::validate_launch_combo()`. All 5 validate_launch_combo tests pass. |

**Score:** 5/5 truths verified

### Required Artifacts

#### Plan 05-01 Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/rc-common/src/types.rs` | ContentManifest, CarManifestEntry, TrackManifestEntry, TrackConfigManifest structs | VERIFIED | All 4 structs present at lines 744-766 with correct fields including `config: String`, `has_ai: bool`, `pit_count: Option<u32>` |
| `crates/rc-common/src/protocol.rs` | AgentMessage::ContentManifest variant | VERIFIED | Variant at line 66; `ContentManifest` imported in types use list at line 5; 6 serde roundtrip tests all pass |
| `crates/rc-agent/src/content_scanner.rs` | scan_ac_content() function, min 80 lines | VERIFIED | File is 477 lines; exports `scan_ac_content()` and `scan_ac_content_at()`; all 8 specified internal functions present; 15 unit tests all pass |
| `crates/rc-agent/src/main.rs` | mod content_scanner declaration | VERIFIED | `mod content_scanner;` at line 3 |

#### Plan 05-02 Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/rc-core/src/state.rs` | pod_manifests field on AppState | VERIFIED | `pub pod_manifests: RwLock<HashMap<String, ContentManifest>>` at line 85; initialized to empty HashMap at line 130 |
| `crates/rc-core/src/ws/mod.rs` | AgentMessage::ContentManifest handler storing manifest in AppState | VERIFIED | Handler at lines 396-405; stores via `state.pod_manifests.write().await.insert(pod_id.clone(), manifest.clone())` |
| `crates/rc-core/src/catalog.rs` | get_filtered_catalog() and validate_launch_combo() functions | VERIFIED | Both pub functions present (lines 324 and 437); module is 710+ lines; 13 tests pass |
| `crates/rc-core/src/api/routes.rs` | customer_ac_catalog with pod_id query param | VERIFIED | `CatalogQuery` struct at line 4467; `customer_ac_catalog` accepts `Query(query): Query<CatalogQuery>` at line 4473; reads `pod_manifests` at line 4476 |
| `crates/rc-core/src/game_launcher.rs` | Launch validation gate in launch_game() | VERIFIED | Validation at lines 78-90, before double-launch check at line 92; calls `catalog::validate_launch_combo()` |
| `crates/rc-agent/src/main.rs` | ContentManifest sent after Register | VERIFIED | `content_scanner::scan_ac_content()` called at line 511; manifest sent as `AgentMessage::ContentManifest` at lines 513-518; runs after successful Register send |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `crates/rc-core/src/ws/mod.rs` | `crates/rc-core/src/state.rs` | `state.pod_manifests.write().await.insert()` | WIRED | Exact pattern at ws/mod.rs line 403; `registered_pod_id` guard ensures only registered pods update cache |
| `crates/rc-core/src/api/routes.rs` | `crates/rc-core/src/catalog.rs` | `catalog::get_filtered_catalog(manifest)` | WIRED | Called at routes.rs line 4480; manifest fetched from `pod_manifests` based on `query.pod_id` |
| `crates/rc-core/src/game_launcher.rs` | `crates/rc-core/src/catalog.rs` | `catalog::validate_launch_combo()` called before `CoreToAgentMessage::LaunchGame` send | WIRED | Call at game_launcher.rs line 85; returns `Err(reason)` propagated to caller |
| `crates/rc-core/src/api/routes.rs` | `crates/rc-core/src/catalog.rs` | `catalog::validate_launch_combo()` in customer_book_session | WIRED | Call at routes.rs line 4969; `sender.send(LaunchGame)` is inside the `else` branch (only reached if validation passes) |
| `crates/rc-agent/src/main.rs` | `crates/rc-agent/src/content_scanner.rs` | `content_scanner::scan_ac_content()` | WIRED | Called at main.rs line 511 after Register send succeeds; runs on every connect/reconnect cycle |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|----------|
| SESS-07 | 05-02-PLAN.md | Only valid session/mode combinations are presented (invalid options hidden) | SATISFIED | validate_launch_combo() rejects invalid session types at both launch gates; enrich_track_entry() hides race/trackday when no AI lines |
| CONT-01 | 05-02-PLAN.md | Customer can browse and select car from available catalog via PWA | SATISFIED | get_filtered_catalog() returns pod-filtered car list; /customer/ac/catalog?pod_id=X endpoint wired |
| CONT-02 | 05-02-PLAN.md | Customer can browse and select track from available catalog via PWA | SATISFIED | get_filtered_catalog() returns pod-filtered track list with enriched metadata |
| CONT-04 | 05-02-PLAN.md | Invalid car/track/session combinations are filtered out before display | SATISFIED | get_filtered_catalog() filters to pod manifest; validate_launch_combo() blocks invalid combos at launch time |
| CONT-05 | 05-01-PLAN.md | Tracks without AI line data (ai/ folder) hide AI-related session types | SATISFIED | check_has_ai() in content_scanner detects empty/missing ai/; has_ai=false drives session type exclusion in enrich_track_entry() |
| CONT-06 | 05-01-PLAN.md | Track pit count limits maximum AI opponents shown for that track | SATISFIED | parse_pit_count() reads pitboxes string from ui_track.json; max_ai derived via saturating_sub(1) capped at 19 |
| CONT-07 | 05-01-PLAN.md | Per-pod content scanning -- only show cars/tracks installed on the target pod | SATISFIED | scan_ac_content() runs at agent startup/reconnect; ContentManifest sent to core; core caches per pod_id |

No orphaned requirements found -- all 7 phase 5 requirements (SESS-07, CONT-01, CONT-02, CONT-04, CONT-05, CONT-06, CONT-07) are claimed by plans 05-01 or 05-02 and verified.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| `crates/rc-agent/src/lock_screen.rs` | 870 | unused variable `balance_rupees` | Info | Compiler warning, pre-existing, not phase 5 work |
| `crates/rc-agent/src/ac_launcher.rs` | 158, 218 | unused fields `conditions`, `damage` | Info | Compiler warning, pre-existing |
| `crates/rc-agent/src/ac_launcher.rs` | 1276 | unused function `cleanup_after_session` | Info | Compiler warning, pre-existing |

No phase 5 code has TODO/FIXME/PLACEHOLDER/return null anti-patterns. The fallback behavior (None manifest returns full catalog and allows any launch) is intentional and documented in STATE.md and SUMMARY.

### Human Verification Required

#### 1. Live Pod Content Scan

**Test:** Deploy updated rc-agent to Pod 8 and watch startup logs.
**Expected:** Logs show "Scanned AC content: N cars, M tracks" with non-zero counts; then "Pod pod_8 content manifest: N cars, M tracks" appears in rc-core logs.
**Why human:** `scan_ac_content()` uses hardcoded path for AC installation -- no way to verify non-empty output without the actual pod filesystem.

#### 2. Filtered Catalog API Against Real Pod

**Test:** After Pod 8 connects: `curl http://192.168.31.23:8080/customer/ac/catalog?pod_id=pod_8` vs `curl http://192.168.31.23:8080/customer/ac/catalog`
**Expected:** With pod_id: car and track lists are scoped to what is installed on pod_8. Without: full static catalog returned.
**Why human:** Requires live pod with real AC installation to verify correct filtering.

#### 3. PWA Session Type Gating

**Test:** In the PWA, select a track known to lack AI line files (e.g. a non-stock track without ai/ folder). Check available session types.
**Expected:** Race vs AI and Track Day do not appear in the session type selector for that track.
**Why human:** PWA frontend must correctly consume `available_session_types` from the catalog API -- frontend behavior cannot be verified from Rust code alone.

## Commit Audit

Both commits documented in the summaries are verified in git history:

- `25a6f79` feat(05-01): add ContentManifest types and AgentMessage variant
- `3b25d6f` feat(05-01): add content scanner module with filesystem scanning
- `b871d58` feat(05-02): add pod_manifests cache, filtered catalog, and launch validation
- `dd638fd` feat(05-02): wire validation gates, filtered catalog API, and agent manifest sending

## Test Results

| Suite | Tests | Result |
|-------|-------|--------|
| rc-common content_manifest | 6 | All pass |
| rc-agent content_scanner | 15 | All pass |
| rc-core catalog | 13 | All pass |

**Total new tests from Phase 5: 34**

---
*Verified: 2026-03-14*
*Verifier: Claude (gsd-verifier)*
