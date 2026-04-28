---
created: 2026-04-28T10:49:55.878Z
title: Retire stale pact-091-mi-watermark branch — Phase 2 content already in main via PR #43 squash
area: tooling
files:
  - crates/racecontrol/src/mi_watermark.rs
  - crates/racecontrol/src/fleet_kb.rs
  - crates/racecontrol/src/metrics.rs
  - crates/racecontrol/src/api/mesh_intelligence.rs
---

## Problem

Initial framing of this todo was wrong. PR #43 (`5ca4fe3c`) was a **squash merge** that combined PACT-091 Phase 1 + Phase 2 into a single commit on `main`. The branch `origin/pact-091-mi-watermark` still carries the unsquashed `0a221e42` (Phase 1) + `158602b1` (Phase 2) + `4abdc42b` (PACT-071 test fix) commits, none of which are commit-hash ancestors of main — but **content is identical**.

Verified 2026-04-28 ~16:30 IST:
- `git diff origin/main origin/pact-091-mi-watermark -- crates/racecontrol/src/mi_watermark.rs crates/racecontrol/src/api/mesh_intelligence.rs crates/racecontrol/src/fleet_kb.rs crates/racecontrol/src/metrics.rs` → empty
- `git log -L "287,287:crates/racecontrol/src/fleet_kb.rs"` → `5ca4fe3c` (the merged commit)
- Same for `metrics.rs:138` (launch_events) and `metrics.rs:337` (recovery_events)
- All 4 production watermark callsites are wired in HEAD AND in venue `b74aadce`: `mesh_intelligence.rs:134` (config_set), `fleet_kb.rs:278` (audit_known_issues), `metrics.rs:129` (launch_events), `metrics.rs:328` (recovery_events).

The lesson: `git merge-base --is-ancestor` checks commit-hash ancestry, which always fails for squash-merged content even when the diff is null. **Use content-equivalence (`git diff branch main -- <files>`) when the question is "did the work land," not ancestry.**

## Solution

1. Confirm with branch owner (likely Bono or whoever filed PR #43) that the squash absorbed Phase 2.
2. Delete the stale remote branch: `git push origin --delete pact-091-mi-watermark` (only after confirmation; check no in-flight PR references it).
3. Drop `4abdc42b` (PACT-071 test-rot fix) — verify it's also in main; if not, cherry-pick before deleting the branch.
4. Add a feedback memory `feedback_squash_merge_ancestor_check.md` so the structural fix sticks: "for `did the work land` questions, use `git diff <branch> main -- <paths>`, not `merge-base --is-ancestor`."
5. Optionally roll the watermark coverage forward to the missing 5th callsite (`fleet_kb.rs:258` INSERT OR REPLACE has the seed write; `mi_watermark::audit_log_mi_edit` is called immediately after — already wired). Re-confirm full coverage.
