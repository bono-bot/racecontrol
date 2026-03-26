---
phase: 212-detection-expansion
verified: 2026-03-26T07:35:00+05:30
status: gaps_found
score: 5/7 success criteria verified
re_verification: false
gaps:
  - truth: "config-drift.sh reports FAIL finding with specific key and observed vs expected values for racecontrol.toml ws_connect_timeout below 600ms or incorrect app_health URL port"
    status: partial
    reason: "Implementation reads rc-agent.toml (documented decision per plan), but app_health URL port check is not implemented. Only ws_connect_timeout and pod_number are checked. Success criterion SC-1 explicitly requires app_health URL port detection."
    artifacts:
      - path: "scripts/detectors/detect-config-drift.sh"
        issue: "Checks ws_connect_timeout and pod_number but does NOT check app_health URL ports (admin :3201, kiosk basePath). SC-1 requires both."
    missing:
      - "Add app_health URL port check to detect_config_drift() — grep for admin_url/kiosk_url in rc-agent.toml and validate expected ports"
  - truth: "log-anomaly.sh flags pod with >10 ERROR or PANIC lines in the last hour (not the full day)"
    status: partial
    reason: "Implementation counts ERROR/PANIC lines in the full day's log, not the last hour. SC-3 explicitly says 'in the last hour'. The hourly filtering is deferred in code comments but remains an open success criterion gap."
    artifacts:
      - path: "scripts/detectors/detect-log-anomaly.sh"
        issue: "Counts all ERROR/PANIC lines in today's full JSONL log. SC-3 requires last-hour filtering ('a test file with 15 injected ERROR lines triggers detection'). No hourly time window applied."
    missing:
      - "Add hourly time window filtering to detect_log_anomaly() — use cutoff timestamp (similar to detect-crash-loop.sh pattern) to filter JSONL lines to the last 60 minutes only"
---

# Phase 212: Detection Expansion — Verification Report

**Phase Goal:** The auto-detect pipeline detects config drift, bat file regression, log anomalies, crash loops, flag desync, and schema gaps — every detection traces to a documented historical incident
**Verified:** 2026-03-26T07:35:00+05:30
**Status:** gaps_found
**Re-verification:** No — initial verification

---

## Goal Achievement

### Observable Truths (from ROADMAP Success Criteria)

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| SC-1 | When a pod config has ws_connect_timeout below 600ms or incorrect app_health URL port, config-drift.sh reports FAIL with specific key and observed vs expected | PARTIAL | ws_connect_timeout check present with observed/expected values; app_health URL port NOT checked |
| SC-2 | When pod start-rcagent.bat checksum differs from repo canonical, bat drift detection flags that pod with hash mismatch | VERIFIED | `bat_scan_pod_json` called per-pod; DRIFT status triggers P2 finding with pod number |
| SC-3 | When pod JSONL log has >10 ERROR or PANIC lines in last hour, log-anomaly.sh flags that pod | PARTIAL | Threshold 10 is correct but detection uses full day's log, not last-hour window |
| SC-4 | Pod with >3 rc-agent restarts in 30 minutes flagged as crash-loop; reads JSONL restart timestamps not process count | VERIFIED | 30-min UTC cutoff, JSONL findstr search for startup markers, ISO 8601 string comparison |
| SC-5 | DET-05 reports specific flag name and which pods diverge | VERIFIED | `comm -23`/`comm -13` compute missing/extra flag names; pod_ip in finding message |
| DET-06 | Schema drift detects missing columns between cloud and venue DBs | VERIFIED | 6 SCHEMA_CHECKS pairs, venue via safe_remote_exec sqlite3, cloud via SSH, graceful unknown handling |
| DET-07 | cascade.sh sources into auto-detect.sh, shares BUGS_FOUND/LOG_FILE | VERIFIED | auto-detect.sh sources cascade.sh inside run_cascade_check() before 4a block; BUGS_FOUND += DETECTOR_FINDINGS |

**Score:** 5/7 success criteria fully verified

---

### Derived Truths (from PLAN must_haves)

