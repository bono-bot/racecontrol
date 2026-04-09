---
phase: 365
slug: ai-behavior-validation-via-mma
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-04-10
---

# Phase 365 -- Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in (`cargo test`) |
| **Config file** | `Cargo.toml` (workspace) |
| **Quick run command** | `cargo test -p racecontrol-crate --lib 2>&1 \| tail -5` |
| **Full suite command** | `cargo test -p racecontrol-crate -p rc-common -p rc-agent-crate 2>&1 \| tail -10` |
| **Estimated runtime** | ~30-60 seconds |

---

## Sampling Rate

- **After every task commit:** Run quick run command
- **After every plan wave:** Run full suite command
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 60 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | Status |
|---------|------|------|-------------|-----------|-------------------|--------|
| 365-01-01 | 01 | 1 | GLD-E-01 | unit | `cargo test -p racecontrol-crate ai_behavior_samples` | pending |
| 365-01-02 | 01 | 1 | GLD-E-01 | unit | `cargo test -p racecontrol-crate tier_for_level` | pending |
| 365-01-03 | 01 | 1 | GLD-E-01 | integration | `cargo test -p racecontrol-crate ai_behavior_collector` | pending |
| 365-02-01 | 02 | 2 | GLD-E-02 | unit | `cargo test -p racecontrol-crate mma_consensus` | pending |
| 365-02-02 | 02 | 2 | GLD-E-02 | unit | `cargo test -p racecontrol-crate kb_toml` | pending |
| 365-02-03 | 02 | 2 | GLD-E-03 | integration | `cargo test -p racecontrol-crate kb_file_write` | pending |
| 365-03-01 | 03 | 2 | GLD-E-04 | unit | `cargo test -p rc-common ai_behavior_anomaly_roundtrip` | pending |
| 365-03-02 | 03 | 2 | GLD-E-04 | unit | `cargo test -p racecontrol-crate anomaly_check_outside_band` | pending |

*Status: pending / green / red / flaky*

---

## Wave 0 Requirements

Existing infrastructure covers Rust test framework. No new framework install needed.

- [ ] New test module `#[cfg(test)]` in `ai_behavior_batch.rs` with:
  - `test_ai_car_detection` -- `driver_guid.is_empty()` identifies AI cars in AcResultEntry
  - `test_tier_for_level_all_tiers` -- all 5 DifficultyTier values map correctly
  - `test_mma_consensus_3_of_5` -- 3 matching responses = consensus = true
  - `test_mma_consensus_2_of_5` -- 2 matching = no consensus = false
  - `test_anomaly_too_slow` -- median 20% above p90 -> direction "too_slow"
  - `test_anomaly_too_fast` -- median 20% below p10 -> direction "too_fast"
  - `test_no_kb_no_anomaly` -- missing KB entry = no event fired
- [ ] `AiBehaviorAnomaly` variant in `DashboardEvent`: roundtrip serialize/deserialize test in protocol.rs

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| OpenRouter API call returns 200 with valid JSON | GLD-E-02 | Requires live API key + network | Run batch manually: `POST /api/v1/admin/ai-behavior-batch/run` with admin token, check logs for "MMA batch complete" |
| KB TOML file appears in .planning/kb/ai-behavior/ | GLD-E-03 | Requires real session data | After 10+ AI session samples exist, trigger batch and verify file at `.planning/kb/ai-behavior/{car}-{track}.toml` |
| AiBehaviorAnomaly event appears in admin dashboard | GLD-E-04 | Requires browser + WS | Open admin dashboard, start AC session with AI, end session, verify anomaly card appears (if KB has data) |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 60s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
