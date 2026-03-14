---
phase: 13-leaderboard-core
verified: 2026-03-15T18:00:00Z
status: gaps_found
score: 4/5 must-haves verified
gaps:
  - truth: "User can filter leaderboard by car"
    status: partial
    reason: "Backend API supports ?car= query param on /public/leaderboard/{track}, but the leaderboard PWA page has no car filter dropdown. Success criterion 1 requires user-facing car filtering."
    artifacts:
      - path: "pwa/src/app/leaderboard/public/page.tsx"
        issue: "No car filter dropdown or input; only sim_type and show_invalid controls present"
    missing:
      - "Add car filter dropdown to leaderboard/public/page.tsx (populate from unique cars in response or track stats)"
  - truth: "Vehicle records endpoint ignores sim_type query parameter from PWA"
    status: partial
    reason: "PWA publicApi.vehicleRecords passes sim_type param, but backend public_vehicle_records handler does not accept Query params -- only Path(car). The sim_type is silently ignored. Records page still works but filtering by sim on vehicle records view has no effect."
    artifacts:
      - path: "crates/rc-core/src/api/routes.rs"
        issue: "public_vehicle_records at line 8367 takes only Path(car), no Query extractor for sim_type"
    missing:
      - "Add VehicleRecordsQuery struct with sim_type: Option<String> and add WHERE sim_type = ? clause to public_vehicle_records handler"
human_verification:
  - test: "Open app.racingpoint.cloud on a phone, navigate to each page"
    expected: "All pages load without login, data renders, mobile layout works at 375px"
    why_human: "Visual rendering, touch interactions, and actual mobile viewport behavior cannot be verified programmatically"
  - test: "Trigger a track record being broken while previous holder has email"
    expected: "Previous holder receives email with track, car, old time, new time, new holder name, leaderboard link"
    why_human: "Email delivery via send_email.js requires running Node script and checking inbox"
---

# Phase 13: Leaderboard Core Verification Report

**Phase Goal:** Customers can browse public leaderboards, circuit records, vehicle records, and driver profiles from the cloud PWA using existing lap data -- and receive an automated email when their track record is broken -- all without any login

**Verified:** 2026-03-15T18:00:00Z
**Status:** gaps_found
**Re-verification:** No -- initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | User can open leaderboard, navigate to any track, filter by sim type, and see fastest valid laps sorted by time without login | VERIFIED | `publicApi.trackLeaderboard` with `sim_type` param; `public_track_leaderboard` handler (routes.rs:8224) accepts LeaderboardQuery with sim_type, defaults to assetto_corsa; PWA page has sim_type dropdown; route registered at /public/leaderboard/{track} (no auth) |
| 1a | User can filter track leaderboard by car | PARTIAL | Backend supports `?car=X` (LeaderboardQuery.car field, routes.rs:8240), PWA publicApi.trackLeaderboard accepts car param, but leaderboard page has NO car filter UI dropdown |
| 2 | User can view circuit records (fastest per vehicle per circuit) and vehicle records (fastest per circuit for a vehicle) | VERIFIED | `/public/circuit-records` handler (routes.rs:8312) returns per (track, car, sim_type) records; `/public/vehicle-records/{car}` handler (routes.rs:8367) returns per track best; PWA records page consumes both via publicApi.circuitRecords and publicApi.vehicleRecords; records page has car filter that switches between circuit and vehicle views |
| 3 | User can search for a driver by name, open public profile via shareable URL, see stats, personal bests, and full lap history with sectors | VERIFIED | `/public/drivers?name=X` handler (routes.rs:8406) with LIKE search, LIMIT 20; `/public/drivers/{id}` handler (routes.rs:8435) returns display_name, total_laps, total_time_ms, personal_bests, lap_history with sector times; PWA drivers/page.tsx has debounced search with links; drivers/[id]/page.tsx renders stats cards, personal bests table, lap history with S1/S2/S3; profile excludes PII (no email/phone/wallet in SQL SELECT); class_badge: null placeholder present |
| 4 | When track record is beaten, previous holder receives email with track, car, old time, new time, new holder name, and leaderboard link | VERIFIED | `get_previous_record_holder` (lap_tracker.rs:268) fetches BEFORE UPSERT; notification fires via `tokio::spawn + Command::new("node")` (lap_tracker.rs:203-234); email body includes track, car, old/new time formatted as M:SS.mmm, new holder display name, link to /leaderboard/public; NULL email silently skipped (lap_tracker.rs:235-241); first record = no notification |
| 5 | Invalid laps hidden by default; user can toggle to show invalid laps | VERIFIED | Default clause: `AND l.valid = 1 AND (l.suspect IS NULL OR l.suspect = 0)` (routes.rs:8237); show_invalid=true drops valid=1 but keeps suspect filter (routes.rs:8234-8238); PWA leaderboard page has "Show Invalid" checkbox (page.tsx:130-138); re-fetches on toggle change (useEffect deps at line 95) |

