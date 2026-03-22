---
phase: 142-rules-hygiene
plan: "01"
subsystem: documentation
tags: [rules, standing-rules, claude-md, hygiene, comms]
dependency_graph:
  requires: []
  provides: [categorized-standing-rules, standing-rules-sync]
  affects: [CLAUDE.md, standing-rules.md]
tech_stack:
  added: []
  patterns: [categorized-rules, justification-comments]
key_files:
  created: []
  modified:
    - C:/Users/bono/racingpoint/racecontrol/CLAUDE.md
    - C:/Users/bono/.claude/projects/C--Users-bono/memory/standing-rules.md
    - C:/Users/bono/racingpoint/racecontrol/LOGBOOK.md
decisions:
  - "Standing Rules section placed before 4-Tier Debug Order (where Deployment Rules was)"
  - "Security category added as a short cross-reference section (allowlist auth + process guard)"
  - "F2 Learnings from standing-rules.md merged into Process sub-notes section"
  - "All _Why: justifications written from incident/failure mode perspective"
metrics:
  duration_seconds: 366
  completed_date: "2026-03-22"
  tasks_completed: 2
  files_modified: 3
requirements:
  - RULES-01
  - RULES-02
  - RULES-03
  - RULES-04
---

# Phase 142 Plan 01: Standing Rules Reorganization Summary

**One-liner:** Replaced two flat numbered rule lists in CLAUDE.md with a single categorized "## Standing Rules" block (6 categories, 28 justifications), synced to standing-rules.md.

---

## What Changed

### CLAUDE.md

- **Removed:** `## Deployment Rules` (9 numbered rules, flat list)
- **Removed:** `## Standing Process Rules` (15 numbered rules, flat list)
- **Added:** `## Standing Rules` with 6 named sub-sections

New section position: before `## 4-Tier Debug Order` (same location as old Deployment Rules).

**Final rule count by category:**

| Category | Rule count | Source |
|----------|-----------|--------|
| Deploy | 5 | Deployment Rules 1, 2, 3, 7, 8 |
| Comms | 4 | Standing Rules 6, 7+8 merged, 12, 15 |
| Code Quality | 7 | Development Rules + Standing Rules 6 (bat) + Deploy 9 |
| Process | 6 | Standing Rules 1, 2, 3, 4, 5, 9 |
| Debugging | 4 | Standing Rules 10, 11, 13, 14 |
| Security | 2 | New — allowlist auth + process guard |
| **Total** | **28** | |

**Justifications:** 28 `_Why:` lines, each referencing the incident or failure mode the rule prevents.

### standing-rules.md

- **Renamed** sections B through F to match CLAUDE.md category names exactly:
  - B. Deployment Rules → **Deploy**
  - C. Testing → **Debugging**
  - D. Code Quality → **Code Quality**
  - E. Communication → **Comms**
  - F. Process Rules → **Process**
  - F2. Learnings → merged into Process sub-notes
- **Added** `## Security` section matching CLAUDE.md Security category
- **Kept** sections A (Service Health), G (Session Discipline), H (Safety & Security) unchanged
- **Added** `_Why:` justifications matching CLAUDE.md (28 total)
- Retained standing-rules.md-only operational detail (deploy sequence steps, Windows gotchas, messaging Bono detail, debugging techniques) as sub-content under the matching category

### Pruned Rules

| Rule | Original text (abbreviated) | Absorbed into |
|------|-----------------------------|---------------|
| Deploy Rule 4 | Clean old binaries before downloading | Deploy verification sequence note |
| Deploy Rule 5 | Latest builds take priority | Deploy verification sequence (build_id match check) |
| Standing Rule 8 | Atomic sequence for push+WS+INBOX | Comms auto-push rule (content preserved verbatim) |

---

## Commits

| Hash | Message | Files |
|------|---------|-------|
| `8d545f5` | fix: MJPEG route + bypass AuthGate for cameras (also contained CLAUDE.md rewrite) | CLAUDE.md |
| `fc4cebb` | docs(142-01): reorganize standing rules into categories with justifications | LOGBOOK.md |

Note: CLAUDE.md was committed in `8d545f5` alongside unrelated fixes (MJPEG, AuthGate). The standing-rules.md change was applied to disk (outside racecontrol repo, no git tracking).

---

## Notifications

- Bono notified via comms-link WS: "Phase 142 rules hygiene complete. CLAUDE.md standing rules reorganized into 6 categories..."
- INBOX.md entry: `## 2026-03-22 10:25 IST — from james` (committed in comms-link repo HEAD)

---

## Deviations from Plan

### Auto-fixed Issues

None — plan executed as specified with one structural note:

**Deviation: CLAUDE.md was already committed in HEAD (8d545f5) before Task 1 git commit was executed**

- **Found during:** Task 1 commit attempt
- **Issue:** The file I edited on disk matched the HEAD-committed version exactly — the Edit tool wrote the same content already present in `8d545f5`. Git saw no diff.
- **Resolution:** Task 1 result is correct. CLAUDE.md in HEAD already has the Standing Rules structure. Separate LOGBOOK + docs commit (`fc4cebb`) captures the 142-01 work.
- **Impact:** Zero — acceptance criteria fully met by HEAD state.

---

## Self-Check: PASSED

Files created/modified:
- FOUND: C:/Users/bono/racingpoint/racecontrol/CLAUDE.md
- FOUND: C:/Users/bono/.claude/projects/C--Users-bono/memory/standing-rules.md
- FOUND: C:/Users/bono/racingpoint/racecontrol/.planning/phases/142-rules-hygiene/142-01-SUMMARY.md

Commits:
- FOUND: 8d545f5 (CLAUDE.md Standing Rules)
- FOUND: fc4cebb (LOGBOOK.md docs(142-01))
