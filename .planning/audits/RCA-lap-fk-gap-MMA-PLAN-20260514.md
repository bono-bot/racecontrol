# MMA Step 2 PLAN — Lap-persistence FK gap fix-design (synthesis)

**Authored:** 2026-05-14T16:55 IST · james-LEAD · per Captain auth verbatim "Proceed with your recommendation that is aligned with Racing Point ecosystem v2 development. Proceed autonomously" 2026-05-14 ~16:36 IST
**Class:** MMA v4.0 Step 2 PLAN (≥5 models / ≥3 vendor families / role-fit) · sibling to [RCA-lap-fk-gap-vms-grounded-20260514.md](./RCA-lap-fk-gap-vms-grounded-20260514.md) + [RCA-lap-fk-gap-MMA-DIAGNOSE-20260514.md](./RCA-lap-fk-gap-MMA-DIAGNOSE-20260514.md)
**Surface:** Plan-design for fix of `racecontrol.laps.session_id REFERENCES sessions(id)` FK violation; Captain per-PR merge auth retained for EXECUTE phase

---

## §1 Methodology

**Prompt:** [racecontrol/.tmp/mma-lap-fk-plan-prompt.md](../../.tmp/mma-lap-fk-plan-prompt.md) (5,731 bytes) — Step 1 consensus + 3 candidates + Captain doctrine + V2 substrate + mechanism-trust 5Q.

**Models dispatched (parallel via OpenRouter; 2 retries on initial truncation/degenerate-stream):**

| Slug | Model ID | Role | Vendor | Status | Cost |
|---|---|---|---|---|---|
| `opus-4-7` | anthropic/claude-opus-4-7 | Reasoner | Anthropic | finish=stop | $0.154 |
| `sonnet-4-6` | anthropic/claude-sonnet-4-6 | Code Expert | Anthropic | finish=stop (retry at 10000 tok) | $0.136 (retry) + $0.095 (initial truncated) |
| `deepseek-v3` | deepseek/deepseek-v3.2-exp | Reasoner+Code | DeepSeek | finish=stop | $0.0013 |
| `qwen-coder-plus` | qwen/qwen3-coder-plus | Code Expert | Qwen | finish=stop | $0.0074 |
| `gemini-2-5-pro` | google/gemini-2.5-pro | Generalist | Google | finish=stop | $0.078 |
| `deepseek-r1` (excluded) | deepseek/deepseek-r1-0528 | Reasoner | DeepSeek | DEGENERATE-STREAM (~6KB whitespace) | n/a |
| `nemotron-70b` (excluded) | nvidia/llama-3.1-nemotron-70b-instruct | SRE | Nvidia | "no endpoints found" | n/a |

**Vendor families: 4** (Anthropic ×2 max-cap, DeepSeek, Qwen, Google). **Models with complete plans: 5.** **MMA v4.0 thresholds:** ≥5 ✓, ≥3 vendor families ✓.

**Role-fit caveat (continued from Step 1):** SRE-role slot not filled. Nemotron-3-Super-120b stale; Nemotron-70b returns no-endpoints. Strict §S-166 role compliance would require `mistralai/mistral-large-2411` or another SRE-leaning model. Given the unanimous 5/5 consensus on Candidate A with confidence 0.93–0.98, the SRE-fit shortfall is unlikely to flip findings; flagged as Q-MMA-1 (carried from Step 1).

**Cost (PLAN step total): $0.3762.** Cumulative session MMA cost (Step 1 + Step 2): ~$0.78. Under $5/session budget.

---

## §2 Consensus: **Candidate A SELECTED 5/5 unanimous**

Per-model selected_candidate + confidence:

| Model | Selected | Confidence |
|---|---|---|
| opus-4-7 | **A** | 0.93 |
| sonnet-4-6 | **A** | 0.94 |
| deepseek-v3 | **A** | 0.95 |
| qwen-coder-plus | **A** | 0.94 |
| gemini-2-5-pro | **A** | 0.98 |

**Avg confidence: 0.948** (5/5 unanimous; no minority).

### §2.1 Why A — convergent rationale across models

- Only A resolves all three consensus root causes from Step 1 simultaneously (5/5 explicit cite)
- A is the smallest reversible change: new schema is strictly more permissive than old; old binaries continue to function against new schema with no data migration (binary rollback is clean)
- A aligns with Captain identity-axis doctrine: lap existence is no longer conditional on any annotation axis (sessions or billing)
- A is forward-compatible with F25 ETL: VMS lap-mirror table will be a separate substrate; DB-layer referential integrity to V1 `sessions` table is premature optimization
- B (phantom sessions row) reproduces RC-2 on a new vector and creates hostile-to-F25 phantom data (5/5 explicit cite)
- C (re-target FK to billing_sessions) resolves RC-1 but re-entrenches RC-2 on the billing axis — directly contradicts Captain doctrine since lap-without-billing still hits annotation-as-gate (5/5 explicit cite)

