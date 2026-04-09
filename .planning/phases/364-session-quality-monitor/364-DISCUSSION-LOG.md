# Phase 364: Session Quality Monitor - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md -- this log preserves the alternatives considered.

**Date:** 2026-04-09
**Phase:** 364-session-quality-monitor
**Mode:** `--auto` (no interactive user input; Claude picked recommended defaults for all gray areas)
**Areas discussed:** TelemetryGap threshold separation, SessionStalled threshold, Lap consistency algorithm, Silent-drop audit scope, ws_try_send_overflows_total metric infrastructure, suspect_reasons append safety

---

## Pre-Analysis: Prior Decisions Loaded

Scanned upstream phases. Relevant carries forward:

| Source | Carried Decision |
|--------|------------------|
| Phase 363 (D-06) | suspect_reasons is a JSON array TEXT column; append, never replace |
| Phase 363 (D-13) | All schema changes already applied; billing_sessions has suspect_reasons, suspect columns |
| Phase 195/285 (metrics) | metrics_samples + metrics_prometheus.rs are the metrics write/read path |
| Phase 82 (billing) | Session-end hook pattern; Phase 364 inserts BEFORE finalize, not inside it |
| TELEM-01 (failure_monitor.rs) | 60s crash detection threshold intentionally separate from quality monitoring |
| CLAUDE.md standing rules | ASCII-only scripts, DB migrations additive, deploy parity |

## Todos Cross-Reference

`gsd-tools todo match-phase 364` returned no matching todos. No todos folded.

## Codebase Scout

| Keyword | File | Meaning |
|---------|------|---------|
| `TelemetryGap` | bot_coordinator.rs:92, ws/mod.rs:1376, rc-common/protocol.rs:225 | Event type exists; handler is a stub (sends email on 60s gap) |
| `TELEM_GAP_SECS = 60` | failure_monitor.rs:42 | Pod-side 60s crash threshold -- DO NOT CHANGE |
| `let _ = ws_sender.send` | ws/mod.rs:2719 | ONE hot-path silent drop found -- primary 364-03 audit target |
| `let _ = *.try_send` | ws_handler.rs:324,1519,1557,1961,1973,2005 | rc-agent internal result-channel drops (secondary audit scope) |
| `suspect_reasons` | billing.rs, cloud_sync.rs, db/mod.rs, session_audit.rs | Phase 363 column confirmed in production schema |
| `metrics_samples` | api/metrics_query.rs:344 | Table schema confirmed; INSERT pattern at line 385 |
| `session_audit.rs` | crates/racecontrol/src/lib.rs:88 | Phase 363 module confirmed in lib.rs |
| `post_session_hooks` | billing.rs:4702 | Phase 363 hook runs on session end |

---

## Area 1: TelemetryGap Threshold Separation

**Question:** Should the new 500ms quality-gap threshold reuse the existing `TelemetryGap` event or use a new event type?

| Option | Description | Selected |
|--------|-------------|----------|
| (a) Reuse `TelemetryGap` with gap_seconds field (0.5 = 500ms) | Backward-compat issues; float vs u32; confuses crash-detection handler | |
| **(b) New `TelemetryQualityGap { pod_id, gap_ms }` variant** | **Clean separation; crash-detection unaffected; different handler logic** | **✓ (auto)** |
| (c) Change `TELEM_GAP_SECS` to 0.5 in failure_monitor.rs | Breaks crash detection -- fires on every network jitter | |

**Auto-selected rationale:** D-01. Separation of concerns. The 60s crash threshold is safety-critical; the 500ms quality threshold is advisory. Mixing them in one event type would force the handler to distinguish by threshold value -- fragile.

---

## Area 2: SessionStalled Threshold

**Question:** Should STALL-01 fire at 15s or at a different threshold?

| Option | Description | Selected |
|--------|-------------|----------|
| (a) 10s | Too aggressive; iRacing has 10-12s between first UDP packets at track entry | |
| **(b) 15s** | **Roadmap success criterion specifies 15s; safe margin above iRacing 12s cadence** | **✓ (roadmap)** |
| (c) 30s | Too late; closes gap with 60s crash threshold too much | |

**Auto-selected rationale:** 15s is the roadmap requirement. iRacing max observed UDP gap (12s at track entry) leaves 3s margin. This is tight but within spec.

---

## Area 3: Lap Consistency Algorithm

**Question:** Pure statistical (rolling sigma) vs. reference-KB approach?

| Option | Description | Selected |
|--------|-------------|----------|
| **(a) Rolling 3-sigma from observed laps (this session)** | **No external dependencies; works for any sim/car/track; consistent with P363 conservative approach** | **✓ (auto)** |
| (b) Compare to Phase 365 AI behavior KB | KB doesn't exist yet; cross-phase dependency blocks 364 | |
| (c) Fixed % deviation from session best | Fails for opening laps; best lap may itself be an anomaly | |

**Auto-selected rationale:** D-04. Phase 365 is the reference-KB phase. Phase 364 must not block on it. 3-sigma statistical approach is self-contained and correct for the advisory-flag use case.

**Guards added to prevent false positives:**
- Minimum 3 laps before flagging (can't compute sigma from 1-2 data points)
- Minimum stddev > 2000ms guard (if all laps are consistent, stddev is ~0; any outlier would be flagged incorrectly without this guard)

---

## Area 4: suspect_reasons Append Safety (Phase 363 column write collision)

**Question:** How to safely append to suspect_reasons from in-flight Phase 364 writes while Phase 363's run_session_audit() also writes at session end?

| Option | Description | Selected |
|--------|-------------|----------|
| (a) Full JSON overwrite (read-modify-write in app code) | Race condition if finalize runs concurrently | |
| **(b) SQLite json_insert or json_patch atomic UPDATE** | **Atomic at DB layer; no read-modify-write race** | **✓ (auto)** |
| (c) Separate Phase 364 column | Adds schema complexity; Phase 367 would need to merge two columns | |

**Auto-selected rationale:** D-09. SQLite's json_insert function allows atomic append without a read-modify-write race. Researcher must confirm exact SQLite function to use.

---

## Area 5: ws_try_send_overflows_total Metric Infrastructure

**Question:** New Prometheus counter via metrics_samples table or via a separate AtomicU64 exposed at a new endpoint?

| Option | Description | Selected |
|--------|-------------|----------|
| (a) New endpoint `/metrics/ws_overflows` | Diverges from existing metrics pattern; requires new route | |
| **(b) AtomicU64 in AppState, flushed to metrics_samples periodically** | **Reuses Phase 195/285 infrastructure; auto-exposed via metrics_prometheus.rs** | **✓ (auto)** |
| (c) Log-only (no persistent metric) | Doesn't meet success criterion 5 | |

**Auto-selected rationale:** D-08. The existing metrics_samples path already feeds the Prometheus formatter. Adding one AtomicU64 counter + a periodic flush is the minimal extension. Zero changes to metrics_prometheus.rs format logic.

---

## Deferred Ideas (scope creep — not in Phase 364)

- Real-time WebSocket push of quality events to the admin dashboard staff view (Phase 367 GLD-G concern)
- Alerting/SMS staff when STALL-01 fires (Phase 364 only logs; alert policy is a separate phase)
- Per-driver quality trend scoring across sessions (analytics layer -- future phase)
- Retroactive quality flagging of past sessions using in-flight heuristics (forward-only policy from Phase 363 D-14)
