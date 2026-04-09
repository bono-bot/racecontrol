# Phase 364: Session Quality Monitor - Context

**Gathered:** 2026-04-09
**Status:** Ready for planning
**Mode:** `--auto` (Claude picked recommended defaults for all gray areas; log in DISCUSSION-LOG.md)

<domain>
## Phase Boundary

**What this phase delivers:** In-flight session quality detection — detects degradation BEFORE session end and writes to `billing_sessions.suspect_reasons` (the same column Phase 363 writes at session end). Three concerns:

1. **364-01:** Wire `TelemetryGap` events into the hot path with tighter thresholds (>500ms gap, not just 60s), AND add a `SessionStalled` warning after 15s in-race telemetry silence. Both fire on the server side from the existing `bot_coordinator.rs` handler. Log events to `billing_sessions` row as in-flight markers.

2. **364-02:** Lap consistency checker — as each lap completes, compare it against the rolling lap-time distribution for this session. If a new lap falls outside the 3-sigma band, mark it as `suspect` with reason `lap_outlier`. Write to `billing_sessions.suspect_reasons` (append, not replace — D-06 from Phase 363 already defines this as a JSON array).

3. **364-03:** Silent-drop audit (zero `let _ = ws_send(...)` in hot path, rg-verified) AND expose `ws_try_send_overflows_total` counter via the existing Prometheus-style metrics infrastructure (`metrics_samples` table + `/metrics/prometheus` endpoint).

**This is the HOT-PATH counterpart to Phase 363 (end-of-session).** Phase 363 reconciles at finalize; Phase 364 monitors during the session.

**Hard scope boundaries (NOT in this phase):**
- Admin UI for in-flight quality display — Phase 367 GLD-G reads the suspect flags.
- AI-tier-aware lap time targets — Phase 365 GLD-E. Phase 364 uses a pure statistical approach (rolling sigma from observed laps, NOT a reference KB).
- Historical backfill of in-flight events — forward-only.
- Changes to rc-agent UDP ingest threshold — the existing 60s TELEM-01 threshold in `failure_monitor.rs` is for crash detection and is intentionally separate. Phase 364 adds the new 500ms gap and 15s stall thresholds server-side, routed from pod events; it does NOT change `TELEM_GAP_SECS = 60` on the pod (that would change crash detection semantics).

</domain>

<decisions>
## Implementation Decisions

### D-01: TelemetryGap Threshold (500ms vs existing 60s)

- **D-01:** The >500ms gap threshold for quality monitoring is a NEW concern separate from the existing TELEM-01 crash-detection threshold (60s). The implementation adds a new `AgentMessage::TelemetryQualityGap { pod_id, gap_ms }` variant to `rc-common/src/protocol.rs`, sent by rc-agent when a gap exceeds 500ms AND billing is active AND game_state is Running. This is NOT a replacement for the existing `TelemetryGap` (crash detection) — it is additive.
  - **Auto-pick rationale:** Overloading the existing `TelemetryGap` would conflate two semantically different signals (quality degradation vs. crash). The crash-detection at 60s survives unchanged; the quality gap at 500ms is a new signal on a new event type, keeping handlers independent.
  - _Rejected: (a) change TELEM_GAP_SECS to 0.5 — breaks crash detection, a TELEM-01 email fires every 500ms of network jitter. (b) reuse TelemetryGap with a new field — backward-compat headache on protocol decode._

- **D-02:** Server-side handling of `TelemetryQualityGap`: `bot_coordinator.rs` gets a new `handle_telemetry_quality_gap()` function. It:
  1. Logs `tracing::warn!` with pod_id and gap_ms.
  2. Appends `telemetry_gap_ms_{N}` (where N = gap rounded to nearest 100ms bucket) to `billing_sessions.suspect_reasons` for the active session on that pod, via a non-blocking DB write.
  3. Does NOT send staff email (quality gap is not a crash alert — email only for TELEM-01 60s crash).
  - **Rate-limit:** Max 1 DB write per 5 seconds per pod (debounce). If the gap persists, the single entry is sufficient; the `telemetry_coverage_pct` (Phase 363) will capture the full story at finalize.

