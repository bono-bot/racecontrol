---
phase: 42
slug: kiosk-source-prep-browser-smoke
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-19
---

# Phase 42 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | @playwright/test 1.58.2 (from Phase 41) |
| **Config file** | `playwright.config.ts` |
| **Quick run command** | `npx playwright test tests/e2e/playwright/smoke.spec.ts` |
| **Full suite command** | `npx playwright test` |
| **Estimated runtime** | ~15 seconds |

---

## Sampling Rate

- **After every task commit:** Run `npx playwright test --list` (verify specs discovered)
- **After every plan wave:** Run `npx playwright test tests/e2e/playwright/smoke.spec.ts`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 15 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 42-01-01 | 01 | 1 | FOUND-06 | grep | `grep -r "data-testid" kiosk/src/components/SetupWizard.tsx` | ❌ W0 | ⬜ pending |
| 42-01-02 | 01 | 1 | FOUND-06 | grep | `grep -r "data-testid" kiosk/src/app/book/page.tsx` | ❌ W0 | ⬜ pending |
| 42-02-01 | 02 | 2 | FOUND-04 | playwright | `npx playwright test cleanup.spec.ts` | ❌ W0 | ⬜ pending |
| 42-02-02 | 02 | 2 | BROW-01 | playwright | `npx playwright test smoke.spec.ts` | ❌ W0 | ⬜ pending |
| 42-02-03 | 02 | 2 | BROW-07 | playwright | Check `test-results/` for screenshots on failure | ❌ W0 | ⬜ pending |
| 42-02-04 | 02 | 2 | FOUND-07 | playwright | `npx playwright test keyboard.spec.ts` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `tests/e2e/playwright/fixtures/cleanup.ts` — pre-test cleanup fixture
- [ ] `tests/e2e/playwright/smoke.spec.ts` — page smoke spec
- [ ] `tests/e2e/playwright/keyboard.spec.ts` — keyboard navigation spec
- [ ] data-testid attributes in SetupWizard.tsx + book/page.tsx

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Screenshots contain useful debug info | BROW-07 | Visual inspection needed | Intentionally fail a test, check test-results/ for PNG + HTML |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 15s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
