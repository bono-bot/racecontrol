# Phase 365: AI Behavior Validation via MMA - Discussion Log (Assumptions Mode)

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions captured in CONTEXT.md — this log preserves the analysis.

**Date:** 2026-04-10
**Phase:** 365-ai-behavior-validation-via-mma
**Mode:** assumptions (--auto)
**Areas analyzed:** AI Lap Collection, MMA Batch Job, KB File Format, Live Anomaly Detector, DB Schema

---

## Assumptions Presented

### AI Lap Collection

| Assumption | Confidence | Evidence |
|------------|-----------|----------|
| AI laps NOT currently in `laps` table; new `ai_behavior_samples` table needed | Confident | `lap_tracker.rs` line 239 INSERT has no `is_ai` field; `laps` schema has no `is_ai` column |
| `DifficultyTier` enum in rc-agent/ac_launcher.rs is the canonical tier type | Confident | `tier_for_level(u32) -> Option<DifficultyTier>` defined lines 89-100 of ac_launcher.rs |
| `ac_camera.rs` shows per-car SHM access pattern for AI lap data | Likely | ac_camera.rs reads `lap_time_ms` per car entry; CI = `i + 1` is AI (from write_ai_car_sections) |
| Collection hook fires at lap 3 threshold (not session end) per GLD-E-01 | Confident | GLD-E-01 explicit: "after lap 3 of any AI session" |

### MMA Batch Job

| Assumption | Confidence | Evidence |
|------------|-----------|----------|
| "Via MMA" = analytics batch using OpenRouter, NOT the Unified MMA Protocol Q1-Q4 gate | Confident | MMA Protocol v3.0 is incident-response (Q1-Q4 problem detection). Phase 365 is periodic analytics. GLD-E-02: "weekly batch job" confirms periodic, not event-driven. |
| OpenRouter key access: `OPENROUTER_KEY` env or `data/openrouter-mma-key.txt` | Confident | `server_diagnostics.rs` lines 465-469 shows exact access pattern |
| Scheduling pattern: `spawn_data_retention_job` template (tokio::time::interval) | Confident | `spawn_data_retention_job` at routes.rs line 21539 is the established pattern for background periodic jobs |
| 3/5 consensus threshold per ROADMAP.md SC-2 | Confident | ROADMAP.md success criteria: "Weekly MMA batch produces KB updates with 3/5 consensus" |
| Minimum 10 samples per (car, track, tier) before batch runs | Likely | Statistical sufficiency; GLD-E-02 doesn't specify a minimum but 10 is a reasonable floor |

### KB File Format

| Assumption | Confidence | Evidence |
|------------|-----------|----------|
| TOML in `.planning/kb/ai-behavior/{car}-{track}.toml` per GLD-E-03 | Confident | GLD-E-03 explicit: "AI behavior KB in `.planning/kb/ai-behavior/{car}-{track}.toml`" |
| One file per (car, track), all tiers within | Likely | Minimizes file count; tiers are a small enum (5 values); grouping by car+track matches the query pattern |
| KB files committed to git on batch run | Likely | `.planning/kb/` is under the planning directory; version control is standard for planning artifacts |

### Live Anomaly Detector

| Assumption | Confidence | Evidence |
|------------|-----------|----------|
| 3 consecutive laps outside band = anomaly (>3 per GLD-E-04) | Confident | GLD-E-04: ">3 consecutive laps outside band fire AiBehaviorAnomaly WS event" |
| `AiBehaviorAnomaly` added to `rc-common/src/protocol.rs` | Confident | All WS events are defined in protocol.rs (established pattern) |
| Anomaly detector runs server-side (not pod-side) | Likely | Server has session context + KB access; pod-side would require KB file sync |

### DB Schema

| Assumption | Confidence | Evidence |
|------------|-----------|----------|
| `ai_behavior_samples` is a new table (not alter existing) | Confident | `laps` has no is_ai; adding is_ai would pollute leaderboards and cloud sync payload |
| Cloud sync excludes `ai_behavior_samples` | Likely | AI analytics are venue-specific; cloud has no use case for venue AI lap data |

---

## Corrections Made

No corrections — all assumptions auto-confirmed (--auto mode, all Confident/Likely).

---

## Auto-Resolved

None — all assumptions were Confident or Likely. No Unclear items required resolution.

---

## External Research

No external research performed (codebase analysis was sufficient for all assumptions).
