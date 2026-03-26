---
phase: 215-self-improving-intelligence
plan: 03
subsystem: intelligence
tags: [approval-sync, proposals, self-healing, learn-05]
dependency_graph:
  requires: [215-02]
  provides: [approval-sync, apply_approved_suggestion, approve_suggestion]
  affects: [auto-detect-config.json, standing-rules-registry.json, audit/results/suppress.json, scripts/detectors/*]
tech_stack:
  added: []
  patterns: [jq-in-place-update, dual-channel-comms, source-guard, set-uo-pipefail]
key_files:
  created:
    - scripts/intelligence/approval-sync.sh
  modified: []
decisions:
  - Threshold increment 20% round-up (not down) to ensure threshold actually increases for small base values
  - suppress.json path is audit/results/suppress.json (same dir as proposals) — not audit/lib/
  - Standing rule IDs use SR-LEARNED-NNN prefix to distinguish engine-generated from manual rules
  - new_audit_check and self_patch queued for 215-04 self-patch loop — status queued_for_selfpatch, not silently dropped
metrics:
  duration: 2m
  completed: "2026-03-26"
  tasks_completed: 1
  files_created: 1
  files_modified: 0
---

# Phase 215 Plan 03: Approval Sync Summary

**One-liner:** Approval sync layer that maps approved proposals to target files (detector threshold patch, autofix allowlist, standing rule registry, suppress.json) with git commit + Bono dual-channel notification on every applied change.

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | Create approval-sync.sh | `5c63ecc8` | scripts/intelligence/approval-sync.sh |

## What Was Built

`scripts/intelligence/approval-sync.sh` exports two functions:

**`approve_suggestion(proposal_id)`**
- Finds proposal file (exact match then glob)
- Validates status must be "pending" — warns and returns 1 if already approved/applied/failed
- Updates status to "approved"
- Calls `apply_approved_suggestion`

**`apply_approved_suggestion(proposal_id)`**
- Loads proposal JSON — extracts category, bug_type, pod_ip, evidence, confidence
- Dispatches on 6 categories:
  - `threshold_tune`: finds detector script by bug_type slug, increments threshold var by 20% via sed, updates `threshold_overrides.{bug_type}` in auto-detect-config.json
  - `new_autofix_candidate`: appends bug_type to `approved_auto_fixes[]` array in auto-detect-config.json (deduplicated via jq)
  - `standing_rule_gap`: generates next SR-LEARNED-NNN ID, appends entry to standing-rules-registry.json with source=suggestion_engine
  - `cascade_coverage_gap`: appends suppress.json entry with reason=cascade_coverage_gap_pending_review and expires_at=7 days
  - `new_audit_check` / `self_patch`: queues for 215-04 self-patch loop (status=queued_for_selfpatch), returns 0
  - unknown category: sets status=apply_failed, returns 1
- After successful apply: git add staged files + git commit + git push
- Updates proposal status to "applied"
- Notifies Bono via dual-channel (WS send-message.js + INBOX.md append + git push)
- Any failure: sets status=apply_failed, logs WARN, returns 1

## Deviations from Plan

None — plan executed exactly as written.

## Self-Check

```
FOUND: scripts/intelligence/approval-sync.sh
FOUND: commit 5c63ecc8
```

## Self-Check: PASSED
