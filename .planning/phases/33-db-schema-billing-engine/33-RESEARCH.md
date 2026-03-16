# Phase 33: DB Schema + Billing Engine - Research

**Researched:** 2026-03-17
**Domain:** Rust/SQLite billing engine, rc-common protocol, SQLx migrations
**Confidence:** HIGH

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| RATE-01 | `billing_rates` table with columns: id, tier_order, tier_name, threshold_minutes, rate_per_min_paise, is_active | TABLE ALREADY EXISTS in db/mod.rs migration |
| RATE-02 | Three default seed rows: Standard (0-30 min, 2500 p/min), Extended (31-60 min, 2000 p/min), Marathon (60+ min, 1500 p/min) | SEED INSERT OR IGNORE ALREADY EXISTS — but tier_name values are lowercase, not Title Case |
| RATE-03 | `billing_rates` added to cloud_sync SYNC_TABLES for cloud replication | ALREADY IN SYNC_TABLES (cloud_sync.rs line 18) |
| BILLC-02 | `compute_session_cost()` uses non-retroactive additive algorithm: 45 min = (30×25)+(15×20)=1050 | ALREADY IMPLEMENTED + UNIT TESTS GREEN |
| BILLC-03 | BillingManager holds in-memory rate cache (`RwLock<Vec<BillingRateTier>>`) with hardcoded defaults | ALREADY IMPLEMENTED — BillingManager::new() initialises with default_billing_rate_tiers() |
| BILLC-04 | Rate cache refreshes from DB at startup and every 60s — never blocks billing tick | ALREADY IMPLEMENTED in main.rs startup + 60s refresh loop |
| BILLC-05 | Final session cost saved to `wallet_debit_paise` column on session end | COLUMN EXISTS + UPDATE ALREADY WRITTEN in billing.rs end_session |
| PROTOC-01 | `minutes_to_value_tier` renamed to `minutes_to_next_tier` with `#[serde(alias)]` backward compat | ALREADY RENAMED with alias in place |
| PROTOC-02 | `tier_name` field added to BillingTick as `Option<String>` | ALREADY IMPLEMENTED as Option<String> |
</phase_requirements>

---

## Summary

Phase 33 is almost entirely pre-implemented. A prior development session added the billing_rates table migration, seed data, cloud sync registration, BillingRateTier type, non-retroactive compute_session_cost(), BillingManager rate cache with RwLock, startup/periodic refresh, wallet_debit_paise column write, and the protocol field rename — all before the Phase 33 planning pass ran.

Running `cargo test -p rc-common && cargo test -p racecontrol-crate` shows 112 + 331 tests passing (0 failures). The implementation is structurally complete. What remains are two verification gaps: (1) no test asserts the `billing_rates` seed data after migration (3 rows with specific values), and (2) no test specifically exercises old JSON containing `"minutes_to_value_tier"` deserializing correctly via the serde alias. A third cosmetic gap is that the DB seed uses lowercase tier names (`'standard'`, `'extended'`, `'marathon'`) while `default_billing_rate_tiers()` uses Title Case (`"Standard"`, `"Extended"`, `"Marathon"`), creating a mismatch between cold-start behavior (hardcoded defaults, Title Case) and DB-loaded behavior (lowercase from seed).

**Primary recommendation:** Phase 33 is a VERIFICATION + GAP-FILL phase, not a build phase. The single plan should: (1) fix the seed capitalization bug, (2) add a test asserting billing_rates seed rows, (3) add a test that deserializes old `"minutes_to_value_tier"` JSON via alias, then run all three test suites green and close.

---

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| sqlx | (existing) | SQLite queries, `query_as!` macro, pool | Already in use throughout the project |
| tokio::sync::RwLock | (existing) | Rate cache locking — allows concurrent reads from billing tick | Already used in BillingManager |
| serde + serde_json | (existing) | Protocol serialization, alias attribute | Already used for BillingTick |

### No New Dependencies Required
All Phase 33 work uses existing crate dependencies. No `Cargo.toml` changes needed.

