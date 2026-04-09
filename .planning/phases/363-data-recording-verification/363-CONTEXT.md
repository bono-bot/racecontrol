# Phase 363: Data Recording Verification - Context

**Gathered:** 2026-04-09
**Status:** Ready for planning
**Mode:** `--auto` (Claude picked recommended defaults for all gray areas; log in DISCUSSION-LOG.md)

<domain>
## Phase Boundary

**What this phase delivers:** Guarantees that every completed session has complete lap data, and that billing never refunds or charges against incomplete data. Server-only, backend-heavy. Closes the three P0 data-loss gaps from `project_game_diagnostics_plan.md`:

- **P0-01** Billing race condition on lap reject
- **P0-03** CSV fallback not auto-synced
- **P1-03** No session→laps reconciliation

**Requirements (from `v46.0-REQUIREMENTS.md`):**

- **GLD-C-01** Per-session lap audit: at session end, `racecontrol` compares expected lap count vs actual recorded laps and flags sessions with >10% gap.
- **GLD-C-02** Telemetry completeness check: sessions with <80% telemetry coverage are marked `suspect: true` in the DB and surfaced in Phase 367's admin "Suspect Laps" view.
- **GLD-C-03** CSV telemetry fallback auto-sync: when primary telemetry ingest drops, the CSV fallback file is auto-synced to the server within 30s of session end.
- **GLD-C-04** Billing lap-reject 5s grace window: when a lap is rejected post-completion, billing waits 5s before finalizing so the rejection arrives before the refund calculation runs.

**NOT in scope (hard boundary — deferred to other phases):**

- **Admin Suspect Laps UI rendering** — Phase 367 GLD-G-01 (`/admin/suspect-laps` page). Phase 363 writes the DB flag only. Phase 367 reads it.
- **Retroactive flagging of historical sessions** — forward-only. Any session ending after Phase 363 deploys gets audited; pre-deploy sessions untouched. A historical backfill job, if ever needed, is a separate phase.
- **Telemetry schema replacement** — explicitly out of scope per `v46.0-REQUIREMENTS.md` "Out of Scope" section. Use the existing schema, add columns only.
- **Telemetry gap detection during session** — that's Phase 364 GLD-D-01 (`TelemetryGap` event wired into hot path). Phase 363 only does end-of-session reconciliation.
- **AI behavior lap time comparison** — Phase 365 GLD-E-01/E-02. Phase 363 uses a simple heuristic for "expected laps," not AI-tier-aware targets.
- **Pod-side code changes** — Phase 363 is server-only. All new logic lives in `crates/racecontrol/`. The CSV fallback file already exists on pods; Phase 363 only adds the server-side pull/receive endpoint and the pod-side session-end push hook (thin).

</domain>

<decisions>
## Implementation Decisions

### GLD-C-01: Expected Lap Count Heuristic

- **D-01:** Expected lap count uses a **conservative floor heuristic** for Phase 363, NOT a per-car/track AI-tier target. Formula: `expected_laps = max(1, floor(session_minutes / 3))` for trackday/practice sessions; `expected_laps = max(1, session_minutes / 2)` for hotlap sessions. Rationale: P0-01/P1-03 closure requires a "gap flag" that triggers an alert, not a precise prediction. A conservative floor avoids false positives on slow drivers while still catching catastrophic under-recording (e.g., "30min session, 0 laps recorded"). Phase 365's `ai_behavior_samples` table can later refine this to a per-car/track band — but Phase 363 ships the infrastructure and the floor.
  - _Rejected: (a) session_duration / target_lap_time — needs a target per car/track we don't have yet; would force cross-phase dependency on Phase 365. (b) rolling median of other drivers' pace — requires a minimum sample size; first-week-of-data-recording would always fail. (d) session-type-only logic — doesn't catch hotlap sessions that are genuinely fast._
  - **Auto-pick rationale:** simplest approach that closes P0 without introducing new data dependencies.

- **D-02:** The >10% gap flag is **directional** — we flag `actual < expected * 0.9` ("recorded too few"), NOT `actual > expected * 1.1` ("recorded too many"). Recording MORE laps than expected is not a data-loss indicator; it just means the driver was fast. Only under-recording signals loss.

- **D-03:** Gap flag writes to a new `sessions.lap_count_flag` column (enum: `OK` | `UNDER_RECORDED` | `UNVERIFIED`). `UNVERIFIED` = session ended before the audit ran (e.g., crash, DB shutdown). Default `UNVERIFIED` until the audit completes.