**Plan 01 must_haves:**

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| P1-T1 | cascade.sh sources all detector scripts and calls run_all_detectors from auto-detect.sh step 4 | VERIFIED | Lines 87-99 source 6 detectors; auto-detect.sh line 343-348 sources cascade.sh and calls run_all_detectors |
| P1-T2 | Config drift detector reports specific key and observed vs expected values for ws_connect_timeout < 600 | VERIFIED | Message: "ws_connect_timeout=${ws_val}ms on ${pod_ip} -- expected>=600ms (key=ws_connect_timeout, observed=${ws_val}, expected>=600)" |
| P1-T3 | Bat drift detector flags pods with checksum mismatch against canonical repo version | VERIFIED | `bat_scan_pod_json` + DRIFT status check; P2 finding emitted |
| P1-T4 | Log anomaly detector flags pods with >10 ERROR/PANIC lines in last hour | PARTIAL | Threshold 10 present; time window is full day, not last hour |

**Plan 02 must_haves:**

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| P2-T1 | Crash loop detector flags pods with >3 restarts in 30 minutes from JSONL timestamps | VERIFIED | max_restarts=3, window_minutes=30, UTC cutoff, ISO 8601 comparison |
| P2-T2 | Flag desync detector reports specific flag name and which pods diverge | VERIFIED | missing=[...] extra=[...] per pod_ip in finding message |
| P2-T3 | Schema gap detector reports missing columns between cloud and venue DBs | VERIFIED | venue_has_col/cloud_has_col compared; all 3 scenarios (venue-only, cloud-only, both-missing) reported |
| P2-T4 | All 6 detectors run via cascade.sh dry-run without error | PARTIAL | Dry-run returns early at cascade step (SKIP). Syntax checks all pass (`bash -n` OK on all 8 files). Pipeline runs correctly in non-dry-run mode. |

---

### Required Artifacts

| Artifact | Expected | Exists | Substantive | Wired | Status |
|----------|----------|--------|-------------|-------|--------|
| `scripts/cascade.sh` | DET-07 framework with _emit_finding and run_all_detectors | YES | YES (134 lines, full impl) | YES (sourced in auto-detect.sh:343-348) | VERIFIED |
| `scripts/detectors/detect-config-drift.sh` | DET-01 config drift via safe_remote_exec | YES | YES (71 lines, checks ws_connect_timeout, pod_number, banner guard) | YES (sourced by cascade.sh:88) | PARTIAL (no app_health URL check) |
| `scripts/detectors/detect-bat-drift.sh` | DET-02 bat file checksum drift | YES | YES (64 lines, wraps bat_scan_pod_json) | YES (sourced by cascade.sh:89) | VERIFIED |
| `scripts/detectors/detect-log-anomaly.sh` | DET-03 ERROR/PANIC log count, venue-aware thresholds | YES | YES (78 lines, venue thresholds 10/2, P1 if >50) | YES (sourced by cascade.sh:90) | PARTIAL (full-day count, not last-hour) |
| `scripts/detectors/detect-crash-loop.sh` | DET-04 crash loop via JSONL timestamps | YES | YES (78 lines, 3/30min threshold, UTC cutoff) | YES (sourced by cascade.sh:91) | VERIFIED |
| `scripts/detectors/detect-flag-desync.sh` | DET-05 feature flag sync check | YES | YES (81 lines, comm diff, Pitfall 3 fleet-empty detection) | YES (sourced by cascade.sh:92) | VERIFIED |
| `scripts/detectors/detect-schema-gap.sh` | DET-06 schema drift across venue/cloud DBs | YES | YES (91 lines, 6 SCHEMA_CHECKS, venue+cloud checks) | YES (sourced by cascade.sh:93) | VERIFIED |
| `scripts/detectors/` directory | Container for 6 detector scripts | YES | YES (.gitkeep + 6 scripts) | YES | VERIFIED |