### §2.2 Mechanism-trust-check 5Q on Candidate A (5/5 PASS)

| Q | Question | Verdict | Evidence |
|---|---|---|---|
| Q1 | Atomic primitives? | PASS | INSERT is single SQLite txn; broadcast moved post-commit makes persist the atomic primitive |
| Q2 | TTL-bounded sentinels? | PASS | No sentinels introduced; `lap_rejections` is monotonic counter, not TTL state |
| Q3 | Behavioral-verify success? | PASS | V-LAP-FK-3 is behavioral replay of the 2026-04 incident fixture (billing UUID bound), not synthetic |
| Q4 | Single-target dry-run? | PASS | Migration is single-table (`laps`); dry-run on snapshot DB with `PRAGMA foreign_key_check` before commit |
| Q5 | Guard contracts? | PASS | "lap row exists iff persist returned Ok"; broadcast guarded by `Result::is_ok`; `lap_rejections` guarded by `Result::is_err`; mutually exclusive |

---

## §3 Selected plan (synthesized from 5-model output; Opus's plan is the primary skeleton)

### §3.1 Migration — `migrations/0023_drop_laps_session_fk.sql`

SQLite-aware multi-step (legacy_alter_table=0 forces table rewrite for FK drop):

```sql
BEGIN;
PRAGMA foreign_keys=OFF;
CREATE TABLE laps_new (
  id TEXT PRIMARY KEY,
  session_id TEXT NULL,                    -- annotation, no FK
  driver_id  TEXT REFERENCES drivers(id),  -- retained: drivers IS populated, identity-axis canonical
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
  billing_session_id TEXT,                 -- already nullable post-UX-04
  validity TEXT NOT NULL DEFAULT 'valid',
  venue_id TEXT NOT NULL DEFAULT 'racingpoint-hyd-001',
  CHECK (lap_time_ms > 0)
);
INSERT INTO laps_new SELECT * FROM laps;
DROP TABLE laps;
ALTER TABLE laps_new RENAME TO laps;
-- Recreate indexes from original schema
CREATE INDEX idx_laps_session ON laps(session_id);
CREATE INDEX idx_laps_driver ON laps(driver_id);
CREATE INDEX idx_laps_track_car ON laps(track, car);
CREATE INDEX idx_laps_leaderboard ON laps(track, car, valid, lap_time_ms);
CREATE INDEX idx_laps_driver_created ON laps(driver_id, created_at);
CREATE INDEX idx_laps_car_class ON laps(track, car_class);
CREATE INDEX idx_laps_assist_tier ON laps(track, assist_tier, validity);
CREATE INDEX idx_laps_billing_session_id ON laps(billing_session_id);
PRAGMA foreign_keys=ON;
PRAGMA foreign_key_check;  -- must return empty
COMMIT;
```

**Migration guards (per CLAUDE.md standing rule "SQLite RENAME rewrites FK refs in OTHER tables"):**
- Pre-flight: `grep -rn "REFERENCES laps" crates/v2-db/migrations/` — confirm no sibling tables reference `laps`; `telemetry_samples.lap_id REFERENCES laps(id)` IS such a sibling. Migration must preserve `telemetry_samples` FK by recreating with same name (`laps_new RENAME TO laps`). If `PRAGMA legacy_alter_table=0` is default, the rewrite is automatic; verify with `foreign_key_check`.
- Pre-flight count: `SELECT COUNT(*) FROM laps` captured before/after; invariant asserted (currently 0; will stay 0 unless live laps land during migration window — coordinate with maintenance window).
- Post-flight: `PRAGMA foreign_key_check;` must return empty result set.
- `sqlx::migrate!` cache invalidation per CLAUDE.md standing rule: `cargo clean -p racecontrol-crate` after adding the migration file (or `touch crates/racecontrol/src/lib.rs`).

### §3.2 Code changes