### GLD-C-02: Telemetry Completeness Metric

- **D-04:** "Telemetry completeness" is measured as **% of session-seconds with ≥1 telemetry packet received**, NOT sample count vs expected 10Hz, and NOT laps-with-full-data. Formula: `(seconds_with_any_packet / total_session_seconds) * 100`. Threshold: <80% = `suspect: true`. Rationale: (a) works across all 5 sims regardless of their native sample rates, (b) tolerates brief network blips, (c) simple to compute with a 1s bucket histogram, (d) matches the human intuition of "did the session mostly have data?"
  - _Rejected: (a) samples-vs-expected — each sim has a different native rate; AC is 10Hz, F1 is ~60Hz, iRacing is 60Hz. Normalizing across them is brittle. (b) laps-with-full-data — can't flag a session where the driver never completed a lap but should have (e.g., 30min practice with 0 completed laps and no telemetry)._

- **D-05:** Completeness metric is **computed server-side at session end** using the existing telemetry packet timestamps in the ingest pipeline. Add a 1s-bucket histogram maintained in memory during the session, flushed to `sessions.telemetry_coverage_pct` on finalize. If the server crashes mid-session, the bucket is lost and completeness = `NULL` → `lap_count_flag: UNVERIFIED`.

- **D-06:** `sessions.suspect` is a **computed boolean** derived from `lap_count_flag != OK OR telemetry_coverage_pct < 80`. Store the boolean AND the reasons: `sessions.suspect_reasons TEXT[]` (e.g., `['under_recorded','telemetry_low']`). Phase 367 GLD-G-01 reads both to render the drill-down UI.

### GLD-C-03: CSV Fallback Auto-Sync

- **D-07:** CSV fallback auto-sync uses a **pod-side session-end push hook**, NOT a server pull or a background scanner. On session end, rc-agent calls `POST /api/v1/sessions/{id}/telemetry-fallback` with the CSV file as multipart body. Server writes to `C:\RacingPoint\telemetry-fallback\{session_id}.csv` and records the receipt in `sessions.csv_fallback_received_at`. Rationale: (a) deterministic — triggered by a known event, not a timer; (b) fails loudly — if the POST fails, rc-agent retries with exponential backoff up to 10min; (c) no server-side polling of every pod every 30s; (d) matches the "session-end hook" pattern already used for other session artifacts.
  - _Rejected: (a) background scanner on rc-agent — wastes cycles checking for files that usually don't exist. (b) server pulls from pod — requires the server to know which pods have fallback files pending; inverts the natural direction of the session-end flow._
  - **30s requirement met via:** the POST fires immediately on session end; 30s budget is the retry envelope. If the first POST succeeds (normal case), latency is ~1-2s.

- **D-08:** "Fallback triggered" is defined as **any session where `telemetry_coverage_pct < 100` at session end**. If the primary ingest got everything, no CSV push happens (no point). The pod-side logic: after session end, if the primary ingest self-reports "gaps happened this session," read the pod's local CSV file (already being written as a safety net) and POST it. If the primary ingest had no gaps, skip.

- **D-09:** Server-side receipt endpoint is **staff-authenticated via the existing `sentry_service_key`** (same key rc-agent uses for `/exec` today). This is NOT a kiosk-facing endpoint. Route: `POST /api/v1/sessions/{id}/telemetry-fallback`, middleware: `require_service_key`. Max body size: 50 MB (a 30min session at 60Hz is ~2 MB CSV; 50 MB gives 25× headroom).

### GLD-C-04: Billing Lap-Reject 5s Grace Window

- **D-10:** The grace window is implemented as a **`lap_reject_pending` flag on the session + finalize re-check**, NOT a tokio delay timer. Sequence:
  1. Session ends → billing enters `FinalizePending` state, writes `lap_reject_grace_until = now() + 5s`.
  2. `finalize_session()` checks `if now() < lap_reject_grace_until { return Deferred }` — the finalize task is requeued by the billing FSM tick for re-check.
  3. Any lap reject arriving during the 5s window clears the pending lap from the recorded laps count BEFORE finalize runs.
  4. After 5s elapses, finalize proceeds with the corrected lap count.
  5. The billing FSM tick runs every 1s (existing cadence), so the worst-case additional latency for the customer is 1s beyond the 5s window = 6s total.
  - **Why not a tokio delay:** (a) doesn't survive server restart — if the server restarts during the 5s window, the timer is lost; the flag-based approach lets the next tick pick up where it left off. (b) inspectable state — `lap_reject_grace_until` in the DB is visible to admin tooling and debugging. (c) consistent with the existing billing_fsm.rs pattern.
  - _Rejected: (a) tokio::time::sleep(5s) in the finalize path — not restart-safe. (c) deferred task queue with 5s watermark — heavier than needed for a single flag._

