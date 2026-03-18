---
phase: 41
slug: test-foundation
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-19
---

# Phase 41 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | bash (shell scripts) + @playwright/test 1.58.2 + cargo-nextest |
| **Config file** | `tests/e2e/playwright.config.ts` + `.config/nextest.toml` |
| **Quick run command** | `bash tests/e2e/smoke.sh` |
| **Full suite command** | `cargo nextest run --workspace && npx playwright test --list` |
| **Estimated runtime** | ~30 seconds |

---

## Sampling Rate

- **After every task commit:** Run `bash tests/e2e/smoke.sh`
- **After every plan wave:** Run `cargo nextest run --workspace`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 41-01-01 | 01 | 1 | FOUND-01 | integration | `source tests/e2e/lib/common.sh && pass "test"` | ❌ W0 | ⬜ pending |
| 41-01-02 | 01 | 1 | FOUND-02 | integration | `source tests/e2e/lib/pod-map.sh && echo $POD1_IP` | ❌ W0 | ⬜ pending |
| 41-01-03 | 01 | 1 | FOUND-01 | integration | `bash tests/e2e/smoke.sh` (refactored to use lib/) | ✅ | ⬜ pending |
| 41-02-01 | 02 | 1 | FOUND-03 | integration | `npx playwright test --list` | ❌ W0 | ⬜ pending |
| 41-02-02 | 02 | 1 | FOUND-05 | integration | `cargo nextest run -p racecontrol-crate` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `tests/e2e/lib/common.sh` — pass/fail/skip/info helpers + summary_exit
- [ ] `tests/e2e/lib/pod-map.sh` — pod_ip() function with all 8 pod IPs
- [ ] `npm install -D @playwright/test@1.58.2` — Playwright devDependency
- [ ] `npx playwright install chromium` — bundled browser
- [ ] `cargo install cargo-nextest` — Rust test runner
- [ ] `.config/nextest.toml` — retries config

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Playwright Chromium launches on Windows | FOUND-03 | GUI browser needs display | Run `npx playwright test` on James's machine, verify Chromium opens |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
