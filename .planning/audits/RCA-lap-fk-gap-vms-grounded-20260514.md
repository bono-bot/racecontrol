# RCA — Lap-persistence FK gap (VMS-doctrine-grounded §S-146 v0.1)

**Authored:** 2026-05-14T16:22 IST · james-LEAD per Captain auth verbatim "authorize (1) + (2)" 2026-05-14 ~16:17 IST
**Class:** §S-146 V1↔V2 RCA · foundational-boundary (DB schema + customer-day-critical) · gates MMA Step 1 DIAGNOSE
**Trigger:** Live customer-impact — Pod 6 / Vishal / Monza session 2026-05-14T09:31:31Z–09:37:03Z completed 2 laps (117.870s + 119.686s); both rejected at lap_tracker.rs:217 with SQLite (code 787) FOREIGN KEY constraint failed; `laps` table count = 0 fleet-wide.
**Sibling artefact:** [V2-MASTER-STATE §S-N close-anchor pending]
**Doctrine reference:** [comms-link/v2-skeleton/06-vms-srl-cloud-migration-analysis.md](../../../comms-link/v2-skeleton/06-vms-srl-cloud-migration-analysis.md)
**Stale-at:** 2026-08-14 (90d from author; revisit if Phase 1 VMS ETL F25 status changes)

---

## §1 Boundary map (paths + line numbers)