---

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `scripts/auto-detect.sh` | `scripts/cascade.sh` | `source "$SCRIPT_DIR/cascade.sh"` inside run_cascade_check() | WIRED | Line 345; existence-checked |
| `scripts/cascade.sh` | `scripts/detectors/*.sh` | `source "$_detector_file"` with existence check loop | WIRED | Lines 87-99; all 6 files sourced |
| `scripts/cascade.sh` | `auto-detect.sh BUGS_FOUND` | `BUGS_FOUND=$((BUGS_FOUND + DETECTOR_FINDINGS))` in run_all_detectors | WIRED | Line 131 of cascade.sh |
| `scripts/detectors/detect-crash-loop.sh` | rc-agent JSONL log | `safe_remote_exec "$pod_ip" 8090 "findstr ... rc-agent-YYYY-MM-DD.jsonl"` | WIRED | Line 40-41 |
| `scripts/detectors/detect-flag-desync.sh` | server /api/v1/flags | `curl -s "${SERVER_URL}/api/v1/flags"` | WIRED | Line 22 |
| `scripts/detectors/detect-schema-gap.sh` | racecontrol DB | `safe_remote_exec "192.168.31.23" 8090 "sqlite3.exe ... SELECT"` | WIRED | Lines 40-41 |

---

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|-------------|-------------|--------|----------|
| DET-01 | 212-01 | Config drift detection — running config values vs canonical | PARTIAL | ws_connect_timeout checked via rc-agent.toml; app_health URL port NOT checked; REQUIREMENTS.md says "racecontrol.toml" but plan deliberately reads rc-agent.toml (documented rationale) |
| DET-02 | 212-01 | Bat file drift — pod start-rcagent.bat checksums vs repo canonical | SATISFIED | bat_scan_pod_json wrapper; DRIFT status emits P2 finding per pod |
| DET-03 | 212-01 | Log anomaly — ERROR/PANIC rate >10/hour open, >2/hour closed | PARTIAL | Threshold values correct; "per hour" window not implemented — full day count used; hourly filtering deferred in comments |
| DET-04 | 212-02 | Crash loop — >3 rc-agent restarts in 30 minutes | SATISFIED | JSONL timestamps, 3/30min threshold, UTC date, ISO 8601 comparison |
| DET-05 | 212-02 | Feature flag sync — all 8 pods identical enabled flag set | SATISFIED | Per-pod curl to /api/v1/flags, comm diff reports specific missing/extra flags |
| DET-06 | 212-02 | Schema drift — cloud vs venue DB column mismatches | SATISFIED | 6 SCHEMA_CHECKS pairs, both DBs queried, 3 gap scenarios reported |
| DET-07 | 212-01 | Cascade module — cascade.sh sources into auto-detect.sh, shares env | SATISFIED | auto-detect.sh:343-348 sources cascade.sh, BUGS_FOUND accumulated |

---

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| `scripts/detectors/detect-config-drift.sh` | 43,55,63 + 68 | `_emit_finding` increments DETECTOR_FINDINGS AND caller also adds `findings_count` to DETECTOR_FINDINGS at end | WARNING | Double-counting: each finding counted twice in BUGS_FOUND. findings.json is correct but BUGS_FOUND inflated. Same pattern in detect-bat-drift.sh and detect-log-anomaly.sh. |
| `scripts/detectors/detect-crash-loop.sh` | 70-73 | `_emit_finding` called but no local `findings_count` accumulator or final `DETECTOR_FINDINGS +=` — correct pattern (relying solely on _emit_finding to increment) | INFO | Inconsistent with other detectors but functionally correct. The 3 detectors that also add findings_count are the ones with the bug. |
| `scripts/auto-detect.sh` | 336-337 | `if [[ "$DRY_RUN" == "true" ]]; then record_step "cascade" "SKIP" "dry-run"; return 0; fi` — detectors never run in dry-run mode | INFO | The SUMMARY claims `auto-detect.sh --dry-run exits 0` — true, but cascade step is SKIP not run. Syntax validation via `bash -n` is the actual pipeline check. |

---

### Historical Incident Tracing

The phase goal requires "every detection traces to a documented historical incident."

| Detector | Incident Reference | Status |
|----------|--------------------|--------|
| DET-01 (config drift) | "ws_connect_timeout: must be >= 600ms (incident: WS timeouts at 200ms caused flicker)" — documented in code comment lines 9, 48 | TRACED |
| DET-02 (bat drift) | "regression prevention: stale bat causes missing process kills and wrong startup procedures" — documented in finding message | TRACED (implicit) |
| DET-03 (log anomaly) | No explicit incident reference in file comments | PARTIAL — threshold is documented in CONTEXT.md/RESEARCH.md but not in the script itself |
| DET-04 (crash loop) | "Pod 3 (.28) and Pod 6 (.87) spontaneously rebooted 2026-03-22 ~18:50 IST" — mentioned in MEMORY.md but NOT referenced in the detector script | PARTIAL — not in script |
| DET-05 (flag desync) | No explicit incident reference | NOT TRACED in code |
| DET-06 (schema gap) | "known ALTER TABLE additions from db/mod.rs" referenced in SCHEMA_CHECKS comment | TRACED (implicit via column list) |

