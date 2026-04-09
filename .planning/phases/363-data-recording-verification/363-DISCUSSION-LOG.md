# Phase 363: Data Recording Verification - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-04-09
**Phase:** 363-data-recording-verification
**Mode:** `--auto` (no interactive user input; Claude picked recommended defaults for all gray areas)
**Areas discussed:** Expected lap count heuristic, Telemetry completeness metric, CSV fallback auto-sync mechanism, Billing grace window implementation, Suspect lap storage contract, Retroactive flagging scope, Cloud sync contract, F-05 bug scope inclusion

---

## Pre-Analysis: Prior Decisions Loaded

Scanned 160+ prior CONTEXT.md files. Relevant carries forward:

| Source | Carried Decision |
|--------|------------------|
| Phase 82 (billing-and-session-lifecycle) | Session-end hook pattern; billing FSM is authoritative for finalize |
| Phase 198 (on-track-billing) | On-track billing FSM states; keep existing state machine, extend don't replace |
| Phase 252 (financial-atomicity-core) | Atomicity invariants on refund/wallet writes — must preserve |
| Phase 257 (billing-edge-cases) | Prior edge cases catalogued; do not re-introduce |
| Phase 301 (cloud-data-sync-v2) | Column-oriented cloud sync; new columns replicate via existing pipeline |
| Phase 195/285 (metrics foundation / ring buffer) | Reuse metrics infrastructure for coverage histogram |
| Phase 362 (post-launch-config-verification) | Stage 5 audit pattern from launch_verifier.rs — template for end-of-session audit |
| CLAUDE.md F-05 incident (2026-03-28) | Read-after-write bug in end_billing_session() on wallet_debit_paise — still unfixed |
| CLAUDE.md "Financial flow E2E" rule | Trace currency values through complete flows before shipping |
| CLAUDE.md "DB migrations must cover ALL consumers" | ALTER TABLE IF NOT EXISTS mandatory for existing tables |

## Todos Cross-Reference

`gsd-tools todo match-phase 363` returned `todo_count: 0`. No todos folded, no todos deferred-after-review.

## Codebase Scout

Grep results for Phase 363 scope keywords:

| Keyword | Files Found | Meaning |
|---------|-------------|---------|
| `lap_reject` | bot_coordinator.rs, ws/mod.rs, rc-common/protocol.rs | Lap rejection events already flow through protocol |
| `telemetry_gap` | (same set) | `TelemetryGap` event defined but currently `let _ =`'d (Phase 364 will wire it) |
| `csv_fallback` | (none) | New concept — does not exist yet |
| `suspect_lap` | (none) | New concept — does not exist yet |
| `end_billing_session` | billing.rs, billing_fsm.rs, api/routes.rs, bot_coordinator.rs | Primary modification target confirmed |

---

## Area 1: Expected Lap Count Heuristic

| Option | Description | Selected |
|--------|-------------|----------|
| (a) session_duration / target_lap_time | Needs per-car/track targets we don't have until Phase 365 | |
| (b) Rolling median of other drivers' pace | Requires minimum sample size; first-week deployment would always fail | |
| **(c) Conservative floor heuristic: `max(1, session_minutes/3)` for trackday; `/2` for hotlap** | **Simplest; closes P0 without new data dependencies; catches "0 laps in 30min"** | **✓ (auto)** |
| (d) Session-type-only logic | Doesn't catch hotlap sessions that are genuinely fast | |

**Auto-selected rationale:** Conservative floor is the smallest implementation that achieves P0 closure. Phase 365's AI behavior KB can later refine this to per-car/track bands. Recording the expected value lets us upgrade the formula in a later phase without rewriting consumers.

**Notes:** Decision made directional — flag only when `actual < expected * 0.9`. Over-recording (fast driver) is not a data-loss signal.

---

## Area 2: Telemetry Completeness Metric

| Option | Description | Selected |
|--------|-------------|----------|
| (a) Samples vs expected 10Hz × session seconds | Brittle across sims with different native rates (AC 10Hz, F1 60Hz, iRacing 60Hz) | |
| (b) % of laps with full telemetry rows | Can't flag sessions where driver never completed a lap | |
| **(c) % of session-seconds with ≥1 telemetry packet** | **Rate-agnostic; tolerates brief blips; matches human intuition** | **✓ (auto)** |

**Auto-selected rationale:** Sim-rate-agnostic is the only approach that works across all 5 sim adapters without per-sim calibration. 1s-bucket histogram is cheap to compute.

**Threshold: <80% = suspect.** This threshold is the value from v46.0-REQUIREMENTS.md GLD-C-02 and was not re-discussed.

**Notes:** If server crashes mid-session, bucket is lost → completeness = NULL → `lap_count_flag: UNVERIFIED` (not `suspect: true`).

---

## Area 3: CSV Fallback Auto-Sync Mechanism

| Option | Description | Selected |
|--------|-------------|----------|
| **(a) Pod-side session-end push hook** | **Deterministic trigger; fails loudly with retries; no server-side polling; matches existing session-end hook pattern** | **✓ (auto)** |
| (b) Rc-agent background scanner | Wastes cycles checking for files that usually don't exist | |
| (c) Server pulls from pod | Inverts natural session-end flow direction; server must track which pods have pending files | |

**Auto-selected rationale:** Push matches the existing session-end hook direction. Pod knows when its session ended; pod decides whether to push (only if `telemetry_coverage_pct < 100`).