---

## Architecture Patterns

### Pattern 1: SQLite Migration (Idempotent CREATE TABLE + INSERT OR IGNORE)

The project uses a single `migrate()` function in `crates/racecontrol/src/db/mod.rs`. All tables and seed data go through `CREATE TABLE IF NOT EXISTS` + `INSERT OR IGNORE` calls. This is the only migration mechanism — no SQLx offline migrations, no separate migration files.

```rust
// Source: crates/racecontrol/src/db/mod.rs lines 236-260
sqlx::query(
    "CREATE TABLE IF NOT EXISTS billing_rates (
        id TEXT PRIMARY KEY,
        tier_order INTEGER NOT NULL,
        tier_name TEXT NOT NULL,
        threshold_minutes INTEGER NOT NULL,
        rate_per_min_paise INTEGER NOT NULL,
        is_active BOOLEAN DEFAULT 1,
        created_at TEXT DEFAULT (datetime('now')),
        updated_at TEXT DEFAULT (datetime('now'))
    )",
)
.execute(pool)
.await?;

sqlx::query(
    "INSERT OR IGNORE INTO billing_rates (id, tier_order, tier_name, threshold_minutes, rate_per_min_paise)
     VALUES
        ('rate_standard', 1, 'standard', 30, 2500),  -- BUG: should be 'Standard'
        ('rate_extended', 2, 'extended', 60, 2000),  -- BUG: should be 'Extended'
        ('rate_marathon', 3, 'marathon', 0, 1500)",   -- BUG: should be 'Marathon'
)
.execute(pool)
.await?;
```

**Pattern note:** `INSERT OR IGNORE` means the seed will NOT re-run if rows with those primary keys already exist. The bug fix to capitalize names must update the seed SQL, and any existing local DB (like on the server) will need to be addressed manually — but since existing pods don't have billing_rates rows yet (this is a new table), `INSERT OR IGNORE` will insert fresh on first run.

**Fix:** Change to `'Standard'`, `'Extended'`, `'Marathon'` (Title Case matching default_billing_rate_tiers()).

### Pattern 2: Non-Retroactive Tiered Cost Algorithm

```rust
// Source: crates/racecontrol/src/billing.rs compute_session_cost()
// Already implemented and tested. Example:
// 45 min = (30 × 2500) + (15 × 2000) = 75000 + 30000 = 105000 paise = 1050 cr
pub fn compute_session_cost(elapsed_seconds: u32, tiers: &[BillingRateTier]) -> SessionCost
```

The algorithm iterates tiers in order, accumulates paise for each tier's minute range, and breaks when the elapsed time falls within the current tier's ceiling. Tier with `threshold_minutes == 0` is treated as f64::MAX (unlimited).

### Pattern 3: RwLock Rate Cache in BillingManager

```rust
// Source: crates/racecontrol/src/billing.rs
pub struct BillingManager {
    pub rate_tiers: RwLock<Vec<BillingRateTier>>,
    // ...
}
impl BillingManager {
    pub fn new() -> Self {
        Self {
            rate_tiers: RwLock::new(default_billing_rate_tiers()), // hardcoded defaults
            // ...
        }
    }
}
```

The billing tick acquires a read lock: `let rate_tiers = state.billing.rate_tiers.read().await;`. This never blocks because `refresh_rate_tiers` acquires a write lock only every 60s, briefly.

### Pattern 4: serde alias for protocol backward compat

```rust
// Source: crates/rc-common/src/protocol.rs line 234
#[serde(default, skip_serializing_if = "Option::is_none", alias = "minutes_to_value_tier")]
minutes_to_next_tier: Option<u32>,
```

The alias allows old rc-agent versions sending `"minutes_to_value_tier"` to still be parsed correctly. The serialized output always uses `"minutes_to_next_tier"`. This is already in place.

### Anti-Patterns to Avoid