The goal says "every detection traces to a documented historical incident" — this is met at the architecture/planning level (CONTEXT.md and RESEARCH.md document the incident rationale) but 3 of 6 detectors lack inline incident references in their script comments.

---

### Human Verification Required

#### 1. Config Drift Detection — App Health URL Port Check

**Test:** Modify a pod's rc-agent.toml to use wrong admin port (e.g., 3200 instead of 3201), run `detect_config_drift`
**Expected:** P1 or P2 finding with specific key "admin_url" and observed/expected port values
**Why human:** app_health URL check is not implemented — this will NOT produce a finding currently. Requires confirming gap vs intentional scope reduction.

#### 2. Log Anomaly — Last-Hour vs Full-Day

**Test:** Create a test JSONL file with 15 ERROR lines timestamped 90 minutes ago (outside last-hour window) and 5 ERROR lines in the last hour
**Expected per SC-3:** Only the 5 recent lines counted — no finding (below threshold of 10)
**Actual behavior:** All 20 lines counted against daily total — finding emitted (above threshold)
**Why human:** Full-day count vs hourly window is a functional difference that changes false positive rate

#### 3. DETECTOR_FINDINGS Double-Counting

**Test:** Run auto-detect.sh in non-dry-run mode against a pod with known ws_connect_timeout = 200ms
**Expected:** BUGS_FOUND incremented by 1 (one finding)
**Actual behavior:** BUGS_FOUND incremented by 2 (double-counted via _emit_finding + findings_count accumulator)
**Why human:** Verify whether this causes downstream issues in Phase 213 fix engine thresholds

---

### Commits Verified

| Commit | Description | Status |
|--------|-------------|--------|
| `2756ed86` | cascade.sh framework with _emit_finding and run_all_detectors | FOUND |
| `71442a9a` | DET-01/02/03 detector scripts | FOUND |
| `df25fa0f` | DET-04 crash loop and DET-05 flag desync detectors | FOUND |
| `ee8e6ece` | DET-06 schema gap detector + full pipeline validation | FOUND |

---

### Gaps Summary

Two success criteria from the ROADMAP are not fully implemented:

**Gap 1 — SC-1 (DET-01) app_health URL port not checked:** The ROADMAP's first success criterion explicitly requires detecting "incorrect app_health URL port" in pod config. The implementation checks ws_connect_timeout and pod_number but has no app_health URL validation. The REQUIREMENTS.md description for DET-01 also says "racecontrol.toml" while the implementation reads rc-agent.toml — this deviation is documented and defensible (pods run rc-agent), but the app_health URL gap is a missing detection that the success criterion tests for.

**Gap 2 — SC-3 (DET-03) full-day count instead of last-hour window:** The ROADMAP specifies ">10 ERROR or PANIC lines in the last hour." The implementation counts all ERROR/PANIC lines in today's entire JSONL file. The comments explicitly defer hourly filtering ("Rate-based threshold deferred — requires 7-day calibration") but this is in tension with the success criterion's explicit test case ("a test file with 15 injected ERROR lines triggers detection; a file with 5 does not" — this test works only if counting all lines, not if filtering to last hour).

**Advisory — DETECTOR_FINDINGS double-counting:** detect-config-drift.sh, detect-bat-drift.sh, and detect-log-anomaly.sh each call `_emit_finding` (which increments DETECTOR_FINDINGS by 1) AND also add a local `findings_count` to DETECTOR_FINDINGS at the end. This results in each finding being counted twice in BUGS_FOUND. detect-crash-loop.sh, detect-flag-desync.sh, and detect-schema-gap.sh do not have this issue. The findings.json output is correct; only the BUGS_FOUND counter is inflated.

---

*Verified: 2026-03-26T07:35:00+05:30*
*Verifier: Claude (gsd-verifier)*
