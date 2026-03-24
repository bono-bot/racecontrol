---
phase: 59-auto-switch-configuration
verified: 2026-03-24T19:15:00+05:30
status: passed
score: 5/5 must-haves verified
re_verification:
  previous_status: gaps_found
  previous_score: 3/4
  gaps_closed:
    - "VENUE_GAME_KEYS now has 4 confirmed entries — ASSETTO_CORSA_EVO added from Pod 8 hardware inspection (commit 5ae28d8b). No TBD comment remains in the constant block. PROF-02 fully satisfied."
    - "Human physically verified ConspitLink auto-switch on Pod 8 hardware (Plans 59-03 + 59-04). AC and F1 25 presets auto-loaded and switched correctly. PROF-04 satisfied by human attestation."
  gaps_remaining: []
  regressions: []
---

# Phase 59: Auto-Switch Configuration Verification Report

**Phase Goal:** ConspitLink automatically detects which game is running and loads the correct FFB preset without staff action
**Verified:** 2026-03-24T19:15:00 IST
**Status:** passed
**Re-verification:** Yes — after gap closure by Plans 59-03 and 59-04

---

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | Global.json placed at C:\RacingPoint\ with AresAutoChangeConfig=open | VERIFIED | `place_global_json()` at line 736 — serde_json parse + force + atomic rename. Pod 8 confirmed: `build_id 5ae28d8b` + type output showed `"AresAutoChangeConfig": "open"`. |
| 2 | GameToBaseConfig.json has entries for all 4 venue games pointing to existing .Base files | VERIFIED | `VENUE_GAME_KEYS` now has 4 confirmed entries: `"Assetto Corsa"`, `"F1 25"`, `"Assetto Corsa Competizione"`, `"ASSETTO_CORSA_EVO"`. Confirmed from Pod 8 hardware inspection (commit 5ae28d8b). No TBD comment in constant block. |
| 3 | ConspitLink restarted only when config actually changed | VERIFIED | `if result.global_json_changed || result.game_to_base_fixed` gate at line 715. Production-only via `if install_dir.is_none()` guard. Unit tests confirm no-restart path passes. |
| 4 | Config placement runs before enforce_safe_state in startup | VERIFIED | `spawn_blocking` at main.rs line 560 is inserted BEFORE the delayed 8s block at line 579–589. Comment confirms intent: "Runs BEFORE enforce_safe_state". |
| 5 | Launching a venue game causes ConspitLink to auto-load the matching preset (PROF-04) | VERIFIED | Human physically walked to Pod 8 (192.168.31.91) and confirmed: AC launched → AC preset loaded within ~5s; F1 25 launched → ConspitLink switched to F1 25 preset (distinct from AC). Human attestation accepted per Plan 59-04. |

**Score:** 5/5 truths verified