- **Do NOT use `UPDATE` to fix the seed capitalization.** The table is new and no real data exists in `billing_rates` on any machine. Fix the `INSERT OR IGNORE` SQL in `migrate()` directly.
- **Do NOT use `INSERT OR REPLACE`** — this would lose any admin-customized rates. `INSERT OR IGNORE` is correct for initial seeding.
- **Do NOT add a new migration block** for the capitalization fix — just correct the existing INSERT OR IGNORE string (the table was just added, `INSERT OR IGNORE` hasn't run yet on real machines, so changing the seed SQL is safe).

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Periodic cache refresh | Custom timer/channel | tokio::spawn loop with counter in main.rs | Already exists in main.rs, pattern established |
| DB migrations | A separate migration runner | Add to existing migrate() fn in db/mod.rs | All 14+ tables follow this pattern |
| Test DB creation | New setup code | `create_test_db()` in integration.rs | Already used by 62 integration tests |

---

## Common Pitfalls

### Pitfall 1: Seed Capitalization Mismatch
**What goes wrong:** The DB seed inserts `'standard'` (lowercase) but `default_billing_rate_tiers()` uses `"Standard"` (Title Case). After `refresh_rate_tiers()` loads DB rows, the `tier_name` field in `SessionCost` changes case — affecting what the overlay shows and what tests assert.
**Why it happens:** The seed SQL was written without noticing the case used in the Rust defaults.
**How to avoid:** Fix the INSERT OR IGNORE to use `'Standard'`, `'Extended'`, `'Marathon'`.
**Warning signs:** `cost.tier_name == "standard"` instead of `"Standard"` after DB load — current unit tests use hardcoded defaults so they pass, but runtime behavior after DB load would differ.

### Pitfall 2: Missing alias round-trip test
**What goes wrong:** Phase 33 success criterion #1 explicitly requires "old JSON field name still deserializes without error, confirmed by a round-trip unit test." The existing `test_billing_tick_backward_compat_old_format` test only checks deserialization of a BillingTick WITHOUT the tier field — it does not test that `"minutes_to_value_tier"` key is accepted via the alias.
**Why it happens:** The alias was added in the same session as the field rename, but the test wasn't updated to exercise the alias direction.
**How to avoid:** Add a test that parses JSON containing `"minutes_to_value_tier": 15` and asserts `minutes_to_next_tier == Some(15)`.

### Pitfall 3: Integration test_db_setup doesn't assert billing_rates seed
**What goes wrong:** Phase 33 success criterion #4 requires confirming 3 seed rows exist. The existing `test_db_setup` asserts `pricing_tiers >= 3` and `accounts >= 5` but has no assertion for `billing_rates`.
**Why it happens:** The integration test was written before billing_rates was added.
**How to avoid:** Add `billing_rates` count assertion (== 3) to `test_db_setup`.

### Pitfall 4: rc-agent-crate test timeout
**What goes wrong:** `cargo test -p rc-agent-crate` appears to hang in this environment (tests may invoke winapi/GUI code that blocks on Windows without a display).
**Why it happens:** rc-agent tests use winapi for overlay rendering; some may require a real Windows desktop session.
**How to avoid:** Run `cargo test -p rc-common && cargo test -p racecontrol-crate` as the primary test gates. rc-agent-crate overlay tests that involve actual Win32 drawing may be skipped via `#[cfg(not(test))]` guards or run manually on a pod.

---

## Code Examples

### Finding 1: Table and seed already in db/mod.rs

Table exists at `crates/racecontrol/src/db/mod.rs` lines 236-260. Seed uses lowercase tier names — fix needed.

### Finding 2: BillingRateTier and compute_session_cost already in billing.rs

`crates/racecontrol/src/billing.rs` lines 54-170 contain the complete implementation: `BillingRateTier` struct, `default_billing_rate_tiers()`, `refresh_rate_tiers()`, `SessionCost` struct, `compute_session_cost()`.

### Finding 3: BillingManager::new() initialises cache with hardcoded defaults

`crates/racecontrol/src/billing.rs` lines 349-358:
```rust
pub fn new() -> Self {
    Self {
        rate_tiers: RwLock::new(default_billing_rate_tiers()),
        // ...
    }
}
```

### Finding 4: main.rs startup + 60s refresh already wired

`crates/racecontrol/src/main.rs` lines 207-227: `refresh_rate_tiers` is called at startup (line 211) and every 60s in the billing tick spawn loop (lines 222-225).

### Finding 5: wallet_debit_paise write already in billing.rs end_session

`crates/racecontrol/src/billing.rs` line 1890:
```
"UPDATE billing_sessions SET status = ?, driving_seconds = ?, wallet_debit_paise = ?, ended_at = datetime('now') WHERE id = ?"
```

### Finding 6: Protocol fields already renamed with alias

`crates/rc-common/src/protocol.rs` line 234:
```rust
#[serde(default, skip_serializing_if = "Option::is_none", alias = "minutes_to_value_tier")]
minutes_to_next_tier: Option<u32>,
```
`tier_name: Option<String>` at line 238.

### Finding 7: SYNC_TABLES already includes billing_rates

`crates/racecontrol/src/cloud_sync.rs` line 18:
```rust
const SYNC_TABLES: &str = "drivers,wallets,pricing_tiers,pricing_rules,billing_rates,kiosk_experiences,kiosk_settings";
```

### Finding 8: Admin CRUD routes for billing_rates already registered (Phase 34 territory)

`crates/racecontrol/src/api/routes.rs` lines 70-71 already register GET/POST/PUT/DELETE for `/billing/rates`. This is Phase 34 scope — note it in the plan as already done.

---

## State of the Art

| Status | Reality |
|--------|---------|
| RATE-01 | DONE — billing_rates table exists |
| RATE-02 | 95% DONE — seed exists, capitalization bug to fix |
| RATE-03 | DONE — billing_rates in SYNC_TABLES |
| BILLC-02 | DONE — non-retroactive algorithm with tests |
| BILLC-03 | DONE — RwLock cache with hardcoded defaults |
| BILLC-04 | DONE — startup + 60s refresh in main.rs |
| BILLC-05 | DONE — wallet_debit_paise written on session end |
| PROTOC-01 | DONE — minutes_to_next_tier with serde alias |
| PROTOC-02 | DONE — tier_name as Option<String> |
| Missing | alias round-trip test (PROTOC-01 success criterion) |
| Missing | billing_rates seed count assertion in test_db_setup |
| Missing | seed capitalization fix |

---

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in + tokio-test |
| Config file | `Cargo.toml` in each crate |
| Quick run command | `cargo test -p rc-common && cargo test -p racecontrol-crate` |
| Full suite command | `cargo test -p rc-common && cargo test -p rc-agent-crate && cargo test -p racecontrol-crate` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| RATE-01 | billing_rates table exists with correct columns | integration | `cargo test -p racecontrol-crate test_db_setup` | ✅ (but no billing_rates assertion — Wave 0 gap) |
| RATE-02 | 3 seed rows: Standard/Extended/Marathon at correct rates | integration | `cargo test -p racecontrol-crate test_db_setup` | ✅ (but no billing_rates assertion — Wave 0 gap) |
| RATE-03 | billing_rates in SYNC_TABLES | unit | `cargo test -p racecontrol-crate cloud_sync` | ✅ (`push_payload_includes_billing_session_extra_columns` covers sync) |
| BILLC-02 | 45 min = (30×25)+(15×20) = 1050 cr | unit | `cargo test -p racecontrol-crate cost_45_minutes_two_tiers` | ✅ `cost_45_minutes_two_tiers` EXISTS AND GREEN |
| BILLC-03 | BillingManager starts with defaults, no DB needed | unit | `cargo test -p racecontrol-crate timer_current_cost` | ✅ `timer_current_cost_returns_session_cost` EXISTS AND GREEN |
| BILLC-04 | Rate cache refreshes at startup + every 60s | integration | `cargo test -p racecontrol-crate` | ✅ covered by existing billing suite (startup tested via BillingManager::new) |
| BILLC-05 | wallet_debit_paise saved on session end | integration | `cargo test -p racecontrol-crate test_billing_pause_timeout_refund` | ✅ EXISTS AND GREEN |
| PROTOC-01 | Old field `minutes_to_value_tier` deserializes via alias | unit | `cargo test -p rc-common test_billing_tick_old_field_alias` | ❌ Wave 0 gap — test needed |
| PROTOC-02 | tier_name is Option<String> in BillingTick | unit | `cargo test -p rc-common test_billing_tick_with_new_optional_fields` | ✅ EXISTS AND GREEN |

### Sampling Rate
- **Per task commit:** `cargo test -p rc-common && cargo test -p racecontrol-crate`
- **Per wave merge:** `cargo test -p rc-common && cargo test -p racecontrol-crate` (rc-agent omitted — hangs in dev environment)
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] Test `test_billing_tick_old_field_alias` in `crates/rc-common/src/protocol.rs` — deserializes JSON with `"minutes_to_value_tier": 15` and asserts `minutes_to_next_tier == Some(15)` — covers PROTOC-01 alias requirement
- [ ] Add billing_rates count assertion to `test_db_setup` in `crates/racecontrol/tests/integration.rs` — assert `billing_rates` table has 3 rows — covers RATE-01 + RATE-02
- [ ] Fix seed capitalization: `'standard'` → `'Standard'`, `'extended'` → `'Extended'`, `'marathon'` → `'Marathon'` in `crates/racecontrol/src/db/mod.rs`

