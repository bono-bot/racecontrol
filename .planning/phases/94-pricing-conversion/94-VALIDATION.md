---
phase: 94
slug: pricing-conversion
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-24
---

# Phase 94 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | vitest (web/kiosk), cargo test (racecontrol) |
| **Config file** | web/vitest.config.ts, kiosk/vitest.config.ts |
| **Quick run command** | `cd web && npx vitest run --reporter=verbose` |
| **Full suite command** | `cargo test -p racecontrol && cd web && npx vitest run && cd ../kiosk && npx vitest run` |
| **Estimated runtime** | ~45 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cd web && npx vitest run --reporter=verbose`
- **After every plan wave:** Run full suite command
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 45 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 94-01-01 | 01 | 1 | PRICE-01 | unit | `cargo test pricing_tier` | ❌ W0 | ⬜ pending |
| 94-01-02 | 01 | 1 | PRICE-02 | unit | `cargo test pod_availability` | ❌ W0 | ⬜ pending |
| 94-01-03 | 01 | 1 | PRICE-03 | unit | `cargo test commitment_ladder` | ❌ W0 | ⬜ pending |
| 94-01-04 | 01 | 1 | PRICE-04 | unit | `cargo test social_proof` | ❌ W0 | ⬜ pending |
| 94-02-01 | 02 | 2 | PRICE-01 | e2e | `cd web && npx vitest run pricing` | ❌ W0 | ⬜ pending |
| 94-02-02 | 02 | 2 | PRICE-02 | e2e | `cd kiosk && npx vitest run booking` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] Backend test stubs for pricing tier display, pod availability, commitment ladder, social proof
- [ ] Frontend test stubs for pricing component rendering, booking wizard scarcity display

*Existing infrastructure covers test framework setup — only stubs needed.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Middle tier visual emphasis | PRICE-01 | Visual layout verification | Open /book page, verify middle tier has Racing Red border + "Most Popular" badge |
| Pod availability color gradient | PRICE-02 | Color rendering check | View booking page with varying pod counts, verify green→yellow→red gradient |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 45s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
