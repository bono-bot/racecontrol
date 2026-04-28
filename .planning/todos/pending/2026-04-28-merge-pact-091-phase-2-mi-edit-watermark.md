---
created: 2026-04-28T10:49:55.878Z
title: Merge PACT-091 Phase 2 — MI edit watermark — wire 3 remaining callsites
area: api
files:
  - crates/racecontrol/src/mi_watermark.rs
  - crates/racecontrol/src/api/mesh_intelligence.rs
  - crates/racecontrol/src/api/mesh_intelligence_cloud.rs
---

## Problem

PACT-091 Phase 1 (`5ca4fe3c`, merged via PR #43) shipped the MI-edit watermark scaffolding to venue at `b74aadce-dirty`: new `mi_watermark.rs` module with `MiSubsystem` enum (me/re/pa/rc/fi/pe/ad), `MiEditCtx`, `mi_edit_marker()`, `log_mi_edit()`, `audit_log_mi_edit()`, plus 3 idempotent migrations (`recovery_events.created_by_agent`, `ai_suggestions.created_by_agent`, `audit_log.action_type`). Phase 1 only wired **1 of 5** target callsites.

Phase 2 (`158602b1` "pact(091): MI edit watermark v1 — Phase 2 wires 3 remaining callsites") was committed to branch `origin/pact-091-mi-watermark` but is not an ancestor of `main` and not in the venue deploy. Verified 2026-04-28: `git merge-base --is-ancestor 158602b1 HEAD → exit 1`, `--is-ancestor 158602b1 b74aadce → exit 1`, `git branch -a --contains 158602b1 → remotes/origin/pact-091-mi-watermark`.

Until Phase 2 lands, autonomous Mesh Intelligence writes from those 3 callsites lack the 4-layer attribution (sub / tier / solution_id / confidence / src_node / model / incident_id), so forensic queries on MI edits remain a multi-file source archeology instead of the single grep / single SQL query Phase 1 promised. Source: 2026-04-28 freshness probe of MI service.

## Solution

1. Check out `origin/pact-091-mi-watermark`; rebase on `main` if needed (PACT-091 Phase 1 is already in main, so the foundation should be a no-conflict ancestor).
2. Identify the 3 callsites Phase 2 wires (likely fleet_healer / pod_healer_ai / a recovery path — confirm from the commit diff).
3. Verify each call passes a fully populated `MiEditCtx`; check `action_type` strings follow `mi-edit:<sub>` convention.
4. Open / refresh the PR. If Phase 2 touches cross-system MI write paths, run MMA per CGP v4.3 (cross-system bridge deploy = MANDATORY MMA).
5. Merge to main, push, then deploy to venue per Standing Rule #16 (commit ≠ shipped). Verify the 3 callsites emit `[MI-EDIT v1 sub=… t=… c=… s=… …]` markers in the venue tracing target `mi_edit`.
6. Confirm `audit_log` rows from those callsites carry `action_type LIKE 'mi-edit:%'` and `created_by_agent != 'human'` where applicable.
7. Update `feedback_query_mi_before_spec.md` if the watermark changes the recommended MI seeding format.
