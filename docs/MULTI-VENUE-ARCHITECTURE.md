# Multi-Venue Architecture

**Phase:** 303 — Multi-Venue Schema Prep
**Status:** Schema ready (single-venue mode), INSERT threading deferred to Plan 303-02
**Requirement:** VENUE-04

---

## 1. Current State (Phase 303)

All 44 major operational tables now have:

```sql
venue_id TEXT NOT NULL DEFAULT 'racingpoint-hyd-001'
```

Tables already having `venue_id` before this phase (excluded from ALTER block):

| Table | Source |
|-------|--------|
| `model_evaluations` | Phase 301 CREATE TABLE |
| `metrics_rollups` | Phase 301 ALTER migration |
| `fleet_solutions` | `fleet_kb.rs` CREATE TABLE |

**Behavioral impact:** Zero. All queries run identically. The column exists in schema but is not yet
passed explicitly in INSERT statements — existing rows read the DEFAULT value transparently via SQLite's
schema metadata (no per-row backfill needed, SQLite 3.37+).

**Access path:** `state.config.venue.venue_id` (added to `VenueConfig`, serde default = `"racingpoint-hyd-001"`).

---

## 2. Trigger Conditions for Venue 2

### Business Triggers
- Second physical location confirmed by Uday
- Revenue justification: marginal cost of server/software per new venue is near zero
- Pod count exceeds 8 at current venue (logistics trigger, not technical)

### Technical Triggers
- New `racecontrol.toml` with `[venue] venue_id = "racingpoint-xxx-002"`
- Separate SQLite DB file (each venue is sovereign — no shared DB)
- Separate rc-agent fleet with separate network segment

### Operational Triggers
- New rc-agent fleet provisioned via pendrive install (existing pendrive deploy kit)
- Cloud sync target remains the same Bono VPS — cloud aggregates across venues
- Driver identity is phone-based; existing drivers can check in at either venue

---

## 3. Schema Strategy

**Each venue runs its own racecontrol binary with its own SQLite DB.**

```
Venue 1 (HYD-001)            Venue 2 (XXX-002)
 racecontrol.exe               racecontrol.exe
 racecontrol.db                racecontrol.db
 8 pods, LAN .23               N pods, LAN ???
       │                             │
       └──────────┬──────────────────┘
                  │ cloud sync (30s push/pull)
            Bono VPS (cloud)
            racecontrol.db  ← all rows carry venue_id
```

**Tables NOT scoped by venue_id** (deployment-scoped, not data-scoped):

| Table | Reason |
|-------|--------|
| `settings` | Global key-value config per deployment |
| `pricing_tiers`, `billing_rates` | Venue-local pricing config |
| `staff_members` | Staff belong to one venue by deployment |
| `pods` | Pods belong to one venue by deployment |
| `feature_flags` | Runtime flags per deployment |
| `policy_rules` | Automation rules per deployment |
| `game_presets` | Game config per deployment |
| `audit_log` | System audit — cross-entity by design |

---

## 4. Sync Model

`cloud_sync.rs` already supports `venue_id` in push/pull payloads (Phase 301 SYNC-03).

| Direction | Tables | Authority | Rule |
|-----------|--------|-----------|------|
| Venue → Cloud (push) | billing_sessions, laps, sessions, events | Venue | Append-only financial data |
| Cloud → Venue (pull) | drivers | Cloud | Driver profiles follow phone identity |
| Bidirectional | metrics_rollups, model_evaluations | LWW | max-sample-count wins |

**Conflict resolution:** Last-Write-Wins (LWW) with `updated_at` timestamp. `venue_id` acts as tiebreaker
for same-timestamp conflicts — venue 1 rows and venue 2 rows never collide (different `venue_id` values).

**No cross-venue DB joins at the venue level.** Cross-venue queries (e.g., "total revenue across all venues")
run on the Bono VPS cloud DB, never at the venue.

---

## 5. Breaking Points (What Needs Code Changes for Venue 2)

| Area | Status | Change Needed |
|------|--------|---------------|
| Driver identity | Ready | Phone-based lookup works cross-venue |
| Wallet balances | Design decision needed | Venue-scoped (default) vs cross-venue wallet |
| Leaderboard | Needs work | track_records/personal_bests need cross-venue aggregation on cloud |
| Pricing | Ready | billing_rates are deployment-scoped, no cross-venue pricing conflict |
| Staff auth | Ready | staff_members are deployment-scoped |
| Admin dashboard | Needs work | Shows single-venue data; cross-venue dashboard is a new feature |
| INSERTs (routes.rs) | Plan 303-02 | ~80-100 INSERT statements need venue_id bound explicitly |

---

## 6. Migration Checklist (for the day Venue 2 launches)

- [ ] Deploy racecontrol with `[venue] venue_id = "racingpoint-xxx-002"` in new `racecontrol.toml`
- [ ] Provision fresh SQLite DB (schema auto-migrates on first start)
- [ ] Cloud sync pulls existing driver profiles via phone-based identity (cross-venue guest drivers)
- [ ] Verify wallet isolation: topup at Venue 1 does NOT appear at Venue 2
- [ ] Verify leaderboard isolation: track records are per-venue (or explicit cross-venue query)
- [ ] Run `cargo test -p racecontrol-crate -- venue_id` on new deployment to confirm schema
- [ ] Update MEMORY.md: add Venue 2 to network map, deploy targets list
- [ ] Cross-venue dashboard (if needed): query Bono VPS cloud DB filtered by venue_id

---

## 7. Implementation History

| Phase | Change |
|-------|--------|
| 301 | `venue_id` added to `model_evaluations`, `metrics_rollups`, `fleet_solutions` |
| 303-01 | `venue_id TEXT NOT NULL DEFAULT 'racingpoint-hyd-001'` on all 44 major tables |
| 303-02 | All INSERT statements pass `venue_id` explicitly via `state.config.venue.venue_id` |
