---
phase: 149
slug: menu-data-model-crud
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-22
---

# Phase 149 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust) + vitest/jest (Next.js) |
| **Config file** | Cargo.toml (workspace) + web/vitest.config.ts |
| **Quick run command** | `cargo test -p racecontrol -- cafe` |
| **Full suite command** | `cargo test -p racecontrol && cd web && npm test` |
| **Estimated runtime** | ~15 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p racecontrol -- cafe`
- **After every plan wave:** Run `cargo test -p racecontrol && cd web && npm test`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 15 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 149-01-01 | 01 | 1 | MENU-02 | unit | `cargo test -p racecontrol -- cafe::tests` | ❌ W0 | ⬜ pending |
| 149-01-02 | 01 | 1 | MENU-03 | unit | `cargo test -p racecontrol -- cafe::tests` | ❌ W0 | ⬜ pending |
| 149-01-03 | 01 | 1 | MENU-04 | unit | `cargo test -p racecontrol -- cafe::tests` | ❌ W0 | ⬜ pending |
| 149-01-04 | 01 | 1 | MENU-05 | unit | `cargo test -p racecontrol -- cafe::tests` | ❌ W0 | ⬜ pending |
| 149-02-01 | 02 | 2 | MENU-02 | integration | `curl POST /api/v1/cafe/items` | ❌ W0 | ⬜ pending |
| 149-03-01 | 03 | 2 | MENU-02 | e2e | browser admin UI test | ❌ manual | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `crates/racecontrol/src/cafe.rs` — cafe module with #[cfg(test)] mod tests
- [ ] Add cafe tables to `run_test_migrations()` in integration.rs
- [ ] Test helpers: `create_test_cafe_item()`, `create_test_category()`

*Existing test infrastructure (in-memory SQLite, create_test_db) covers framework needs.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Admin UI side panel renders correctly | MENU-02 | Visual UI | Open /cafe in browser, click Add Item, verify form fields |
| Category dropdown + inline add works | MENU-05 | Visual UI + interaction | Select category, type new one, verify it appears |
| Unavailable items hidden from customer views | MENU-05 | Cross-system check | Toggle item unavailable, check POS/PWA don't show it |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 15s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