- **D-11:** **F-05 bug fix is bundled into GLD-C-04 scope.** The billing lap-reject grace window change touches `end_billing_session()` in `billing.rs` (line ~2213-2255 per CLAUDE.md's F-05 root cause). While we're in that function, the read-after-write bug on `wallet_debit_paise` (line 2213 overwrites, line 2255 reads — customer loses Rs.162.50 per early-end) MUST be fixed in the same plan. Rationale: two people editing the same 40-line block across two phases is a merge conflict factory; the fixes are in the same conceptual domain (billing correctness at session end); the F-05 bug is a P1 that's been sitting unfixed since 2026-03-28. Reference: `.planning/audits/ROOT-CAUSE-ANALYSIS-F05-2026-03-28.md`.
  - **Scope note:** F-05 fix is additive — the grace window change already requires restructuring the finalize path, so fixing the read-after-write is ~10 additional lines. NOT scope creep; it's scope consolidation.

- **D-12:** Rejected laps during the grace window are logged to a new `lap_rejections` table (if it doesn't exist — check during research): `session_id, lap_number, rejected_at, reason, grace_window_caught BOOLEAN`. The `grace_window_caught` column lets us measure whether the 5s window is sufficient in practice (Phase 364 telemetry will surface this).

### DB Schema Changes

- **D-13:** All Phase 363 schema changes are **additive ALTER TABLE statements** on existing tables. No new tables except possibly `lap_rejections` (pending research check). Columns to add:
  - `sessions.lap_count_expected INTEGER`
  - `sessions.lap_count_actual INTEGER`
  - `sessions.lap_count_flag TEXT` (enum: OK/UNDER_RECORDED/UNVERIFIED, default UNVERIFIED)
  - `sessions.telemetry_coverage_pct REAL` (0.0-100.0, nullable for crashed sessions)
  - `sessions.suspect BOOLEAN NOT NULL DEFAULT 0`
  - `sessions.suspect_reasons TEXT` (JSON array, nullable)
  - `sessions.csv_fallback_received_at TIMESTAMP` (nullable)
  - `sessions.lap_reject_grace_until TIMESTAMP` (nullable, cleared after finalize)
- **D-14:** Migrations go in the existing migration file pattern. Per CLAUDE.md "DB migrations must cover ALL consumers" rule: `CREATE TABLE IF NOT EXISTS` won't alter existing tables — we MUST write explicit `ALTER TABLE ADD COLUMN IF NOT EXISTS` statements. Cloud sync schema (Phase 301 cloud_data_sync_v2) MUST be updated in the same commit to include the new columns in the sync payload.

### Cloud Sync Contract

- **D-15:** All new `sessions.*` columns replicate via the existing Phase 301 cloud_data_sync_v2 pipeline. No new sync path. The Phase 363 PLAN must include updating `sync/sessions.rs` (or equivalent) to include the new columns in the upsert payload. Cloud-side racecontrol on Bono VPS MUST be deployed with the same migration in the same session — deploy parity rule applies.

### Claude's Discretion

- Exact DB migration file naming and numbering (follow existing pattern in `crates/racecontrol/migrations/`)
- Whether to use `TEXT JSON` or a proper TEXT[] (if SQLite doesn't support arrays, JSON string is fine)
- Exact retry backoff schedule for the CSV fallback push (reasonable exponential default)
- Whether to add a feature flag for Phase 363 behaviors (probably yes, `feature_flags.phase363_session_audit: bool`, default true, gives a kill-switch if the audit is noisy in production)
- Telemetry metric names for the new histogram bucket (follow Phase 285/289 naming conventions)
- Exact tracing span structure for the finalize re-check loop

### Folded Todos

_No todos matched Phase 363 scope (gsd-tools reported todo_count=0)._

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Requirements & Scope
- `.planning/milestones/v46.0-REQUIREMENTS.md` §"Phase 363 — Data Recording Verification (Phase C)" — authoritative REQ-IDs GLD-C-01..C-04 and the silent-loss point mapping.
- `.claude/projects/C--Users-bono/memory/project_game_diagnostics_plan.md` — source document for the 21 silent data-loss points; P0-01, P0-03, P1-03 are closed by this phase.
- `.planning/milestones/v46.0-ROADMAP.md` — phase ordering and parallel v47.0 context.

### Billing & Session End (existing code to extend, NOT rewrite)
- `crates/racecontrol/src/billing.rs` — `end_billing_session()` is the primary modification target (F-05 bug region ~line 2213-2255). Read before touching.
- `crates/racecontrol/src/billing_fsm.rs` — existing state machine; add `FinalizePending` variant and re-check tick logic here.
- `.planning/audits/ROOT-CAUSE-ANALYSIS-F05-2026-03-28.md` — MUST READ. F-05 read-after-write bug on `wallet_debit_paise`. Phase 363 fixes this bundled with GLD-C-04.
- `.planning/phases/82-billing-and-session-lifecycle/82-CONTEXT.md` — original billing lifecycle decisions.
- `.planning/phases/198-on-track-billing/198-CONTEXT.md` — on-track billing state machine context.
- `.planning/phases/252-financial-atomicity-core/252-CONTEXT.md` — atomicity invariants that Phase 363 must preserve.
- `.planning/phases/257-billing-edge-cases/257-CONTEXT.md` — prior billing edge cases; do not re-introduce any.

### Telemetry Ingest (existing hot path, Phase 363 is end-of-session only)
- `crates/racecontrol/src/bot_coordinator.rs` — has `telemetry_gap` event already; Phase 363 reads aggregate coverage at session end only, does NOT modify hot path.
- `crates/racecontrol/src/ws/mod.rs` — WS event pipeline; Phase 364 will wire `TelemetryGap`, Phase 363 stays out.
- `crates/rc-common/src/protocol.rs` — protocol types; any new enum/struct for session audit events goes here.
- `.planning/phases/195-metrics-foundation/195-CONTEXT.md` + `.planning/phases/285-metrics-ring-buffer/285-CONTEXT.md` — metrics infrastructure to reuse for the 1s-bucket coverage histogram.

### Phase 362 — Just-Shipped Adjacent Layer
- `.planning/phases/362-post-launch-config-verification/362-01-SUMMARY.md` — shipped build `a9b5eaa3`; Phase 363 lives on top of it. The `SessionConfig` struct and `verify_launch_config()` pipeline are the "launch verified" signal; Phase 363 is the "session recorded" signal.
- `crates/racecontrol/src/launch_verifier.rs` — Phase 362's Stage 5 pattern is a good template for Phase 363's end-of-session audit stage.

### Cloud Sync (MUST be updated in same commit per deploy parity rule)
- `.planning/phases/301-cloud-data-sync-v2/301-CONTEXT.md` — cloud sync column-oriented pattern; new Phase 363 columns must be added to the sync payload.
- `crates/racecontrol/src/sync/` (exact path TBD in research) — sessions replication code.

### DB Migrations
- `crates/racecontrol/migrations/` — existing migration pattern; follow numbering convention.
- CLAUDE.md §"DB migrations must cover ALL consumers" — mandatory ALTER TABLE rule for existing databases.

### Codebase Maps (read in planning, not research)
- `.planning/codebase/ARCHITECTURE.md`
- `.planning/codebase/CONVENTIONS.md`
- `.planning/codebase/STRUCTURE.md`

### Standing Rules (cite-only, don't re-read unless relevant)
- CLAUDE.md §"Financial flow E2E" — trace currency values through complete flows before shipping; applies directly to GLD-C-04 + F-05 fix.
- CLAUDE.md §"Never hold a lock across .await" — billing_fsm is async; Phase 363 mods must snapshot + drop before awaits.
- CLAUDE.md §"Every `::default()` in new code must be reviewed" — no placeholder defaults in new session audit logic.
- CLAUDE.md §"Deploy Manifest Protocol (DMP)" — Phase 363 PLAN MUST include a `deploy:` section; racecontrol binary + cloud parity + DB migration.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- **`billing_fsm.rs` state machine pattern** — Phase 363 adds a `FinalizePending` variant and a re-check tick. Zero new infrastructure; extend the existing tick loop.
- **`telemetry_gap` event in protocol.rs + ws/mod.rs** — the protocol type already exists. Phase 364 will wire it into the hot path; Phase 363 does NOT need to touch it. Phase 363 only reads aggregate coverage at session-end, so the hot path stays clean.
- **`launch_verifier.rs` Stage 5 pattern (Phase 362)** — fresh template for an end-of-session audit stage that writes flags to the session record.
- **Metrics ring buffer (Phase 285)** — the 1s-bucket coverage histogram can piggyback on this if the API fits; otherwise a simple `Vec<bool>` sized to session duration works.
- **Phase 301 cloud_data_sync_v2 column-oriented sync** — adding new sessions columns is a low-friction change, just update the column list in the sync payload.
- **`sentry_service_key` auth middleware** — reuse for the CSV fallback POST endpoint; no new auth surface.
- **Existing rc-agent session-end hook** — the pod side already has a session-end signal; Phase 363 adds one more push target (CSV fallback) to that existing hook.

### Established Patterns
- **Additive ALTER TABLE migrations** — never rewrite existing tables; always `ADD COLUMN IF NOT EXISTS` per CLAUDE.md DB migration rule.
- **Feature flags for new runtime behaviors** — Phase 22.0+ single-binary-tier policy; new behaviors gated by `feature_flags.*` so they can be killed without redeploy.
- **Staff-authenticated POST + public GET** separation — follow Phase 27.0 MMA audit precedent; fallback POST is staff-authenticated.
- **Forward-only session handling** — past sessions are historical; new logic only applies to sessions ending after deploy. No retroactive rewrites.
- **End-of-session audit writes a flag, not an alert** — alerts fire from the downstream admin view (Phase 367). Phase 363's job is to mark the data, not notify humans.

### Integration Points
- **`end_billing_session()` in billing.rs** — primary modification site; grace window + F-05 fix + audit hook all land here.
- **`billing_fsm.rs` tick loop** — Phase 363 extends this with `FinalizePending` handling.
- **`sessions` table schema** — 7 new columns added.
- **Cloud sync payload** — new columns added to replication.
- **New endpoint `POST /api/v1/sessions/{id}/telemetry-fallback`** — added to protected routes (service key), NOT public routes.
- **rc-agent session-end hook** — one added push call for the CSV fallback (pod-side change, thin).

</code_context>

<specifics>
## Specific Ideas

- **The F-05 fix is non-negotiable and bundled into GLD-C-04.** It's been sitting unfixed since 2026-03-28 costing Rs.162.50 per early-ended 30min session. This phase touches the exact same function — fixing it now is scope consolidation, not creep.
- **Heuristic over precision for Phase 363.** The goal is closing P0s, not building the perfect session audit. A conservative floor heuristic that catches "0 laps in 30min" is enough; Phase 365 can refine to AI-tier-aware targets when the data exists.
- **Server-only phase.** The pod-side change is a single additional POST in the existing session-end hook. That's the entire pod delta — no new rc-agent state, no new binary tier, no new config.
- **Phase 363 writes flags; Phase 367 renders them.** Hard boundary. If research reveals Phase 367 expects a different flag schema, resolve by updating Phase 363's schema (research output informs CONTEXT.md for Phase 367 when it's planned).
- **Deploy parity is mandatory.** v46.0-REQUIREMENTS.md §"Phase 363" closes P0s on the venue server; cloud racecontrol on Bono VPS MUST get the same migration + binary in the same session or we lose parity and cloud suspect flags drift.

</specifics>

<deferred>
## Deferred Ideas

### Admin Suspect Laps Page (`/admin/suspect-laps`)
Out of scope — Phase 367 GLD-G-01. Phase 363 writes `suspect: true` + `suspect_reasons` and stops. Phase 367 reads and renders.

### Per-Lap Telemetry Heatmap Drill-Down
Phase 367 GLD-G-01 sub-feature. Phase 363 does not store per-lap granularity for the audit flag — just per-session.

### AI-Tier-Aware Expected Lap Count
Phase 365 GLD-E-01/E-02. Phase 363 uses a simple floor heuristic; the AI behavior KB from Phase 365 will later provide per-car/track expected bands.

### Telemetry Gap Detection in Hot Path
Phase 364 GLD-D-01. Phase 363 is end-of-session only; hot path gap detection is a separate concern.

### Retroactive Historical Session Flagging
Forward-only per D-13. If ever needed, a separate backfill job can be a future phase.

### Session Replay for Admin
Phase 367 GLD-G-03. Not Phase 363's concern.

### Batch Export of Session Data
Phase 367 GLD-G-04.

### Reviewed Todos (not folded)
_No todos were reviewed in cross_reference_todos — gsd-tools reported todo_count=0._

</deferred>

---

*Phase: 363-data-recording-verification*
*Context gathered: 2026-04-09 (--auto mode)*
*Decisions: 15 | Canonical refs: 17 | Deferred items: 7*