---

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/rc-agent/src/ffb_controller.rs` | `ensure_auto_switch_config()` + `_impl()` + `place_global_json()` + `verify_game_to_base_config()` + VENUE_GAME_KEYS with 4 entries + unit tests | VERIFIED | All 4 functions present. VENUE_GAME_KEYS has 4 confirmed entries (lines 592–597). Commit 5ae28d8b added ASSETTO_CORSA_EVO. 37 tests pass (rc-agent-crate ffb_controller module). Atomic write, serde_json parse, compare-before-write implemented. |
| `crates/rc-agent/src/main.rs` | `spawn_blocking` calling `ensure_auto_switch_config()` before delayed startup block | VERIFIED | Line 560–577: spawn_blocking wraps the call. Correct position before enforce_safe_state. |

---

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `main.rs` | `ffb_controller.rs` | `ffb_controller::ensure_auto_switch_config()` | WIRED | Line 561: `let result = ffb_controller::ensure_auto_switch_config();` inside spawn_blocking |
| `ffb_controller.rs` | `restart_conspit_link_hardened` | Conditional call after config change | WIRED | Line 723: `restart_conspit_link_hardened(false)` gated on `install_dir.is_none()` — production-only |
| `ensure_auto_switch_config_impl` | `place_global_json` | Direct function call | WIRED | Line 654: `match place_global_json(&source_global, &target_global)` |
| `ensure_auto_switch_config_impl` | `verify_game_to_base_config` | Conditional call (file must exist) | WIRED | Line 681: `match verify_game_to_base_config(&gtb_path, &install_base)` guarded by `gtb_path.exists()` |
| `VENUE_GAME_KEYS` constant | `verify_game_to_base_config()` | Iteration over all 4 keys | WIRED | Line 799: `for &key in VENUE_GAME_KEYS` iterates all 4 entries including ASSETTO_CORSA_EVO |

---

### Requirements Coverage

| Requirement | Source Plans | Description | Status | Evidence |
|-------------|-------------|-------------|--------|----------|
| PROF-01 | 59-01, 59-04 | Global.json exists at C:\RacingPoint\ (runtime read path) | SATISFIED | `place_global_json()` places file with atomic write. Pod 8 verified at build_id `5ae28d8b`. 4 unit tests cover this path. |
| PROF-02 | 59-01, 59-03 | GameToBaseConfig.json mappings for all 4 active venue games | SATISFIED | `VENUE_GAME_KEYS` has 4 confirmed entries. 4th key `ASSETTO_CORSA_EVO` added in commit 5ae28d8b after Pod 8 hardware inspection. No TBD comment remains in the constant block. Startup log `phase=self_heal no_repairs_needed` confirms all 4 keys found on Pod 8. |
| PROF-04 | 59-01, 59-02, 59-04 | Launching a game causes ConspitLink to auto-load matching preset | SATISFIED | Human physically verified on Pod 8 (Plan 59-04). AC and F1 25 auto-switch confirmed. Previous auto-approval (Plan 59-02) was superseded by physical verification. |

**PROF-03 and PROF-05** belong to Phase 60 (Pre-Launch Profile Loading) — not in scope for Phase 59.

---

### Anti-Patterns Found

No blocking or warning-level anti-patterns found.

| File | Line | Pattern | Severity | Notes |
|------|------|---------|----------|-------|
| `ffb_controller.rs` | 260 | "Value range TBD empirically" | Info | In FFB force-ramp constant — unrelated to VENUE_GAME_KEYS, not a Phase 59 gap. |
| `ffb_controller.rs` | 782, 808 | "path fix deferred to Phase 61" | Info | Doc-comment and warning string for `.Base` file path validation — intentional scoped deferral. Phase 61's responsibility, not a Phase 59 gap. VENUE_GAME_KEYS block itself is clean. |

---

### Re-Verification Summary

**Gap 1 — CLOSED:** VENUE_GAME_KEYS 4th entry (PROF-02)

Plan 59-03 inspected `GameToBaseConfig.json` on Pod 8 hardware via Tailscale SSH. Confirmed exact key string: `ASSETTO_CORSA_EVO` (uppercase-underscore style used by ConspitLink 2.0 for newer games). `ASSETTO_CORSA_RALLY` was found in the file but is not an active venue game — correctly excluded. Commit `5ae28d8b` added the 4th key, removed the TBD comment, rebuilt and deployed rc-agent to Pod 8 (build_id confirmed). Startup log `phase=self_heal no_repairs_needed` provides positive runtime confirmation that all 4 keys were found in the actual GameToBaseConfig.json on the pod.

**Gap 2 — CLOSED:** PROF-04 hardware verification

Plan 59-04 required a human to physically walk to Pod 8. Human completed the checkpoint and confirmed: (a) launching Assetto Corsa → ConspitLink auto-loaded AC preset within ~5 seconds without staff action; (b) launching F1 25 → ConspitLink switched to a distinct F1 25 preset. Both tests passed. The previous auto-approval in Plan 59-02 (AUTO MODE without physical observation) is superseded by this human attestation.

**No regressions** — the VENUE_GAME_KEYS change is additive (3 → 4 entries). Existing 3 entries unchanged. All 37 ffb_controller unit tests pass.

---

_Verified: 2026-03-24T19:15:00 IST_
_Verifier: Claude (gsd-verifier)_
_Re-verification after gap closure by Plans 59-03 and 59-04_
