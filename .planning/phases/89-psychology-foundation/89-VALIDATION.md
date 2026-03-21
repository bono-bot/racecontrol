---
phase: 89
slug: 89-psychology-foundation
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-21
---

# Phase 1 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in test framework (cargo test) |
| **Config file** | Cargo.toml (workspace) |
| **Quick run command** | `cargo test -p racecontrol-crate --lib` |
| **Full suite command** | `cargo test -p racecontrol-crate` |
| **Estimated runtime** | ~15 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p racecontrol-crate --lib`
- **After every plan wave:** Run `cargo test -p racecontrol-crate`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 15 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 01-01-01 | 01 | 1 | FOUND-05 | integration | `cargo test -p racecontrol-crate --lib psychology` | ❌ W0 | ⬜ pending |
| 01-01-02 | 01 | 1 | FOUND-03 | unit | `cargo test -p racecontrol-crate --lib psychology::tests::test_json_criteria_parsing` | ❌ W0 | ⬜ pending |
| 01-02-01 | 02 | 1 | FOUND-02 | unit | `cargo test -p racecontrol-crate --lib psychology::tests::test_badge_evaluation` | ❌ W0 | ⬜ pending |
| 01-02-02 | 02 | 1 | FOUND-02 | unit | `cargo test -p racecontrol-crate --lib psychology::tests::test_streak_update` | ❌ W0 | ⬜ pending |
| 01-02-03 | 02 | 1 | FOUND-01 | unit | `cargo test -p racecontrol-crate --lib psychology::tests::test_whatsapp_budget_enforced` | ❌ W0 | ⬜ pending |
| 01-02-04 | 02 | 1 | FOUND-04 | unit | `cargo test -p racecontrol-crate --lib psychology::tests::test_channel_routing` | ❌ W0 | ⬜ pending |
| 01-02-05 | 02 | 1 | FOUND-04 | unit | `cargo test -p racecontrol-crate --lib psychology::tests::test_priority_ordering` | ❌ W0 | ⬜ pending |
| 01-02-06 | 02 | 1 | FOUND-01 | unit | `cargo test -p racecontrol-crate --lib psychology::tests::test_reactive_bypasses_budget` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `psychology::tests` module — unit tests for badge evaluation, streak logic, budget enforcement, channel routing
- [ ] Integration test for all 6+ new tables created and seed data insertable

*Wave 0 tests are created as part of Plan 01 (schema + module skeleton) and Plan 02 (logic + dispatch).*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| WhatsApp message actually delivered | FOUND-04 | Requires live Evolution API connection | Send test nudge via `/api/psychology/test-nudge`, verify receipt on WhatsApp |
| Discord webhook message posted | FOUND-04 | Requires live Discord webhook URL | Queue Discord notification, verify message appears in Discord channel |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 15s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
