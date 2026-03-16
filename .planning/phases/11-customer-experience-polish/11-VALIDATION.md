---
phase: 11
slug: customer-experience-polish
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-14
---

# Phase 11 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in (`cargo test`) |
| **Config file** | Cargo.toml per crate |
| **Quick run command** | `export PATH="$PATH:/c/Users/bono/.cargo/bin" && cargo test -p rc-common` |
| **Full suite command** | `export PATH="$PATH:/c/Users/bono/.cargo/bin" && cargo test -p rc-common && cargo test --bin rc-agent && cargo test -p racecontrol-crate` |
| **Estimated runtime** | ~15 seconds |

---

## Sampling Rate

- **After every task commit:** Run `export PATH="$PATH:/c/Users/bono/.cargo/bin" && cargo test -p rc-common`
- **After every plan wave:** Run `export PATH="$PATH:/c/Users/bono/.cargo/bin" && cargo test -p rc-common && cargo test --bin rc-agent`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 15 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 11-xx-01 | TBD | 1 | BRAND-01 | unit | `cargo test --bin rc-agent logo` | ❌ W0 | ⬜ pending |
| 11-xx-02 | TBD | 1 | BRAND-02 | unit | `cargo test --bin rc-agent wallpaper` | ❌ W0 | ⬜ pending |
| 11-xx-03 | TBD | 1 | BRAND-03 | unit | `cargo test --bin rc-agent launch_splash` | ❌ W0 | ⬜ pending |
| 11-xx-04 | TBD | 2 | SESS-01 | unit | `cargo test --bin rc-agent session_summary_top_speed` | ❌ W0 | ⬜ pending |
| 11-xx-05 | TBD | 2 | SESS-02 | unit | `cargo test --bin rc-agent session_summary_position` | ❌ W0 | ⬜ pending |
| 11-xx-06 | TBD | 2 | SESS-03 | unit | `cargo test --bin rc-agent session_summary_no_autoblank` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `crates/rc-agent/src/lock_screen.rs` — add test `logo_in_page_shell` (BRAND-01)
- [ ] `crates/rc-agent/src/lock_screen.rs` — add test `logo_in_launch_splash` (BRAND-03)
- [ ] `crates/rc-agent/src/lock_screen.rs` — add test `wallpaper_url_renders_in_css` (BRAND-02)
- [ ] `crates/rc-agent/src/lock_screen.rs` — add test `wallpaper_empty_uses_default_bg` (BRAND-02)
- [ ] `crates/rc-agent/src/lock_screen.rs` — add test `session_summary_shows_top_speed` (SESS-01)
- [ ] `crates/rc-agent/src/lock_screen.rs` — add test `session_summary_hides_top_speed_when_zero` (SESS-01)
- [ ] `crates/rc-agent/src/lock_screen.rs` — add test `session_summary_shows_race_position` (SESS-02)
- [ ] `crates/rc-agent/src/lock_screen.rs` — add test `session_summary_hides_position_when_none` (SESS-02)
- [ ] `crates/rc-agent/src/lock_screen.rs` — add test `session_summary_no_auto_reload_script` (SESS-03)
- [ ] No framework install needed — `cargo test` already works

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Logo renders visually correct on pod screen | BRAND-01 | Visual appearance in Edge kiosk mode | Take screenshot via pod-agent, verify logo visible |
| Wallpaper appears on pod after settings change | BRAND-02 | End-to-end kiosk→core→agent→pod chain | Set wallpaper URL in kiosk settings, verify pod shows it within 10s |
| Loading splash shows before game launch | BRAND-03 | Requires game launch trigger | Start billing session, verify splash appears before AC loads |
| Session results stay on screen indefinitely | SESS-03 | Requires billing end + time observation | End session, wait >60s, verify results still displayed |
| Top speed shows correct value after session | SESS-01 | Requires real telemetry from AC | Drive session, check speed matches max observed |
| Race position shows for race sessions | SESS-02 | Requires AC race mode with AI cars | Run race, finish, verify position is displayed |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 15s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
