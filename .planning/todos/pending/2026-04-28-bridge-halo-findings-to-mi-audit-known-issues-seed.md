---
created: 2026-04-28T10:55:00.000Z
title: Bridge HALO findings → MI audit_known_issues — let MI learn from HALO
area: api
files:
  - comms-link/data/halo-findings.jsonl
  - crates/racecontrol/src/api/mesh_intelligence.rs
  - crates/racecontrol/src/fleet_kb.rs
---

## Problem

HALO and MI are two parallel observability systems with no shared data path. Memory `project_pact_halo_consolidation_20260426.md` flagged "rules-as-docs ≠ rules-as-hooks" — same shape applies here: HALO's findings are docs (JSONL log + 6 catalog markdown files); MI's `audit_known_issues` is the lookup table fleet_healer + diagnose paths actually consult. Until findings cross the seam, MI cannot use HALO experience to short-circuit fresh symptom matches.

Concrete shape of the gap, observed 2026-04-28 ~16:35 IST:

- `comms-link/data/halo-findings.jsonl` has **483 lines** across 15 distinct `probe_id` values. Verdict distribution: 302 VIOLATED, 174 HOLD, 6 NODATA, 1 no_verdict. Top probes: `X.1` (102 — WS thrash p50 lifetime), `X.3` (102 — Bono recipient read-receipt rot), `C.1` (102), `C.6` (102), `V.10/V.11/V.2` (11 each), `R.4` (11).
- HALO has a SQL schema (`comms-link/migrations/001_halo_findings.sql`, `002_halo_acks.sql`) — separate database from racecontrol's sqlite.
- MI's `audit_known_issues` table (defined in `fleet_kb.rs:126`) is read by `check_audit_known_issues` (fleet_kb.rs:156) which is called from `/api/v1/mesh/audit-check` and `audit-check-service`.
- Memory `project_pact_halo_consolidation_20260426.md` claimed "0 LIVE probes (32 PROPOSED)." Outdated — at minimum 15 probe IDs are emitting findings to disk; whether they are formally `LIVE` per HALO-CHARTER lifecycle is a separate question.
- James-side cannot POST to `/api/v1/mesh/audit-seed-service` directly: no `RC_SERVICE_KEY` in env (per `feedback_query_mi_before_spec.md`).

## Solution

**Bridge design — ship as PACT (cross-system, needs MMA per CGP v4.3 cross-bridge rule).**

1. **Source-of-truth choice:** treat `halo-findings.jsonl` as the wire format. Each line already has `probe_id`, `name`, `severity`, `verdict`, `evidence`, `where`, `ts` — sufficient to populate `audit_known_issues` rows.
2. **Schema mapping** (HALO finding → audit_known_issues row):
   - `problem_key` ← `halo:<probe_id>` (e.g. `halo:X.1`)
   - `symptom_patterns` ← extract regex hints from `evidence` text + probe `name`; on first ingest of a probe, seed canonical patterns from the matching `HALO-CATALOG-<L>.md` "Detection method" section
   - `escalation_message` ← `name` + worst-severity `evidence` from last N findings of that probe
   - `fix_status` ← derive from verdict mix: ≥1 RESOLVED→`fixed`; all VIOLATED→`open`; mostly HOLD→`monitoring`
   - `created_by_agent` ← `halo-bridge`
3. **Bridge writer:** new script `scripts/mi/seed-from-halo.{sh,js}` that:
   - Tails `halo-findings.jsonl` (or reads since last `last_offset` checkpoint stored in `data/halo-bridge-decisions.jsonl`)
   - Groups by `probe_id`, computes the row, POSTs to `http://192.168.31.23:8080/api/v1/mesh/audit-seed-service` with `X-Service-Key: $RC_SERVICE_KEY`
   - Idempotent via `INSERT OR REPLACE` semantics already present in `fleet_kb.rs:258`
   - Logs each write through `mi_watermark::audit_log_mi_edit` with `MiEditCtx { sub: MiSubsystem::Me, src_node: "halo-bridge", ... }` so the 04-26 watermark works for HALO-sourced edits too
4. **Auth path:** Bono-side runs the bridge (Bono has the service key); James-side gets read-only `/audit-check?symptom=halo:X.1` access via JWT. Closes James's auth gap by relegating writes to Bono.
5. **Cadence:** every 5 min (matches HALO charter cadence band) — small batch, no thunder.
6. **Rollback:** environment switch `MI_HALO_BRIDGE=0` disables bridge; existing rows persist (idempotent so re-enable is safe).
7. **PACT proposal:** open as `PACT-20260428-XXX` with category `cross-system bridge deploy` so MMA gates it (per CGP v4.3). Compose with `PACT-091` (watermark — bridge writes flow through watermarked path) and `PACT-068` (HALO smoke-test reinforcement).

## Why this is the right structural shape

HALO charter §"What HALO is NOT" rules out HALO becoming a drilled-motor auto-fix layer. So HALO findings are observations needing a learner. MI's `audit_known_issues` already serves the learner role — symptom→fix lookup with versioned `fix_status`. Bridging HALO→MI is the smaller of two structural choices (the alternative — building a parallel KB inside HALO — duplicates MI). Bridging also means future MI consumers (fleet_healer, diagnose) get HALO experience for free without per-consumer changes.

## NOT TESTED / open questions

- Does `audit_known_issues.problem_key` accept the `halo:<probe_id>` namespace, or do we need a separate column? (`fleet_kb.rs:258` INSERT OR REPLACE keys on `problem_key`; should be free-form, but spec-check.)
- Severity-to-fix_status mapping needs review with someone who's tuned a probe — naive verdict counting may misclassify CALIBRATING noise as `open`.
- Cross-hemi: HALO findings include `where: bono-vps`. Should bridge filter to venue-relevant probes only (HALO-V, HALO-R, some HALO-X), or ingest all 6 layers? Answer affects probe-noise in `audit_known_issues`.
- `4abdc42b` (PACT-071 test-rot fix on the stale pact-091-mi-watermark branch) — verify in main before retiring branch.