**Score:** 4/5 truths fully verified (1 partial -- car filter UI missing from leaderboard page)

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/rc-core/src/db/mod.rs` | suspect column migration | VERIFIED | ALTER TABLE at line 1968; idempotent with `let _ =` |
| `crates/rc-core/src/lap_tracker.rs` | suspect computation + notification | VERIFIED | 294 lines; suspect logic lines 47-60; notification logic lines 131-243; get_previous_record_holder exported |
| `crates/rc-core/src/api/routes.rs` | all new public endpoints | VERIFIED | Routes registered lines 247-252; 6 handlers: public_leaderboard, public_track_leaderboard, public_circuit_records, public_vehicle_records, public_drivers_search, public_driver_profile |
| `crates/rc-core/tests/integration.rs` | Phase 13 integration tests | VERIFIED | 5 suspect tests (lines 1449-1677), 6 leaderboard/records tests (lines 1681-1927), 3 notification tests (lines 1940-2193), 7 driver search/profile tests (lines 2195-2430) |
| `pwa/src/lib/api.ts` | publicApi methods | VERIFIED | trackLeaderboard (with params), circuitRecords, vehicleRecords, searchDrivers, driverProfile (lines 926-968) |
| `pwa/src/app/leaderboard/public/page.tsx` | leaderboard with sim_type + show_invalid | VERIFIED (partial) | 346 lines; sim_type dropdown, show_invalid toggle, tab switcher, mobile card + desktop table layouts; MISSING: car filter dropdown |
| `pwa/src/app/records/page.tsx` | circuit + vehicle records page | VERIFIED | 242 lines; fetches circuitRecords, groups by track, car filter dropdown pivots to vehicleRecords view; mobile/desktop responsive |
| `pwa/src/app/drivers/page.tsx` | driver search page | VERIFIED | 161 lines; debounced search (300ms), grid results with avatar/initials, Link to /drivers/{id} |
| `pwa/src/app/drivers/[id]/page.tsx` | driver profile page | VERIFIED | 442 lines; stats cards (total_laps, total_time, personal_bests count), personal bests table, lap history with sector times, class_badge conditional rendering, 404 handling, shareable URL |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| lap_tracker.rs | db/mod.rs | suspect column in INSERT | WIRED | `.bind(suspect_flag)` in INSERT at line 81; suspect computed at lines 47-60 |
| lap_tracker.rs | send_email.js | tokio::process::Command::new("node") | WIRED | Fire-and-forget spawn at lines 203-234; uses state.config.watchdog.email_script_path |
| lap_tracker.rs | drivers.email | SELECT before UPSERT | WIRED | get_previous_record_holder (line 133) called BEFORE UPSERT (line 156) |
| routes.rs (public_track_leaderboard) | laps table | SQL with sim_type + suspect filters | WIRED | WHERE l.sim_type = ? AND (l.suspect IS NULL OR l.suspect = 0) at lines 8250-8256 |
| routes.rs (public_circuit_records) | laps table | GROUP BY track, car, sim_type with suspect filter | WIRED | Queries at lines 8317-8347 with correct filters |
| routes.rs (public_vehicle_records) | laps table | WHERE car = ? with suspect filter | WIRED | Query at lines 8371-8381 |
| routes.rs (public_drivers_search) | drivers table | LIKE query for name/nickname | WIRED | Query at lines 8410-8416 with COLLATE NOCASE |
| routes.rs (public_driver_profile) | drivers + personal_bests + laps | 3 queries, no PII | WIRED | Query 1 selects only safe fields (line 8440-8443); Query 2 joins personal_bests; Query 3 joins laps with suspect filter and sector 0->null mapping |
| leaderboard/public/page.tsx | /public/leaderboard/{track} | publicApi.trackLeaderboard with sim_type | WIRED | Calls at lines 82 and 91 with sim_type and show_invalid params |
| records/page.tsx | /public/circuit-records | publicApi.circuitRecords | WIRED | useEffect at line 47-53 with sim_type param |
| records/page.tsx | /public/vehicle-records/{car} | publicApi.vehicleRecords | WIRED | useEffect at line 57-63 with car and sim_type |
| drivers/page.tsx | /public/drivers | publicApi.searchDrivers | WIRED | setTimeout callback at lines 42-53 |
| drivers/[id]/page.tsx | /public/drivers/{id} | publicApi.driverProfile | WIRED | useEffect at lines 81-112 |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|-----------|-------------|--------|----------|
| LB-01 | 13-02 | Track leaderboard filtered by car and sim_type | PARTIAL | sim_type filter works end-to-end; car filter API works but no UI dropdown on leaderboard page |
| LB-02 | 13-02 | Circuit records: fastest per vehicle per circuit | SATISFIED | /public/circuit-records endpoint + records page |
| LB-03 | 13-02 | Vehicle records: fastest per circuit per vehicle | SATISFIED | /public/vehicle-records/{car} endpoint + records page car filter |
| LB-04 | 13-02 | sim_type prevents AC/F1 mixing | SATISFIED | Tested: test_leaderboard_no_cross_sim |
| LB-05 | 13-01 | Suspect flagging (sanity + sector sum) | SATISFIED | 5 tests passing: sector sum, sanity, valid, no sectors, zero sectors |
| LB-06 | 13-02 | Invalid laps hidden by default, toggle to show | SATISFIED | Tested: test_leaderboard_suspect_hidden, test_leaderboard_invalid_toggle |
| DRV-01 | 13-04 | Driver search by name, public profile | SATISFIED | LIKE search COLLATE NOCASE, LIMIT 20, tested |
| DRV-02 | 13-04 | Profile: stats, personal bests, class badge, no PII | SATISFIED | class_badge: null present, no email/phone/wallet in SELECT |
| DRV-03 | 13-04 | Lap history with sector times | SATISFIED | S1/S2/S3 with 0->null mapping, tested |
| DRV-04 | 13-04 | Shareable URL, nickname display | SATISFIED | /drivers/{id} route, nickname logic tested |
| NTF-01 | 13-03 | Email on track record beaten | SATISFIED | tokio::spawn fire-and-forget, tested data ordering |
| NTF-02 | 13-03 | Email includes track, car, old/new time, holder name | SATISFIED | Email body constructed at lines 192-199 of lap_tracker.rs |
| PUB-01 | 13-05 | All pages accessible without login | SATISFIED | All routes under /public/* (no auth middleware); PWA uses publicApi (no JWT) |
| PUB-02 | 13-05 | Mobile-first, responsive, 14px/16px minimums | NEEDS HUMAN | Font sizes set via inline style `fontSize: "14px"` for times, `fontSize: "16px"` for positions in page code; card/table responsive via sm: breakpoint classes |

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| routes.rs | 1430, 1438 | `"todo": "create_event"` / `"todo": "create_booking"` | Info | Pre-existing stubs unrelated to Phase 13 |
| drivers/page.tsx | 92 | `placeholder=` attribute | Info | CSS class name contains "placeholder-rp-grey" -- this is Tailwind styling, not a placeholder implementation |

No blocker or warning anti-patterns found in Phase 13 artifacts.

### Human Verification Required

### 1. Mobile Rendering Check

**Test:** Open app.racingpoint.cloud on a phone (or Chrome DevTools at 375px). Navigate to Leaderboard, Records, Drivers, and a Driver Profile.
**Expected:** All pages render without horizontal overflow. Times display at 14px minimum, positions at 16px. Card layout on mobile, table on desktop.
**Why human:** Visual layout, touch targets, and viewport behavior cannot be verified programmatically.

### 2. Email Notification End-to-End

**Test:** With rc-core running and send_email.js configured, have Driver A set a track record, then Driver B beat it. Check Driver A's email inbox.
**Expected:** Email arrives with subject "Your {car} record at {track} has been beaten!", body contains old time, new time, new holder name, link to /leaderboard/public.
**Why human:** Email delivery depends on send_email.js, Node runtime, and SMTP configuration.

### 3. No-Login Access

**Test:** Open each PWA page in an incognito browser (no stored JWT). Navigate leaderboard, records, drivers, driver profile.
**Expected:** All pages load and display data. No redirect to /login. No auth errors.
**Why human:** Need to verify actual browser behavior with no session state.

### 4. Driver Profile Shareable URL

**Test:** Copy a driver profile URL (e.g., /drivers/{some-id}), paste in new incognito tab.
**Expected:** Profile loads with stats, personal bests, lap history. Same data as navigating via search.
**Why human:** Deep link routing behavior in Next.js needs browser verification.

### Gaps Summary

Two gaps were identified, one user-facing and one backend:

1. **Car filter UI missing from leaderboard page** (partial gap for LB-01 / Success Criterion #1): The backend API fully supports car filtering via `?car=X` on the track leaderboard endpoint, and the PWA publicApi method accepts a car parameter, but the actual leaderboard page (`leaderboard/public/page.tsx`) does not render a car filter dropdown. The user cannot filter by car from the UI. This is a UI-only gap -- adding a dropdown populated from the track's unique cars (already available in track stats) would close it.

2. **Vehicle records endpoint ignores sim_type query param** (minor backend gap): The PWA `publicApi.vehicleRecords` method passes `sim_type` as a query parameter, but the backend `public_vehicle_records` handler only extracts `Path(car)` and does not accept a `Query` extractor. The sim_type parameter is silently ignored by Axum. The vehicle records view on the records page shows all sim types regardless of the sim type dropdown selection. Adding a `VehicleRecordsQuery` struct with `sim_type: Option<String>` and a WHERE clause would fix this.

Neither gap is a blocker for core functionality -- leaderboards, records, profiles, and notifications all work end-to-end. The car filter gap is the most visible since it affects a stated success criterion.

---

_Verified: 2026-03-15T18:00:00Z_
_Verifier: Claude (gsd-verifier)_
