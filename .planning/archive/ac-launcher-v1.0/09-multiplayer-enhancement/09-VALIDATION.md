---
phase: 9
slug: multiplayer-enhancement
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-14
---

# Phase 9 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust) + Next.js TypeScript compile |
| **Config file** | Cargo.toml workspace |
| **Quick run command** | `cargo test -p rc-core -- multiplayer` |
| **Full suite command** | `cargo test -p rc-common && cargo test -p rc-agent && cargo test -p rc-core` |
| **Estimated runtime** | ~15 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p rc-core -- multiplayer && cd pwa && npx tsc --noEmit`
- **After every plan wave:** Run full suite
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 15 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 09-01-01 | 01 | 1 | MULT-02, MULT-05 | unit | `cargo test -p rc-common` | YES | pending |
| 09-01-02 | 01 | 1 | MULT-01, MULT-02, MULT-05, MULT-06 | unit | `cargo test -p rc-core --lib -- ac_server && cargo test -p rc-core --lib -- multiplayer` | YES | pending |
| 09-02-01 | 02 | 1 | MULT-03 | unit | `cargo test -p rc-core --lib -- billing` | YES | pending |
| 09-02-02 | 02 | 1 | MULT-03 | unit | `cargo test -p rc-core --lib -- billing` | YES | pending |
| 09-03-01 | 03 | 2 | MULT-04 | build | `cd pwa && npx tsc --noEmit` | YES | pending |
| 09-03-02 | 03 | 2 | MULT-04 | build | `cd pwa && npx tsc --noEmit` | YES | pending |

*Status: pending / green / red / flaky*

---

## Wave 0 Requirements

- [ ] Test stub for AssettoServer config generation (AI entries, EnableAi flag)
- [ ] Test stub for synchronized billing start/stop coordination
- [ ] TypeScript compilation checks for PWA after lobby UI changes

*Existing test infrastructure covers framework needs. Only test stubs needed.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Multiple pods join same AC server race | MULT-01 | Requires 2+ pods + AC server running | Start multiplayer, verify 2 pods see each other in race |
| AI opponents visible in multiplayer race | MULT-02 | Requires AssettoServer + pods | Start multiplayer with AI, verify AI cars on track |
| Billing synchronized across pods | MULT-03 | Requires multiple pods with billing active | Book group session, verify billing starts together |
| PWA lobby shows join status in real-time | MULT-04 | Requires browser + multiple devices | Open lobby on 2 phones, verify live status updates |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 15s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