**[crates/racecontrol/src/ws/agent_game.rs:52](../../crates/racecontrol/src/ws/agent_game.rs#L52):**
```rust
// BEFORE
if let Some((driver_id, session_id)) =
    crate::lap_tracker::resolve_driver_for_pod(state, &lap.pod_id).await {
    lap.driver_id = driver_id;
    lap.session_id = session_id;  // assigns billing-namespace UUID into session_id
}

// AFTER
if let Some((driver_id, _billing_session_id)) =
    crate::lap_tracker::resolve_driver_for_pod(state, &lap.pod_id).await {
    lap.driver_id = driver_id;
    // session_id intentionally left as None — sessions table is V1-era abstraction
    // not populated under V2 launch flow. billing_session_id is bound separately
    // in persist_lap (line 211, annotation only). Per Captain identity-axis doctrine
    // 2026-05-14: lap is identity-class; session/billing are annotations not gates.
}
```

**[crates/racecontrol/src/ws/agent_game.rs:62-64](../../crates/racecontrol/src/ws/agent_game.rs#L62) — broadcast-after-persist (closes ghost-lap class):**
```rust
// BEFORE
crate::lap_tracker::persist_lap(state, &lap).await;
// ... broadcast fires unconditionally regardless of persist result
state.dashboard_tx.send(DashboardEvent::LapCompleted(lap));

// AFTER
let persisted = crate::lap_tracker::persist_lap(state, &lap).await;
if persisted {
    state.dashboard_tx.send(DashboardEvent::LapCompleted(lap));
} else {
    // Persist failed — emit structured lap_rejected event (already logged in lap_tracker)
    // Do NOT broadcast to dashboards; would create ghost-lap UI state
    state.dashboard_tx.send(DashboardEvent::LapRejected {
        lap_id: lap.id.clone(),
        pod_id: lap.pod_id.clone(),
        driver_id: lap.driver_id.clone(),
        reason: "persist_failed".to_string(),
    });
}
```

**[crates/racecontrol/src/lap_tracker.rs:216-220](../../crates/racecontrol/src/lap_tracker.rs#L216) — lap_rejections metric increment (closes silent-insert sibling class):**
```rust
// BEFORE
if let Err(e) = result {
    tracing::error!("Failed to insert lap: {}", e);
    let _ = tx.rollback().await;
    return false;
}

// AFTER
if let Err(e) = result {
    let _ = tx.rollback().await;
    // Increment lap_rejections counter + persist to lap_rejections table
    let _ = sqlx::query(
        "INSERT INTO lap_rejections (lap_id, pod_id, driver_id, sim_type, sqlite_code, reason, created_at, venue_id)
         VALUES (?, ?, ?, ?, ?, ?, datetime('now'), ?)"
    )
    .bind(&lap.id).bind(&lap.pod_id).bind(&lap.driver_id)
    .bind(&sim_type_str).bind(format!("{}", e.code().unwrap_or_default()))
    .bind(format!("{}", e)).bind(&state.config.venue.venue_id)
    .execute(&state.db).await;
    tracing::error!(
        pod = %lap.pod_id, driver = %lap.driver_id, lap_id = %lap.id, error = %e,
        "Failed to insert lap — recorded to lap_rejections"
    );
    return false;
}
```

### §3.3 Smallest-reversible-first commit sequence

| Commit | Scope | RCs resolved | Reversible? |
|---|---|---|---|
| **C1 (ships first; atomic)** | Migration 0023 + broadcast-after-persist + lap_rejections metric | RC-1 fully, RC-3 silent-insert class | Binary rollback clean — new schema strictly permissive of old binary's writes |
| C2 (hygiene) | agent_game.rs:52 — stop binding billing UUID into session_id | RC-2 hygiene (cleans up the parallel-column-with-same-value pattern) | Yes — code-only change |
| C3 (sibling) | wallet_redemptions FK audit + same migration shape | Sibling FK class | Yes — independent migration |

**If only C1 ships:** system is already CORRECT — RC-1 closed, lap persistence works, ghost-broadcast closed, lap_rejections instrumented. C2 and C3 are hygiene improvements, not correctness fixes.

### §3.4 Verification plan (V-LAP-FK-1..7 from RCA §6, expanded)

| Test | Behavior | Raw output proves PASS |
|---|---|---|
| V-LAP-FK-1 | Pod 6 / Vishal-class driver / Monza / `ks_ferrari_f2004` launch with active billing; complete 1+ laps | `SELECT COUNT(*) FROM laps WHERE driver_id=? AND created_at > ?` ≥ 1; zero `"Failed to insert lap"` ERROR lines in JSONL for the window |
| V-LAP-FK-2 (Captain identity-axis assertion) | Same launch WITHOUT active billing (staff test / free trial path); complete 1+ laps | Lap persisted; `billing_session_id IS NULL` and `session_id IS NULL` on the row; identity-axis intact |
| V-LAP-FK-3 (2026-04 incident replay) | Replay the exact billing-UUID-into-session_id input pattern that caused the 09:33Z + 09:35Z ERROR lines | Same INSERT shape; no SQLite 787; lap persisted |
| V-LAP-FK-4 (induced failure) | Force a CHECK violation (lap_time_ms=0); persist call returns false | `lap_rejections` table gains 1 row; `DashboardEvent::LapRejected` broadcast NOT `LapCompleted`; UI shows no ghost lap |
| V-LAP-FK-5 (WS-vs-DB consistency) | Replay live session with 5+ laps under chaos kill mid-flight | DB lap count matches WS broadcast count; no ghost laps in UI |
| V-LAP-FK-6 (migration integrity) | Post-migration `PRAGMA foreign_key_check` | Returns empty (no dangling FKs across DB) |
| V-LAP-FK-7 (sibling parity) | `INSERT INTO wallet_redemptions` with session_id=billing-UUID, sessions empty | BEFORE C3: still fails 787 (proves sibling scope) · AFTER C3: succeeds |
| V-LAP-FK-8 (downstream) | After ≥1 lap recorded, query `/api/v1/public/leaderboard` | Non-empty `top_drivers` and `tracks` |
| V-LAP-FK-9 (MI bootstrap) | After ≥1 lap recorded, MI Wave 4 ingestion entry condition | MI ingest reads `laps` non-empty; baseline-learning begins |

---

## §4 Captain decision points (gates retained)

1. **Per-PR merge auth for the C1 PR** — required per §S-146 foundational-boundary doctrine. EXECUTE phase will author the PR; CAPTAIN merges.
2. **Maintenance window for migration** — `laps=0` rows now, so a live migration is low-risk; but coordinate with any in-flight pod-6-class billing session (currently no active sessions; window is open).
3. **C2 + C3 sequencing** — ship C2 in same PR as C1, or queue as separate PRs? Recommended: same PR (small surface) but optional.
4. **C3 wallet_redemptions sibling** — same fix shape; ship within 7 days per §3.4 V-LAP-FK-7 (currently latent because there are 0 redemptions in DB but the substrate-bug exists).
5. **Q-MMA-1 SRE-role re-spawn** — if strict §S-166 role compliance required, re-run with `mistralai/mistral-large-2411`. Recommended: skip (unanimous consensus is robust to one missing slot).

---

## §5 Universal Sync targets at CLOSE

- [x] RCA artifact: [RCA-lap-fk-gap-vms-grounded-20260514.md](./RCA-lap-fk-gap-vms-grounded-20260514.md) ✓ (this session)
- [x] MMA Step 1 DIAGNOSE: [RCA-lap-fk-gap-MMA-DIAGNOSE-20260514.md](./RCA-lap-fk-gap-MMA-DIAGNOSE-20260514.md) ✓ (this session)
- [x] MMA Step 2 PLAN: this artifact ✓ (this turn)
- [ ] LOGBOOK row this turn
- [ ] Bundle commit + push (autonomous-push eligible per Captain standing rule 2026-05-12)
- [ ] V2-MASTER-STATE §S-N close-anchor in comms-link (autonomous-push eligible)
- [ ] V2-PROGRESS-MAP §0 substrate-shape datapoint (autonomous-push eligible)
- [ ] Bono memory mirror via comms-link/briefings/bono/memory/
- [ ] Bono cloud parity probe — count `laps` on Bono VPS racecontrol.db (H4 fleet-wide completeness)
- [ ] EXECUTE phase (next session or this session if Captain authorizes merge) — author the PR for C1
- [ ] VERIFY phase (Step 4) — 3-model adversarial verify on the deployed change

---

## §6 NOT tested this turn (per H3)

1. SRE-role-fit strict adherence (Q-MMA-1) — continued from Step 1; Nemotron-70b also unavailable; alternative SRE model not spawned
2. Bono VPS cloud `racecontrol.db` `laps`/`wallet_redemptions` count — fleet-wide claim deferred to bundle commit step
3. `telemetry_samples` upstream — whether empty because of lap-FK fail downstream OR independent telemetry-sampler issue
4. Why `sessions` is empty — never investigated which V1 code path was supposed to populate it (may have been removed without removing the FK contract)
5. Whether the V2 substrate has a designed-but-unimplemented `race_sessions` table for multi-driver booked events that would re-introduce a populated sessions analog (would change C2 hygiene scope)
6. The actual code fix (C1 PR) — H2 + Captain per-PR auth; not in this PLAN turn
7. Step 4 VERIFY (3-model adversarial) — runs after EXECUTE lands

---

**End MMA Step 2 PLAN synthesis v0.1.**