### D-03: SessionStalled Warning (15s in-race silence)

- **D-03:** `SessionStalled` is a new `AgentMessage` variant: `SessionStalled { pod_id, silence_seconds: u32 }`. Fires after 15s of in-race telemetry silence (billing_active AND game_state::Running AND last_udp_secs_ago >= 15). This is a third threshold (joining 500ms for quality, 60s for crash).
  - **Where it fires from:** rc-agent `failure_monitor.rs` — add a new detection rule `STALL-01`. Uses a new constant `STALL_WARN_SECS: u64 = 15`. Reuses the same `telem_gap_fired`-style flag (`stall_warn_fired: bool`) to prevent repeat fires.
  - **Server handling:** New `handle_session_stalled()` in `bot_coordinator.rs`. Logs warn + appends `session_stalled_Ns` to `billing_sessions.suspect_reasons`. Does NOT auto-end the session (that's the 60s TELEM-01 path). Does NOT email staff (too noisy for a 15s window; email only at 60s crash threshold).
  - **Auto-pick rationale:** 15s silence is long enough to confirm something is wrong but short enough to catch slow-start sims like iRacing which can have 10-12s between first UDP packets on track entry. The 5s failure_monitor poll interval means the earliest fire is at ~20s; this is acceptable.

### D-04: Lap Consistency Checker (>3-sigma outlier detection)

- **D-04:** The lap consistency checker runs server-side, triggered by the existing `AgentMessage::LapCompleted` (or equivalent lap recording event in `bot_coordinator.rs`). On each lap completion:
  1. Query `billing_sessions` for the current session's lap times already recorded (the existing laps table or the telemetry packet stream — researcher to confirm which table stores per-lap times).
  2. Compute rolling mean and stddev from completed laps so far. Minimum sample: need >= 3 laps before flagging (can't compute meaningful sigma from 1-2 data points).
  3. If `|new_lap - mean| > 3 * stddev` AND stddev > 2000ms (i.e., there IS meaningful variance to compare against), append `lap_outlier_lap{N}` to `billing_sessions.suspect_reasons`.
  - **Auto-pick rationale:** Pure statistical approach — no external reference KB (that's Phase 365). 3-sigma is the standard "statistically unusual" threshold. The 3-lap minimum and stddev>2s guard prevent false positives on opening laps and flat sessions.

- **D-05:** The consistency checker does NOT cancel or void the lap. It only appends to `suspect_reasons`. The billing system continues normally. Phase 367 GLD-G-01 surfaces suspect reasons in the admin UI. The check is advisory, not authoritative.

- **D-06:** Lap times for the consistency check come from the in-memory session state on the server, NOT from a DB query per lap. The server already maintains per-pod active session state in `AppState`. Researcher must confirm whether `LapCompleted` events carry lap_time_ms or whether the server computes it from cumulative_lap_time deltas. Either way, lap times are accumulated into a `VecDeque<u32>` (max 50 entries, discard oldest) in the per-pod session state, computed in-memory, and the DB write only happens when a lap is flagged as outlier.

### D-07: Silent-Drop Audit (zero `let _ = ws_send(...)` in hot path)

- **D-07:** "Hot path" is defined as: any function called inside a live billing session's event loop — specifically:
  - `ws/mod.rs` message dispatch loop
  - `bot_coordinator.rs` handlers
  - `billing.rs` session tick / finalize path
  - `failure_monitor.rs` poll loop (rc-agent)
  - `session_audit.rs` (Phase 363)
  
  Any `let _ = ws_sender.send(...)` or `let _ = *.try_send(...)` in these files must be replaced with proper error handling (log the error, increment a counter, do NOT silently discard).
  
  **Scope:** The existing `let _ = state.dashboard_tx.send(...)` calls in non-hot-path files (ac_server.rs, ac_camera.rs, action_queue.rs, activity_log.rs) are OUT OF SCOPE for this audit. Those are broadcast channels where a lagging dashboard client dropping a message is acceptable. The audit targets WS send paths where a drop means the pod loses a command or event.
  - **Auto-pick rationale:** The roadmap success criterion says "Zero `let _ = ws_send(...)` patterns in hot path (rg verified)". The rg query for verification: `rg 'let _ = ws_sender.send' crates/racecontrol/src/ws/` — this is the specific pattern to eliminate.

### D-08: ws_try_send_overflows_total Metric

- **D-08:** The metric is exposed via the existing `metrics_samples` table and `/api/v1/metrics/prometheus` endpoint (Phase 195 + Phase 285 infrastructure). Implementation:
  1. Add an `AtomicU64 ws_try_send_overflows` counter to `AppState` (or a dedicated `WsMetrics` struct on AppState).
  2. Every place where a `try_send` fails with `TrySendError::Full` (channel overflow) in the hot path, increment this counter instead of silently discarding.
  3. A background task (or the existing metrics flush task if one exists) writes `INSERT INTO metrics_samples (metric_name, value, recorded_at)` with `metric_name = 'ws_try_send_overflows_total'` periodically (every 60s, consistent with other metric flush intervals).
  4. The Prometheus formatter in `metrics_prometheus.rs` automatically picks it up from `metrics_samples` — no format change needed.
  - **Naming convention:** `ws_try_send_overflows_total` exactly as specified in the roadmap success criterion (Prometheus counter naming: `_total` suffix for counters).

### D-09: In-Flight suspect_reasons Writes (Phase 364 appends to Phase 363's column)

- **D-09:** Phase 364 appends to `billing_sessions.suspect_reasons` (TEXT, JSON array) using the same format Phase 363 defined in D-06. The append operation is: read existing JSON array (NULL = `[]`), push new reason string, write back. This is a non-transactional read-modify-write on a non-critical advisory column. Collision risk (Phase 363 end-of-session write vs. Phase 364 in-flight write) is low but real: if a session ends mid-in-flight-write, the end-of-session run_session_audit may clobber. Resolution: use SQLite `json_insert` or string concatenation that is idempotent, NOT a full overwrite. Specific approach: `UPDATE billing_sessions SET suspect_reasons = json_patch(suspect_reasons, json_array(?)) WHERE id = ?` — researcher to confirm SQLite supports this or find equivalent.
  - **Fallback:** if SQLite doesn't support json_patch atomically, use `json_insert` or just append to the TEXT with a simple string format (`'["existing","new"]'`). The column is advisory; minor corruption from a race is acceptable; silent drops are not.

### D-10: Feature Flag

- **D-10:** Phase 364 behaviors are guarded by a single feature flag `phase364_quality_monitor` (seeded in the `feature_flags` table via the migration, `enabled = 1` by default). This provides a kill-switch if the in-flight checks produce noise in production. Same pattern as Phase 363's `phase363_session_audit` flag.

### Claude's Discretion

- Exact rc-agent `STALL_WARN_SECS` constant value if 15s proves too noisy after researcher investigates iRacing packet cadence (may adjust to 20s; target remains "detect stall well before the 60s crash threshold").
- Whether to add `TelemetryQualityGap` as a top-level `AgentMessage` variant or as a sub-variant of an existing enum (researcher should check protocol.rs enum complexity).
- Exact DB write debounce implementation for quality gap events (AtomicU64 last-write timestamp, or a tokio Mutex<Instant> — either is fine).
- Whether the lap consistency VecDeque should be 50 or 100 entries (50 is conservative; a 30-min hotlap session at 90s/lap is ~20 laps; 50 is ample).
- Exact `ws_try_send_overflows_total` flush interval (60s is the default; align with whatever the existing metrics flush interval is, researcher to confirm).

</decisions>

<specifics>
## Specific Requirements

- Success criterion 4 is RG-VERIFIED: the build process (or CI) must be able to run `rg 'let _ = ws_sender' crates/racecontrol/src/ws/` and get zero results. This is a hard gate for 364-03.
- `ws_try_send_overflows_total` metric name is EXACT — do not rename.
- The 500ms gap threshold, 3-sigma outlier threshold, and 15s stall threshold are fixed by the roadmap success criteria. They are NOT Claude's discretion.
- Phase 364 MUST NOT break Phase 363's `suspect_reasons` append logic. The `run_session_audit()` call in `billing.rs:post_session_hooks` must still run and still produce correct output after Phase 364 has written in-flight markers to the same column.

</specifics>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Phase 363 (immediate upstream — suspect_reasons contract)
- `crates/racecontrol/src/session_audit.rs` -- run_session_audit(), D-06 implementation, suspect_reasons JSON format
- `crates/racecontrol/src/db/mod.rs` -- Phase 363 ALTER TABLE migrations (~line 3972), feature_flags seeding (~line 4016)
- `.planning/phases/363-data-recording-verification/363-CONTEXT.md` -- D-06 (suspect_reasons format), D-13 (schema)

### Hot-path wiring points
- `crates/racecontrol/src/bot_coordinator.rs` -- `handle_telemetry_gap()` stub (line ~96), existing TELEM-01 handler to extend
- `crates/racecontrol/src/ws/mod.rs:1376` -- AgentMessage::TelemetryGap dispatch (where new event variants get routed)
- `crates/rc-agent/src/failure_monitor.rs` -- TELEM-01 implementation, STALL-01 goes here, `telem_gap_fired` pattern to replicate
- `crates/rc-common/src/protocol.rs:225` -- TelemetryGap variant, new variants go alongside

### Metrics infrastructure
- `crates/racecontrol/src/api/metrics_query.rs:385` -- INSERT INTO metrics_samples pattern
- `crates/racecontrol/src/api/metrics_prometheus.rs:35` -- format_prometheus(), auto-picks from metrics_samples
- `crates/racecontrol/src/api/metrics.rs` -- existing metric endpoint patterns

### Silent-drop audit scope
- `crates/racecontrol/src/ws/mod.rs` -- line 2719: the ONE existing `let _ = ws_sender.send(...)` in hot path (primary audit target)
- `crates/rc-agent/src/ws_handler.rs:324,1519,1557,1961,1973,2005` -- rc-agent try_send patterns (audit scope)

### Requirements
- `.planning/ROADMAP.md:1349` -- Phase 364 success criteria (authoritative)
- `CLAUDE.md` -- standing rules, F-05 note, financial flow E2E rule

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets

- `bot_coordinator.rs::handle_telemetry_gap()` -- existing handler stub; Phase 364 extends its routing, does not replace it
- `billing.rs::post_session_hooks()` + `session_audit::run_session_audit()` -- Phase 363 infrastructure; Phase 364 adds in-flight checks BEFORE this runs
- `metrics_query.rs::push_sample()` (or equivalent INSERT pattern at line ~385) -- reuse for ws_try_send_overflows_total
- `failure_monitor.rs` `telem_gap_fired` bool pattern -- replicate for `stall_warn_fired` (STALL-01)
- `feature_flags` table + seed pattern from Phase 363 db/mod.rs migration -- same pattern for `phase364_quality_monitor`

### Phase 363 Schema Already In Place
- `billing_sessions.suspect_reasons TEXT` -- Phase 364 appends to this (do NOT re-create)
- `billing_sessions.suspect BOOLEAN` -- Phase 364 may set this true if in-flight markers are serious
- `billing_sessions.telemetry_coverage_pct REAL` -- Phase 363 owns this; Phase 364 reads it for context only

### Known Gap: Lap Completed Events
- Researcher must confirm: does the server receive a per-lap event from rc-agent (e.g. `AgentMessage::LapCompleted` or `AgentMessage::TelemetryFrame` with lap_time_ms), or does it infer lap completion from billing_laps table writes? The lap consistency checker (D-04) depends on the answer.
- `ac_camera.rs:48` has `lap_time_ms: u32` tracking per-car in multiplayer. For single-pod sessions, check `ws/mod.rs` for what events carry lap_time_ms to the server.

### ws/mod.rs Silent-Drop Audit Finding
- `crates/racecontrol/src/ws/mod.rs:2719`: `let _ = ws_sender.send(Message::Text(json.into())).await;` -- this is the primary hot-path silent drop. Researcher should confirm whether `ws_sender` here is a `tokio::sync::mpsc::Sender` (in which case the error is `SendError` on closed channel, and logging it is correct) or a `tokio::sync::broadcast::Sender` (in which case lagged receivers are expected and silent discard IS correct). The fix depends on the channel type.

</code_context>
