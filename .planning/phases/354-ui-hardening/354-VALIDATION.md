---
phase: 354
slug: ui-hardening
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-04-11
---

# Phase 354 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Playwright (crawl-all-pages.spec.ts) + grep-based verification |
| **Config file** | apps/racingpoint-admin/playwright.config.ts |
| **Quick run command** | `grep -rn "alert(" apps/racingpoint-admin/app/ --include="*.tsx"` |
| **Full suite command** | `cd apps/racingpoint-admin && npx playwright test crawl-all-pages` |
| **Estimated runtime** | ~30 seconds (grep) / ~120 seconds (Playwright) |

---

## Sampling Rate

- **After every task commit:** Run `grep -rn "alert(" apps/racingpoint-admin/app/ --include="*.tsx" | wc -l` (should decrease)
- **After every plan wave:** Run full Playwright crawl
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 354-01-01 | 01 | 1 | UI-01 | grep | `grep -n "loading" apps/racingpoint-admin/app/(dashboard)/analytics/page.tsx` | ✅ | ⬜ pending |
| 354-01-02 | 01 | 1 | UI-03 | grep | `grep -rn "alert(" apps/racingpoint-admin/app/ --include="*.tsx" \| wc -l` | ✅ | ⬜ pending |
| 354-01-03 | 01 | 1 | UI-07 | playwright | `npx playwright test crawl-all-pages` | ✅ | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

*Existing infrastructure covers all phase requirements.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Loading skeleton renders visually during slow fetch | UI-02 | Visual rendering needs screenshot | Throttle network in DevTools, verify skeleton appears |
| Toast notifications visible on mutation | UI-04 | Visual toast display | Trigger a mutation (edit pricing), verify toast appears |
| Health page tiles animate/update live | UI-05 | Visual real-time update | Open /settings/health, wait 10s, verify tiles refresh |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
