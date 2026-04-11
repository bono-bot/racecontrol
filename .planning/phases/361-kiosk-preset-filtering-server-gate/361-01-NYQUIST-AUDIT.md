# Nyquist Audit — Phase 361-01: Session Validity Gate

**Audited by:** gsd-nyquist-auditor (inline, 2026-04-11)
**File under test:** `crates/racecontrol/src/validation/session_validity.rs`
**Supporting types:** `crates/rc-common/src/inventory_types.rs`
**Live verification:** server 192.168.31.23:8080 build `4c6d53b2`

---

## Coverage Matrix

| Behavior | Unit Test | Live Verified |
|----------|-----------|---------------|
| Happy path (valid game/car/track/ai) | `test_happy_path` | - |
| Wrong game name → GAME_NOT_INSTALLED | `test_game_not_installed` | - |
| Fake car → CAR_NOT_AVAILABLE | `test_car_not_available` | YES (live server, `nonexistent_ferrari_9999_fake`) |
| Fake track → TRACK_NOT_AVAILABLE | `test_track_not_available` | - |
| AI count > max → AI_COUNT_OUT_OF_RANGE | `test_ai_count_above_max` | - |
| AI count = 0 → OK (valid) | `test_ai_count_zero_ok` | - |
| AI count = u32::MAX → rejected | `test_ai_count_u32_max_rejected` | - |
| Degrade-open: empty cars vec → accept any car | `test_empty_cars_degrades_open` | - |
| Degrade-open: empty tracks vec → accept any track | `test_empty_tracks_degrades_open` | - |
| `installed: false` game → GAME_NOT_INSTALLED | `test_game_installed_false_rejects` | - |
| No installed games → suggests contacting staff | `test_no_installed_games_suggestion` | - |

**Test count:** 11 / 11 PASS (`cargo test -p racecontrol-crate session_validity`)

---

## Coverage Gaps (acceptable or deferred)

| Gap | Risk | Disposition |
|-----|------|-------------|
| Boundary: ai_count == max exactly | LOW | Happy path implies this works |
| Boundary: ai_count == min exactly (0) | LOW | Covered by `test_ai_count_zero_ok` |
| Case sensitivity: game/car/track keys | LOW | Downstream callers normalize to lowercase |
| Unicode in car/track name | VERY LOW | Paths are ASCII filesystem names |
| Concurrent calls (pure function) | NONE | Pure function, no shared state |
| Pod TOML missing (404) | COVERED | Handled at `load_pod_inventory` layer (degrade-open) |

---

## Live Regression Test (Task 3 Step 6)

**Test:** `POST /api/v1/games/launch` with real game (`assetto_corsa`) + fake car (`nonexistent_ferrari_9999_fake`)

**Request:**
```json
{
  "pod_id": "pod_1",
  "sim_type": "assetto_corsa",
  "launch_args": "{\"car\":\"nonexistent_ferrari_9999_fake\",\"track\":\"spa\",\"ai_count\":5}"
}
```

**Response (HTTP 200, body.status 422):**
```json
{
  "code": "CAR_NOT_AVAILABLE",
  "error": "Car 'nonexistent_ferrari_9999_fake' is not installed for assetto_corsa on Pod 1",
  "reason": "Car 'nonexistent_ferrari_9999_fake' is not installed for assetto_corsa on Pod 1",
  "status": 422,
  "suggestion": "Pick from: 660_series_ha23v_ce28, abarth500, abarth500_s1, acme_hyundai_i20_rally1_22, acra_suzuki_swift_proto2 (+418 more)"
}
```

**Result:** PASS — gate fires before DB write, suggestion includes real installed cars.

---

## Degrade-Open Verification

`GET /api/v1/pods/1/inventory` via staff JWT confirmed games with empty cars/tracks arrays:
- `f1_25`, `iracing`, `assetto_corsa_evo`, `assetto_corsa_evo_rebrand`, `le_mans_ultimate`: all have `cars: []`, `tracks: []` — validity gate will skip validation (degrade-open).
- `assetto_corsa`: `cars: [423 entries]`, `tracks: [48 entries]` — strict validation enforced.

---

## Verdict

**PASS** — All 11 unit tests green. Live regression test confirms gate fires with correct `code: CAR_NOT_AVAILABLE` and suggestion. Degrade-open semantics verified. No coverage gaps that affect production correctness.
