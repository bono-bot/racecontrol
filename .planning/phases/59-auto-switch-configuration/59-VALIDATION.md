---
phase: 59
slug: auto-switch-configuration
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-24
---

# Phase 59 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (rc-agent-crate) |
| **Config file** | crates/rc-agent/Cargo.toml |
| **Quick run command** | `cargo test -p rc-agent-crate -- --test-threads=1 auto_switch` |
| **Full suite command** | `cargo test -p rc-agent-crate -- --test-threads=1` |
| **Estimated runtime** | ~20 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p rc-agent-crate -- --test-threads=1 auto_switch`
- **After every plan wave:** Run `cargo test -p rc-agent-crate -- --test-threads=1`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 20 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 59-01-01 | 01 | 1 | PROF-01 | unit | `cargo test -p rc-agent-crate -- ensure_global_json` | ❌ W0 | ⬜ pending |
| 59-01-02 | 01 | 1 | PROF-02 | unit | `cargo test -p rc-agent-crate -- game_to_base_config` | ❌ W0 | ⬜ pending |
| 59-02-01 | 02 | 2 | PROF-04 | manual | Pod 8 canary test | N/A | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] Unit tests for `ensure_global_json_at_runtime_path()` — stubs for PROF-01
- [ ] Unit tests for `ensure_game_to_base_config()` — stubs for PROF-02

*Existing test infrastructure (cargo test) covers framework requirements.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Auto game detection on pod | PROF-04 | Requires ConspitLink + game process on real hardware | Launch AC on Pod 8, verify CL auto-loads AC preset |
| Global.json at C:\RacingPoint\ | PROF-01 | File path only exists on Windows pods | Check file exists on Pod 8 after deploy |
| GameToBaseConfig.json mappings | PROF-02 | Requires ConspitLink to read and apply | Launch each game, verify preset switch |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 20s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
