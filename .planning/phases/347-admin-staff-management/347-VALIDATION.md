---
phase: 347
slug: admin-staff-management
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-04-10
---

# Phase 347 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust `#[cfg(test)]` + `#[tokio::test]` (racecontrol-crate) |
| **Config file** | Workspace Cargo.toml (`[profile.test]`) |
| **Quick run command** | `cargo test -p racecontrol-crate change_pin -x` |
| **Full suite command** | `cargo test -p racecontrol-crate && cargo test -p rc-agent-crate && cargo test -p rc-common` |
| **Estimated runtime** | ~45 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p racecontrol-crate change_pin -x`
- **After every plan wave:** Run `cargo test -p racecontrol-crate && cargo test -p rc-agent-crate`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 45 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 347-01-01 | 01 | 1 | STAFF-05 | unit | `cargo test -p racecontrol-crate change_staff_pin_safe_rejects_short_pin` | Wave 0 | pending |
| 347-01-02 | 01 | 1 | STAFF-05 | unit | `cargo test -p racecontrol-crate change_staff_pin_safe_rejects_non_numeric` | Wave 0 | pending |
| 347-01-03 | 01 | 1 | STAFF-06 | unit | `cargo test -p racecontrol-crate change_staff_pin_safe_response_shape` | Wave 0 | pending |
| 347-01-04 | 01 | 1 | STAFF-07 | unit | `cargo test -p racecontrol-crate sync_pull_now_tables_filtered` | Wave 0 | pending |
| 347-03-01 | 03 | 2 | DEP-04 | smoke | `bash scripts/deploy/phase347-preflight.sh` | Wave 0 | pending |

*Status: pending / green / red / flaky*

---

## Wave 0 Requirements

- [ ] `crates/racecontrol/src/api/routes.rs` — unit tests for `change_staff_pin_safe` (validation, response shape)
- [ ] `crates/racecontrol/src/api/routes.rs` — unit test for `sync_pull_now_handler` (table filter)
- [ ] `scripts/deploy/phase347-preflight.sh` — pre-deploy gate script (DEP-04)

*Existing racecontrol test infrastructure covers framework and fixtures.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Modal validates 4+ numeric, matching inputs | STAFF-01, STAFF-04 | Frontend validation — Playwright deferred to Phase 350 | Open `/admin/staff`, click Change PIN, enter invalid PIN, verify error |
| Staged progress UI shows 4 steps | STAFF-08 | Visual UI behavior | Open Change PIN modal, submit, observe staged progress checkmarks |
| No plaintext PINs visible in list | STAFF-03 | Visual inspection | Open `/admin/staff`, verify no PIN column or values shown |
| Old PIN stops working after change | SC-04 (from roadmap) | Requires live venue + kiosk | Change PIN, then try old PIN on kiosk |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 45s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