**Receiving boundary (rc-agent → racecontrol):**
- [crates/racecontrol/src/ws/agent_game.rs:42-64](../../crates/racecontrol/src/ws/agent_game.rs#L42) — `handle_lap_completed(state, lap)`:
  - Line 48-50: `lap_tracker::resolve_driver_for_pod(state, &lap.pod_id)` returns `(driver_id, session_id)` from active billing timer
  - Line 52: `lap.session_id = session_id` — **assigns the BILLING timer's session_id to the LAP's session_id field**
  - Line 55-58: logs `"Lap completed: {} - {}ms on {}"` (this is the INFO line observed in production logs at 09:33:55Z + 09:35:54Z)
  - Line 58: `crate::lap_tracker::persist_lap(state, &lap).await` — kicks off DB write
  - Line 64: `state.dashboard_tx.send(DashboardEvent::LapCompleted(lap))` — broadcasts to dashboards regardless of DB outcome (WS shows the lap; DB does not)

**Persistence boundary (lap_tracker → SQLite):**
- [crates/racecontrol/src/lap_tracker.rs:60-64](../../crates/racecontrol/src/lap_tracker.rs#L60) — `resolve_driver_for_pod`:
  ```rust
  let timers = state.billing.active_timers.read().await;
  timers.get(pod_id).map(|t| (t.driver_id.clone(), t.session_id.clone()))
  ```
  Returns the BillingTimer.session_id. The BillingTimer struct at [crates/racecontrol/src/billing.rs:27](../../crates/racecontrol/src/billing.rs#L27) holds `session_id: String` — semantically the billing-session UUID (e.g. `6efedf04-7661-4b54-b154-9adf63de77ed`).
- [crates/racecontrol/src/lap_tracker.rs:98-114](../../crates/racecontrol/src/lap_tracker.rs#L98) — UX-04 comment block: prior partial fix made `billing_session_id` nullable after **"causing zero laps to be recorded for 43 days"** (incident pre-2026-04). The fix made `billing_session_id` nullable; it did NOT touch the `session_id` FK.
- [crates/racecontrol/src/lap_tracker.rs:189-214](../../crates/racecontrol/src/lap_tracker.rs#L189) — `INSERT INTO laps (..., session_id, ..., billing_session_id, ...)` — binds `lap.session_id` (line 194; the billing-session UUID) into the column with FK to `sessions(id)`, AND binds `billing_session_id` (line 211; the same value, resolved separately) into the column with no FK.
- [crates/racecontrol/src/lap_tracker.rs:216-220](../../crates/racecontrol/src/lap_tracker.rs#L216) — `tracing::error!("Failed to insert lap: {}", e)` then `tx.rollback()`. Lap is dropped; no retry; no surfacing to staff dashboard. This is the source of the two production ERROR lines observed.

**Schema boundary (DB):**
- `laps` table schema on Server .23 (live DDL, read via sqlite3 snapshot 2026-05-14T16:09 IST):
  ```sql
  CREATE TABLE laps (
    id TEXT PRIMARY KEY,
    session_id TEXT REFERENCES sessions(id),     -- V1 FK — failing
    driver_id  TEXT REFERENCES drivers(id),       -- 249 rows; satisfiable
    pod_id     TEXT REFERENCES pods(id),
    sim_type TEXT NOT NULL,
    track TEXT NOT NULL,
    car TEXT NOT NULL,
    lap_number INTEGER,
    lap_time_ms INTEGER NOT NULL,
    sector1_ms INTEGER, sector2_ms INTEGER, sector3_ms INTEGER,
    valid BOOLEAN DEFAULT 1,
    created_at TEXT DEFAULT (datetime('now')),
    car_class TEXT,
    suspect INTEGER NOT NULL DEFAULT 0,
    review_required INTEGER NOT NULL DEFAULT 0,
    session_type TEXT NOT NULL DEFAULT 'practice',
    assist_config_hash TEXT,
    assist_tier TEXT NOT NULL DEFAULT 'unknown',
    billing_session_id TEXT,                      -- V2-leaning column (no FK)
    validity TEXT NOT NULL DEFAULT 'valid',
    venue_id TEXT NOT NULL DEFAULT 'racingpoint-hyd-001'
  );
  ```
- `sessions` table schema (V1 race-session abstraction):
  ```sql
  CREATE TABLE sessions (
    id TEXT PRIMARY KEY,
    type TEXT NOT NULL,
    sim_type TEXT NOT NULL, track TEXT NOT NULL, car_class TEXT,
    status TEXT DEFAULT 'pending',
    max_drivers INTEGER, laps_or_minutes INTEGER,
    started_at TEXT, ended_at TEXT, config_json TEXT,
    created_at TEXT DEFAULT (datetime('now')),
    venue_id TEXT NOT NULL DEFAULT 'racingpoint-hyd-001'
  );
  ```
- Row counts (snapshot 2026-05-14 ~15:09 IST):
  - `sessions`: **0 rows** ← the FK target is empty
  - `billing_sessions`: 636 rows
  - `game_launch_events`: 3310 rows
  - `drivers`: 249 rows
  - `laps`: 0 rows
  - `telemetry_samples`: 0 rows (FK'd to `laps.id`; downstream-empty)
  - `ac_sessions`: 9 rows
  - `hotlap_events` / `hotlap_event_entries` / `session_highlights` / `session_feedback`: all 0

**Migration history boundary:**
- [crates/v2-db/migrations/20260503000001_initial_schema.sql:134-135](../../crates/v2-db/migrations/20260503000001_initial_schema.sql#L134) — `session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE RESTRICT` (initial V2 schema KEPT the V1 FK)
- [crates/v2-db/migrations/20260503000003_staff_table_and_fk.sql](../../crates/v2-db/migrations/20260503000003_staff_table_and_fk.sql) — recreate-table on `sessions` (preserved V1 contract)
- [crates/v2-db/migrations/20260508000001_wallet_redemptions_fk_repair.sql:42-43](../../crates/v2-db/migrations/20260508000001_wallet_redemptions_fk_repair.sql#L42) — `session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE RESTRICT` (NF-james-4 wallet repair, 2026-05-08 — re-affirmed the V1 FK in V2 substrate)
- Notably absent: any migration that creates `sessions` rows alongside `billing_sessions` inserts.

**WS broadcast vs persistence asymmetry:**
- `DashboardEvent::LapCompleted(lap)` is dispatched at agent_game.rs:64 **before** `persist_lap` completes
- Result: kiosk + spectator + admin dashboards observe laps live (via WS); DB does not. Customer-facing leaderboards (which read from DB) stay empty even when WS-attached views see the lap fly past in real-time.

**Connection-matrix touch points:**
- Local: rc-agent (Assetto Corsa adapter, AC plugin telemetry, AC results parser) → racecontrol WS → lap_tracker → SQLite + dashboard broadcast
- Remote: V2 will add `racingpoint.in/swp/...` Custom SWP backend (F24) + VMS API ETL mirror (F25) per [v2-skeleton/06 §10](../../../comms-link/v2-skeleton/06-vms-srl-cloud-migration-analysis.md#L193). Neither shipped yet; PACT-20260502-020 (VMS Replica Activation) bilateral-PARTIAL HOLDS-deferred.

---

## §2 Inherited-issue catalogue

| Item | Class | Source | Touches boundary at |
|---|---|---|---|
| **"Zero laps for 43 days" billing-gate incident** (pre-2026-04) | identity-axis-conflation with charging-axis | lap_tracker.rs:103-108 comment + `tracing::info!("UX-04: Lap has no active billing session...")` | The same FK class. Partial fix made `billing_session_id` nullable; `session_id` FK left intact, repeating the same bug class with a different column |
| **PHASE-363 MMA Anthropic Opus-4.6 finding** (2026-04-10) | FK pointing at wrong table | [PHASE-363-MMA-anthropic-claude-opus-4.6.md:36](./PHASE-363-MMA-anthropic-claude-opus-4.6.md#L36) | Quote: *"fix: Add `FOREIGN KEY (session_id) REFERENCES billing_sessions(id)` to the migration. The INSERT already handles errors with `let _ =`, so FK violations would be silently swallowed — change to log the error."* This finding pre-dates the live error logging but identifies the same FK-target-mismatch class. Disposition: FK still references `sessions(id)`; error logging is now in place (we observe the ERROR lines because of this) but no schema fix landed |
| **PHASE-363 MMA Sonnet-4.6 finding** (2026-04-10) | missing FK on lap_rejections + silent insertion | [PHASE-363-MMA-anthropic-claude-sonnet-4.6.md:57](./PHASE-363-MMA-anthropic-claude-sonnet-4.6.md#L57) | Sibling class — `lap_rejections` table contract uncertainty; observable today as `lap_rejections=0` rows despite 2 known rejections |
| **NF-james-4 wallet_redemptions FK** (2026-05-08) | recreate-table on `sessions` rewrote sibling FKs | [migration 20260508000001](../../crates/v2-db/migrations/20260508000001_wallet_redemptions_fk_repair.sql) | Same root cause class — `sessions` is foundational AND the recreate-table pattern propagates FK chaos. The repair-migration *re-affirmed* `wallet_redemptions.session_id REFERENCES sessions(id)`, which means wallet redemptions are also gated on `sessions` having rows |
| **F-05 wallet_debit_paise UPDATE-then-SELECT bug** (2026-03-28) | billing-vs-session abstraction confusion | racecontrol/CLAUDE.md "Financial flow E2E" standing rule | Adjacent class — billing/session naming and lifecycle are historically a footgun zone in this codebase |
| **§S-186 pre-§S-146 small-fix fast-lane** (Captain 2026-05-11) | NOT eligible | `feedback_pre_s146_small_fix_fastlane_20260511.md` | This RCA is schema-change class → full §S-146 fires, fast-lane carve-out does not apply |
| **§S-146 V1↔V2 RCA gate itself** (Captain 2026-05-09) | foundational-boundary trigger | `feedback_v1_dependent_v2_root_cause_before_proceeding.md` | DB schema is one of the named foundational boundaries (billing/wallet/auth/pod-state-channel/WhatsApp identity/**DB schema**) → MMA Step 1 + per-PR Captain auth required |
| **Mechanism-trust-check upstream** (bono 2026-05-10) | shared-infrastructure trust audit | racecontrol/CLAUDE.md "Mechanism-trust-check upstream of fix RCA" | 5-Q gate on the persistence pipeline before fix RCA proceeds. To be run as Section 3-prerequisite for the fix-RCA phase (post this DIAGNOSE) |
| **lap_tracker `let _` swallow** (general) | error-handling anti-pattern | Captain-listed anti-pattern in standing rules | `lap_tracker.rs:217` was upgraded from `let _` → `tracing::error!` at some point; observable error logs prove the upgrade landed. **Anti-pattern partially closed but symptom-only — root cause persists** |

---

## §3 Past-bug disposition

| Issue | Disposition | Evidence cite |
|---|---|---|
| Zero-laps-for-43-days (billing-gate) | **PATCHED-ONLY** | lap_tracker.rs:103-114 nullable-billing_session_id fix; ROOT cause (identity-vs-charging axis conflation) remains in `session_id` FK |
| PHASE-363 Opus-4.6 FK fix | **UNRESOLVED** | No follow-up migration; FK still references `sessions(id)`; PHASE-363-MMA-SUMMARY-2026-04-10.md does not flag a disposition decision |
| PHASE-363 Sonnet-4.6 lap_rejections | **UNRESOLVED** | `lap_rejections=0` in live DB despite 2 known rejections — silent-insert class persists in lap-reject path or table contract is unfulfilled |
| NF-james-4 wallet_redemptions FK | **ROOT-CAUSED-AND-FIXED for wallet_redemptions specifically**, but the **foundational pattern propagated** to laps (re-affirming sessions(id) FK in V2 substrate without ever populating sessions) — disposition: **NOT-APPLICABLE-TO-V2** for the wallet-redemption side, **OPEN-INHERITED** for the laps side |
| F-05 wallet_debit_paise | **NOT-APPLICABLE** (different mechanism; included for proximity awareness only) |
| §S-146 RCA gate | **ACTIVE-AND-FIRING** for this case |
| Mechanism-trust-check | **PENDING-RUN** for the persistence pipeline; 5-Q gate will run before any fix PR |

---

## §4 V2-alignment delta — what the boundary SHOULD look like

### §4.1 V2 substrate doctrine (VMS-grounded)

Per [v2-skeleton/06-vms-srl-cloud-migration-analysis.md §10](../../../comms-link/v2-skeleton/06-vms-srl-cloud-migration-analysis.md#L193) — the V2 lap-persistence shape:

- **Phase 0 (today):** Customer laps live on SRL VMS at `account.simracing.co.uk` (978 drivers, 3,390 sessions — Q-DATA-A on lap-count totals pending Captain API-key provisioning). Customer-facing URLs at `rps.racecentres.com/...`. Racecontrol's local `laps` table is a **secondary** persistence layer with no authoritative role.
- **Phase 1 (mirror via API):** F25 ETL paginates `/v0.1/customers`, `/v0.1/laps`, `/v0.1/laps/{id}/telemetry` at 2,000 req/hr (180/hr for telemetry exports). Imports into a **V2** lap-mirror table (NOT the V1 `laps` schema).
- **Phase 2 (URL ownership):** Custom SWP flips `rps.racecentres.com` → `racingpoint.in/swp/{hotlapping,group-events,championship,drivers}/%ID%`. Initially proxies to SRL.
- **Phase 3 (native render):** `racingpoint.in/swp/...` serves from V2 mirror data natively.
- **Phase 4 (cut over):** 4-week parity window → end SRL subscription; legacy URLs continue working from V2 mirror.

**Implication for the local `laps` table:** its V2 role is **scratch + dashboard-broadcast source** — laps captured locally for live kiosk/spectator/staff display, optionally synced upward to the V2 mirror table, but **not the system-of-record**. The V1 `sessions` FK contract was written when racecontrol's local DB was the only persistence layer; under V2, that contract is no longer load-bearing.

### §4.2 Captain identity-axis assertion

Captain 2026-05-14 ~16:09 IST verbatim: *"I think a lap should be recorded regardless of billing."*

This is the **identity-axis** contract for V2 lap-record:
- A lap belongs to the driver who drove it. Period.
- Whether they were being charged for that minute (billing-axis) is orthogonal.
- Whether a parallel "race session" abstraction was open (sessions-axis) is orthogonal.
- Whether the lap was clean / suspect / under-floor / out-of-lap-validity is orthogonal *recording-axis* (the lap is still recorded; flags annotate it).
- V2 doctrine: `lap` is identity-class. `billing_session_id` and `session_id` are **annotations** on the lap, not gating predicates.

### §4.3 Named V2-alignment gaps

| Gap | Current V1-shape | V2 doctrine | Severity |
|---|---|---|---|
| **G-V2-LAP-1** | `laps.session_id REFERENCES sessions(id)` enforces V1 identity-axis via charging-context dependency | Lap is identity-class; session is annotation | **BLOCKER** (every customer lap silently dropped today) |
| **G-V2-LAP-2** | `BillingTimer.session_id` is reused as `lap.session_id` (semantic-namespace collision) | Two columns, two namespaces: `lap.billing_session_id` (annotation), `lap.race_session_id` if/when V2 race-session abstraction exists (annotation), neither gating | HIGH |
| **G-V2-LAP-3** | `INSERT INTO laps` happens inside a transaction; FK fail → rollback → lap lost. No retry, no DLQ, no staff dashboard surfacing | Identity-class data has at-least-once persistence semantics (DLQ on FK fail; staff dashboard event; manual replay path) | HIGH |
| **G-V2-LAP-4** | `DashboardEvent::LapCompleted` broadcast precedes persistence. WS-attached views see ghost laps that never made it to DB | Either broadcast post-persistence, OR broadcast carries "pending"/"persisted" tag | MEDIUM |
| **G-V2-LAP-5** | F25 ETL not built; V2 lap-mirror table not designed; F26 VBO exporter from rc-agent AC plugin not built | Phase 1 of VMS migration plan ships F25 + F26 | OPEN — V2-PROGRESS-MAP track |
| **G-V2-LAP-6** | `lap_rejections=0` despite 2 known rejections — silent-insert class persists in lap-reject path | Lap rejections are also identity-class data; need durable record for audit | MEDIUM |
| **G-V2-LAP-7** | `telemetry_samples=0` (FK'd to `laps.id`; downstream-empty) — no per-lap telemetry being persisted either; whether that's "no telemetry sampler running" or "blocked by lap-FK fail" is unverified | Per-lap telemetry is V2-customer-deliverable (post-session coaching surface per mission-load §1.1) | OPEN — needs upstream probe |

### §4.4 Composes-with

- **V2-only forward path** (Captain 2026-05-01) — V2 keeps the local lap-capture path (kiosk/spectator real-time display) AND adds VMS ETL mirror. Both are V2.
- **Wallet Framing C LOCKED** — billing is single-purpose voucher, not session-gate. Reinforces identity-axis-vs-charging-axis separation.
- **MI mini-Jaeger §S-170** — MI baseline learning Wave 4 reads from `laps` table. Today: empty input → zero learning bootstrap. Fix unblocks Wave 4 ingestion.
- **V2-PROGRESS-MAP Layer 6 (sessions) / Layer 7 (driver substrate)** — the substrate-shape datapoint we now have (zero historical laps locally; 3,390 sessions on VMS) reshapes both layers' planning.

---

## §5 V2-framed proposal (NEUTRAL — for MMA DIAGNOSE consensus)

**Not authoring fix here.** §S-146 doctrine + Captain auth gate require MMA Step 1 DIAGNOSE consensus before fix-RCA proceeds. Three candidate directions are surfaced here as DIAGNOSE inputs, not as a chosen plan:

**Candidate A — Make `laps.session_id` nullable, drop the FK to sessions(id) in V2 substrate, keep `billing_session_id` as annotation only.**
- Pros: smallest reversible change; restores identity-axis principle; immediately unblocks customer laps; aligns with V2 doctrine that local `laps` is secondary not system-of-record; mirrors the 2026-04 partial-fix pattern (which made billing_session_id nullable) and extends it to its sibling
- Cons: loses V1 race-session referential integrity (acceptable per V2 doctrine; sessions table will likely be dropped or repurposed in V2); requires migration + sibling-FK audit (per the `migration that renames sessions` standing rule); may need `lap_rejections` parallel fix; need to handle existing rows (zero rows → trivial)

**Candidate B — Have the V2 launch flow create a sibling `sessions` row whenever a `billing_session` is created, preserving V1 FK semantics during V2 incremental rollout.**
- Pros: zero schema change; preserves V1 contract; smallest behavioral change in launcher code; gives V2 a `sessions` table populated for future use
- Cons: keeps the identity-axis-vs-charging-axis conflation alive; doesn't address the underlying V2-alignment gap; `sessions` rows become "phantom" entries that don't model real V1 race-sessions (multi-driver booked events); semantically wrong even if mechanically working

**Candidate C — Re-target the FK to `billing_sessions(id)` (per PHASE-363 Opus-4.6 recommendation), make it nullable, treat as annotation.**
- Pros: closes the PHASE-363 finding; preserves an FK contract (referential integrity within the billing-axis); minimal V2-doctrine reframing required
- Cons: still conflates identity-axis with charging-axis (a lap recorded outside a billing session — free trial, staff test, comp time — would have NULL billing_session_id and no parent record); the `billing_session_id` column already exists separately so this creates two columns with similar semantics; may regress the 2026-04 partial-fix intent

**Decision deferred to:** MMA Step 1 DIAGNOSE consensus (≥5 models, ≥3 vendor families) + Captain per-PR merge auth per §S-146 foundational-boundary doctrine. The DIAGNOSE prompt will surface all three candidates plus this RCA's full text + live log evidence + Captain identity-axis assertion as the doctrine constraint.

---

## §6 Verify-by

After fix lands, the following behavioral tests must pass (V2-LBAC closed-loop CLOSE evidence):

1. **V-LAP-FK-1:** Trigger a kiosk-staff Pod 6 / Vishal / Monza / `ks_ferrari_f2004` launch with active billing; complete 1+ laps; query `SELECT COUNT(*) FROM laps WHERE driver_id=? AND completed in window` ≥ 1; query `tracing::error! "Failed to insert lap"` count in racecontrol JSONL for the same window = 0
2. **V-LAP-FK-2 (Captain assertion):** Same as V-LAP-FK-1 but **without** active billing (e.g. staff test mode or free-trial path); lap is still persisted (identity-axis); `billing_session_id` is NULL on the inserted row
3. **V-LAP-FK-3:** Live leaderboard `/api/v1/public/leaderboard/main` returns non-empty `top_drivers` and `tracks` after ≥1 lap completed
4. **V-LAP-FK-4:** WS `DashboardEvent::LapCompleted` events match DB inserts 1:1 (no ghost broadcasts where WS shows a lap that's not in DB)
5. **V-LAP-FK-5 (Bono mirror):** cloud sync on Bono VPS shows lap parity within 60s of venue persistence (per Cloud sync: pull/push every 30s standing rule)
6. **V-LAP-FK-6 (downstream):** `telemetry_samples` non-zero after a session that records ≥1 lap (or RCA-2 if root cause is elsewhere)
7. **V-LAP-FK-7 (MI bootstrap):** MI Wave 4 ingestion can read `laps` rows; baseline-learning entry condition met

---

## §7 Sibling artefacts + Universal Sync targets

- [ ] MMA Step 1 DIAGNOSE artifact — to be authored as `RCA-lap-fk-gap-MMA-DIAGNOSE-20260514.md` after model consensus lands
- [ ] LOGBOOK row this turn
- [ ] V2-MASTER-STATE §S-N close-anchor (Captain auth pending — autonomous-push standing rule covers V2-MASTER-STATE append; foundational-PR merge for the actual code fix is Captain-gated separately)
- [ ] V2-PROGRESS-MAP §0 entry — lap-substrate row datapoint (zero local laps; VMS holds 3,390 sessions; F25 ETL deferred to PACT-020 Phase 1)
- [ ] Bono memory mirror via comms-link/briefings/bono/memory/ (bilateral §S-146 doctrine)
- [ ] Bono `cloud_sync` mirror audit (cloud `laps` table count; per H4 enumerate-fleet-wide before claim)
- [ ] No CLAUDE.md / hook / harness changes from this RCA (RCA is investigation artifact; doctrine touches only if amended)

---

**End RCA v0.1.**