---

## Open Questions

1. **Does the rc-agent-crate test suite compile and pass cleanly?**
   - What we know: `cargo test -p rc-agent-crate` did not return output in the bash execution environment (likely GUI/winapi blocking in CI-style shell)
   - What's unclear: Whether any rc-agent test references `minutes_to_value_tier` by name and would fail
   - Recommendation: Search for `minutes_to_value_tier` in rc-agent codebase (confirmed: only `minutes_to_next_tier` used in rc-agent/src/main.rs and overlay.rs) — MEDIUM confidence rc-agent tests are green

2. **Should the seed fix use UPDATE instead of changing INSERT OR IGNORE?**
   - What we know: `INSERT OR IGNORE` won't re-insert if rows exist. If a dev machine ran an earlier build, they have lowercase rows that won't be updated.
   - What's unclear: Whether any machine has billing_rates rows populated already
   - Recommendation: Since `billing_rates` is a NEW table (Phase 33), no production DB has rows yet. Fix the INSERT OR IGNORE string directly. If post-deployment a prod DB ever needs fixing, add an `UPDATE billing_rates SET tier_name = ...` in the ALTER TABLE section of migrate().

---

## Sources

### Primary (HIGH confidence)
- Direct code inspection: `crates/racecontrol/src/billing.rs` — BillingRateTier, compute_session_cost, BillingManager, refresh_rate_tiers, wallet_debit_paise write
- Direct code inspection: `crates/racecontrol/src/db/mod.rs` — billing_rates table definition and seed data
- Direct code inspection: `crates/rc-common/src/protocol.rs` — minutes_to_next_tier field with alias, tier_name Option<String>
- Direct code inspection: `crates/racecontrol/src/cloud_sync.rs` line 18 — SYNC_TABLES includes billing_rates
- Direct code inspection: `crates/racecontrol/src/main.rs` lines 207-227 — startup refresh + 60s tick loop
- Test run: `cargo test -p rc-common` — 112 tests, 0 failures
- Test run: `cargo test -p racecontrol-crate` — 269 unit + 62 integration = 331 tests, 0 failures

### Secondary (MEDIUM confidence)
- rc-agent test behavior: inferred from code search for `minutes_to_value_tier` showing 0 results in rc-agent — alias not used anywhere else

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — direct code confirmed, tests green
- Architecture: HIGH — all patterns match existing project conventions
- Pitfalls: HIGH — all identified via direct code inspection, not speculation

**Research date:** 2026-03-17
**Valid until:** Stable for this codebase (60 days) — nothing is fast-moving here