**Notes:**
- 30s budget is the retry envelope, not the normal case. Normal case is ~1-2s for a successful POST.
- Server endpoint: `POST /api/v1/sessions/{id}/telemetry-fallback`, service-key authenticated (reuses existing auth).
- Max body size 50 MB (25× headroom vs a 30min hotlap session's ~2 MB CSV).

---

## Area 4: Billing 5s Grace Window Implementation

| Option | Description | Selected |
|--------|-------------|----------|
| (a) `tokio::time::sleep(5s)` in finalize path | Not restart-safe; timer lost on server restart | |
| **(b) `lap_reject_grace_until` flag + FSM re-check tick** | **Restart-safe; inspectable DB state; matches existing billing_fsm pattern** | **✓ (auto)** |
| (c) Deferred task queue with 5s watermark | Heavier than needed for a single flag | |

**Auto-selected rationale:** Flag-based approach survives restart, is debuggable via DB inspection, and extends rather than replaces the existing billing_fsm tick loop. Worst-case customer latency = 5s grace + 1s tick = 6s beyond session end.

**Notes:**
- Rejected laps during grace window logged to new `lap_rejections` table (pending research confirmation that this table doesn't exist yet).
- `grace_window_caught` column lets us measure whether 5s is empirically sufficient.

---

## Area 5: F-05 Bug Scope Inclusion

| Option | Description | Selected |
|--------|-------------|----------|
| (a) Leave F-05 unfixed; Phase 363 only touches grace window logic | Guaranteed merge conflict with future F-05 fix; leaves P1 bug costing Rs.162.50/session | |
| **(b) Bundle F-05 fix into GLD-C-04 plan** | **Same function touched; same conceptual domain (billing correctness at session end); ~10 additional lines; closes a P1 that's been sitting unfixed since 2026-03-28** | **✓ (auto)** |
| (c) Create a separate Phase 363.1 for F-05 | Administrative overhead for a 10-line fix in a function we're already restructuring | |

**Auto-selected rationale:** Scope consolidation, not creep. The grace window change already requires restructuring `end_billing_session()` (line ~2213-2255); fixing the read-after-write on `wallet_debit_paise` in the same block is cheap, correct, and avoids a merge conflict factory.

**Reference:** `.planning/audits/ROOT-CAUSE-ANALYSIS-F05-2026-03-28.md`

---

## Area 6: Suspect Lap Storage Schema

| Option | Description | Selected |
|--------|-------------|----------|
| **(a) `sessions.suspect BOOLEAN` + `sessions.suspect_reasons TEXT` (JSON array)** | **Normalized; trivial to query; Phase 367 can render reasons drill-down; minimal cloud sync change** | **✓ (auto)** |
| (b) New `suspect_sessions` table | New table to sync, new join for admin view, no benefit over columns | |
| (c) Bitfield flags column | Opaque to debug; SQLite has no native bitfield op support | |

**Auto-selected rationale:** Boolean + reasons is the cleanest contract for Phase 367 to consume. `suspect` is a computed derivation of `lap_count_flag != OK OR telemetry_coverage_pct < 80` — but we store it explicitly so Phase 367's query is a simple WHERE clause, not a computed expression.

---

## Area 7: Retroactive Flagging Scope

| Option | Description | Selected |
|--------|-------------|----------|
| **(a) Forward-only (sessions ending after deploy)** | **No historical rewrites; deterministic scope; simple rollout** | **✓ (auto)** |
| (b) Retroactive backfill of historical sessions | Requires batch job, DB read load, ambiguous completeness data for old sessions | |

**Auto-selected rationale:** Forward-only is the safest rollout. Historical sessions don't have the 1s-bucket coverage histogram data anyway — it's only collected starting at deploy time. If backfill is ever needed, it becomes its own phase.

---

## Area 8: Cloud Sync Contract

| Option | Description | Selected |
|--------|-------------|----------|
| **(a) Extend existing Phase 301 cloud_data_sync_v2 payload with new columns** | **Minimal change to existing sync path; no new sync mechanism** | **✓ (auto)** |
| (b) New sync channel for Phase 363 columns | Over-engineered; existing column-oriented sync handles this trivially | |

**Auto-selected rationale:** Phase 301 already does column-oriented sync; adding 7 new columns is a 1-line change to the column list. Cloud deploy parity is mandatory — cloud racecontrol on Bono VPS MUST get the same migration + binary in the same session.

---

## Claude's Discretion (deferred to planner/executor)

- Exact migration file numbering (follow existing pattern in `crates/racecontrol/migrations/`)
- SQLite TEXT[] vs JSON TEXT encoding for `suspect_reasons`
- Retry backoff schedule for CSV fallback POST (reasonable exponential default)
- Feature flag naming (`feature_flags.phase363_session_audit`, default true)
- Metric names for the 1s-bucket coverage histogram (follow Phase 285/289 naming)
- Tracing span structure for finalize re-check loop

## Deferred Ideas

All 7 items listed in CONTEXT.md `<deferred>` section:

1. Admin Suspect Laps page — Phase 367 GLD-G-01
2. Per-lap telemetry heatmap drill-down — Phase 367 GLD-G-01 sub-feature
3. AI-tier-aware expected lap count — Phase 365 GLD-E-01/E-02
4. Telemetry gap detection in hot path — Phase 364 GLD-D-01
5. Retroactive historical session flagging — future phase if ever needed
6. Session replay for admin — Phase 367 GLD-G-03
7. Batch export of session data — Phase 367 GLD-G-04

## Reviewed Todos (not folded)

_None — gsd-tools reported todo_count=0 for Phase 363._

---

*Discussion completed: 2026-04-09 (--auto mode, no user interaction)*
*Next: /gsd:plan-phase 363 — in a fresh session, after user reviews CONTEXT.md*
