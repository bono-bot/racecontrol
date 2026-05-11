# Postgres Pivot Decision Matrix — `v2-db`

PACT-20260503-003 Phase 0.2. Captain D4 (composite-ratify-event #3) locks
**SQLite-first** with a **documented pivot to Postgres** when any of the
trigger conditions below holds. Postgres is NOT a pre-emptive concern.

## Trigger conditions (per Captain D4 — fire on ANY)

| # | Condition | Detection signal | Pivot urgency |
|---|---|---|---|
| T1 | **>2 concurrent writer processes** sustained ≥1 hour | `sqlite3 ... .timeout` saturating; SQLITE_BUSY > 0.1% of writes; `application_name`-keyed write counters in racecontrol metrics | HIGH — switch within 7 days |
| T2 | **Replication-with-failover required** (e.g., HA standby on a second VPS) | Captain or ops directive; uptime SLA ≥ 99.9% | HIGH — switch within 14 days |
| T3 | **>1M-row sustained workload** in any single table | `sqlite_dbstat` row counts; query p99 > 50ms on indexed paths | MEDIUM — plan within 30 days |

T1 is the most likely first-fire as V2 surfaces multiplex (POS + Kiosk +
Admin all writing wallet/session state).

T3 is unlikely in the near term — at 100 sessions/day × 8 pods × 365 days
= ~292K rows/year, plus the 8-year retention requirement = ~2.3M rows on
sessions alone. T3 fires somewhere between year 3 and year 4.

## Schema portability (what changes on pivot)

| SQLite column type | Postgres equivalent | Migration step |
|---|---|---|
| `TEXT PRIMARY KEY` (UUID) | `UUID PRIMARY KEY` | `ALTER COLUMN id TYPE uuid USING id::uuid;` per UUID column |
| `TEXT` (RFC3339 timestamp) | `TIMESTAMPTZ` | `ALTER COLUMN created_at TYPE timestamptz USING created_at::timestamptz;` |
| `INTEGER` (boolean 0/1) | `BOOLEAN` | `ALTER COLUMN registered_at_pwa TYPE boolean USING registered_at_pwa = 1;` |
| `INTEGER` (paise / credits) | `BIGINT` | identity (Postgres `BIGINT` accepts INTEGER literals) |
| `CHECK (... IN ('a','b'))` | `CREATE TYPE ... AS ENUM` | drop CHECK, add enum, `ALTER COLUMN driver_class TYPE driver_class_enum USING driver_class::driver_class_enum;` |
| `strftime('%Y-%m-%dT%H:%M:%fZ','now')` default | `now()` | rewrite DEFAULT clause |
| `WHERE` partial index | identity | identity (Postgres supports partial indexes) |

## Mechanical port (estimated effort)

1. Add `postgres` feature to `v2-db/Cargo.toml`:
   ```toml
   sqlx = { version = "0.8", features = ["runtime-tokio", "sqlite", "postgres", "chrono", "uuid", "macros", "migrate"] }
   ```
2. Author `migrations/<ts>_postgres_port.sql` performing the column-type
   adjustments above (Postgres-only; gated by `runtime DB url scheme`).
3. Swap `SqlitePool` for `AnyPool` in `lib.rs::DbPool` and feature-gate the
   default driver by `database_url.starts_with("postgres://")`.
4. Run `sqlx migrate run` against an empty Postgres instance; replay
   production via `pg_dump`-of-an-export-from-SQLite (see Phase F ETL —
   PACT-015 D5 sibling-PACT cluster).
5. Soak ≥30 days side-by-side via dual-write (Phase F.2 substrate).

Estimated dev effort: **~3 sessions** plus the Phase F dual-write window.

## What is NOT changing on pivot

- Application-layer types (the `Customer`, `Wallet`, `Session` structs).
  `sqlx::FromRow` covers both backends.
- Migration tooling. `sqlx-migrate` per Phase 0.8 / PACT-20260502-004 Phase B
  is portable across SQLite + Postgres.
- The audit-trail invariants (no DELETE on `wallet_topups` /
  `wallet_redemptions`).
- The Wallet Framing C ledger semantics (paise + credits, never floats;
  18% GST at top-up locked).

## Detection automation (deferred to Phase 0.2.1 sibling-PACT)

- A `pg-pivot-monitor` cron probe that emits `pg_pivot_trigger_recommended`
  metrics into the racecontrol fleet-health stream when T1/T2/T3 cross
  threshold. Not authored in this PACT.
- Integration with `feature-flags` (Phase 0.4) so the pivot can be staged.

## Stale-at

This decision matrix stays current until any of:
- A trigger fires and pivot starts (replace this file with a port runbook)
- Captain re-litigates D4 (e.g., direct-to-Postgres mandate)
- sqlx 1.x changes the migration model (revisit `migrations/` layout)
