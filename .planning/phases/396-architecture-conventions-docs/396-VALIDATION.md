---
phase: 396
slug: architecture-conventions-docs
status: complete
nyquist_compliant: true
wave_0_complete: true
created: 2026-04-16
signed_off: 2026-04-16
---

# Phase 396 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.
> **Phase type:** documentation-only. No compilation, no runtime, no deploy.
> Validation = grep/awk/file-existence assertions over the staged `ARCHITECTURE.md` + `CONVENTIONS.md` drafts.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | `bash` + `grep` + `awk` + `wc` (no test runner needed) |
| **Config file** | none — validation is shell-script assertions |
| **Quick run command** | `bash .planning/phases/396-architecture-conventions-docs/verify-396.sh` (Wave 0 creates this) |
| **Full suite command** | same — one doc phase has one verification script |
| **Estimated runtime** | ~2 seconds |

---

## Sampling Rate

- **After every task commit:** Run `verify-396.sh`
- **After every plan wave:** Run `verify-396.sh` (same — single script)
- **Before `/gsd:verify-work`:** verify-396.sh must exit 0 with all 12 checks green
- **Max feedback latency:** 5 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 396-01-01 | 01 | 0 | FND-03 | infra | `test -x .planning/phases/396-architecture-conventions-docs/verify-396.sh` | ❌ W0 | ⬜ pending |
| 396-02-01 | 02 | 1 | FND-03 | structural | `test -f .planning/phases/396-architecture-conventions-docs/CONVENTIONS.md` | ❌ W0 | ⬜ pending |
| 396-02-02 | 02 | 1 | FND-03 | grep | `grep -q '^## Deferred Rules' CONVENTIONS.md` | ❌ W0 | ⬜ pending |
| 396-02-03 | 02 | 1 | FND-03 | grep | `grep -cE '^\| [0-9]+ \|' CONVENTIONS.md` ≥ 8 deferred rows | ❌ W0 | ⬜ pending |
| 396-02-04 | 02 | 1 | FND-03 | grep | every `Enforcer:` line names a file path OR a `Phase <N>` cite | ❌ W0 | ⬜ pending |
| 396-02-05 | 02 | 1 | FND-03 | size | `wc -l CONVENTIONS.md` ≤ 500 | ❌ W0 | ⬜ pending |
| 396-03-01 | 03 | 1 | FND-03 | structural | `test -f .planning/phases/396-architecture-conventions-docs/ARCHITECTURE.md` | ❌ W0 | ⬜ pending |
| 396-03-02 | 03 | 1 | FND-03 | grep | ARCHITECTURE.md decision table contains ≥ 11 artifact-type rows | ❌ W0 | ⬜ pending |
| 396-03-03 | 03 | 1 | FND-03 | grep | every row prefixed `[DEFERRED — Phase N]` (matches `\[DEFERRED — Phase [0-9]+\]`) | ❌ W0 | ⬜ pending |
| 396-03-04 | 03 | 1 | FND-03 | grep | ARCHITECTURE.md has `## Adding a New Artifact Type` section | ❌ W0 | ⬜ pending |
| 396-03-05 | 03 | 1 | FND-03 | size | `wc -l ARCHITECTURE.md` ≤ 500 | ❌ W0 | ⬜ pending |
| 396-04-01 | 04 | 2 | FND-03 | semantic | every Phase cite `(Phase N)` in both docs references a phase that exists in `.planning/phases/` OR in ROADMAP v52 table | ❌ W0 | ⬜ pending |
| 396-04-02 | 04 | 2 | FND-03 | traceability | guiding principle sentence ("If a rule is not enforced mechanically...") appears verbatim in CONVENTIONS.md head and ARCHITECTURE.md head | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `.planning/phases/396-architecture-conventions-docs/verify-396.sh` — 13 assertions above, bash + grep + awk, exit 0 on all green
- [ ] Script is executable (`chmod +x`)
- [ ] Script is idempotent — safe to re-run after any edit to CONVENTIONS.md or ARCHITECTURE.md

*No framework install needed. No conftest.py. No fixtures.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Prose quality of "Why this structure?" framing | FND-03 | Subjective readability; no grep can score prose quality | James reads both docs end-to-end before marking phase complete; check against 393-CONTEXT.md "Rationale Notes" tone |
| Google Drive analogy attribution | FND-03 | Attribution text is prose, not a pattern | grep finds the phrase but not whether it's tonally correct; eyeball it |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify via verify-396.sh OR Wave 0 script dependency
- [ ] Sampling continuity: single script covers every plan task — no gap possible
- [ ] Wave 0 covers all MISSING references (the verify script itself)
- [ ] No watch-mode flags
- [ ] Feedback latency < 5s (bash grep on <500-line files)
- [ ] `nyquist_compliant: true` set in frontmatter after Wave 0 executes

**Approval:** approved

---

## Sign-Off Block (Phase 396 close)

**Date:** 2026-04-16
**Verified by:** verify-396.sh (deterministic, 13 checks)
**Result:** 13 passed / 0 failed / 0 pending

### verify-396.sh output (captured verbatim)

```
[1/13] Both files exist (gating)
  PASS: CONVENTIONS.md + ARCHITECTURE.md exist
[2/13] Line cap (<= 500)
  PASS: CONVENTIONS.md=59 lines, ARCHITECTURE.md=109 lines
[3/13] Guiding principle verbatim in both
  PASS: guiding principle present in both docs
[4/13] Deferred Rules section present in CONVENTIONS.md
  PASS: Deferred Rules section present
[5/13] All 8 rules demoted to Deferred
  PASS: Deferred Rules contains 8 rows
[6/13] Every Deferred row cites phase 397-412
  PASS: Deferred Rules cites 8 creating-phase references (>=8)
[7/13] ARCHITECTURE.md has >= 11 artifact-type rows
  PASS: ARCHITECTURE.md has 11 DEFERRED rows (>=11)
[8/13] Every ARCHITECTURE decision table row is [DEFERRED — Phase N] or header
  PASS: no orphan decision table rows
[9/13] No stray live Enforcer citations
  PASS: no live Enforcer: lines in either doc
[10/13] D-12 footer present in both
  PASS: D-12 footer present in both docs
[11/13] Rule #9 (canonical-source marker) NOT added per D-05
  PASS: CONVENTIONS.md contains no canonical-source marker
[12/13] 393 back-references
  PASS: ARCHITECTURE.md=4 (>=3), CONVENTIONS.md=4 (>=2) '(Phase 393' cites
[13/13] ARCHITECTURE.md has '## Adding a New Artifact Type' section
  PASS: Adding a New Artifact Type section present

======================================
Phase 396 Verification: 13 passed / 0 failed / 0 pending
======================================
```

### Manual-only items

- Prose quality of "Why this structure?" framing — reviewed against 393-CONTEXT tone, acceptable. The ARCHITECTURE.md "Google Drive Analogy" section carries the Phase 393 §Rationale-Notes register (neutral origin + authority lineage) without embellishment.
- Google Drive analogy attribution — present at ARCHITECTURE.md "The Google Drive Analogy" section, verbatim Uday quote attributed to Uday Singh, 2026-04-15 with `(captured in Phase 393)` parenthetical.

**Phase 396 validation: COMPLETE.**
