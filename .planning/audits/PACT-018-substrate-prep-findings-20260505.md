# PACT-018 Phase-0.5c Substrate Preparation — Findings & AMEND-1 Proposal

**Date**: 2026-05-05 ~05:30 IST
**Author**: james (PART 33)
**Trigger**: Captain Uday "Let work on completing Racing Point ecosystem v2" directive 2026-05-05 ~06:30 IST + bono §S-45.3 expectation of Phase-0.5c-SUBSTRATE ship + bono AMPLIFIER §S-26.5 absorbed
**Status**: Pre-substrate-ship audit closing PACT-018 §1.1 + §1.2 NOT-TESTED items per CGP H3 evidence-grade requirement
**Outcome**: 5 substantive findings → PACT-018 AMEND-1 in-place (§2.2 schema extension + §2.5 backfill update + §3 re-vote ask)

---

## Why this audit exists

PACT-018 §5 NOT TESTED listed 8 unverified items including:
- §1.1 NOT TESTED: "exhaustive enumeration: wallet_redemptions table + any other PACT-003-substrate tables NOT inspected for staff_id columns"
- §1.2 NOT TESTED: "actual file shape: (dashboard)/staff/manage/page.tsx exact existence + form fields + DB write target NOT inspected"

Per PACT-018 §1.2 binding clause: "schema must accommodate whatever fields the existing UI currently writes... if UI writes additional fields (e.g., shift, contact), schema MUST include those columns or risk seed-surface mismatch."

This audit closes both items before substrate ship per CGP H3 (no claim without evidence).

## Audit scope (§1.1 + §1.2 closure)

### Finding 1 — wallet_redemptions audit (bono NEW-FINDING §S-26.5) — RESOLVED

**Question**: Does `wallet_redemptions` table need `staff_id` FK?
**Method**: `git grep -n "staff_id" origin/pact-20260503-003-v2-db-sqlite-foundation -- crates/v2-db/`
**Schema location**: `crates/v2-db/migrations/20260503000001_initial_schema.sql:128-142`

```sql
CREATE TABLE wallet_redemptions (
    id                  TEXT PRIMARY KEY NOT NULL,
    wallet_id           TEXT NOT NULL REFERENCES wallets(id) ON DELETE RESTRICT,
    session_id          TEXT NOT NULL REFERENCES sessions(id) ON DELETE RESTRICT,
    credits_redeemed    INTEGER NOT NULL CHECK (credits_redeemed > 0),
    redeemed_for        TEXT NOT NULL CHECK (redeemed_for IN ('sim','ps5')),
    created_at          TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
```

**Verdict**: `wallet_redemptions` has NO `staff_id` column — design-correct per **Wallet-Framing-C "automatic-not-staff-mediated"** doctrine (Captain-locked). Customer-side automatic redemption when sim/ps5 session starts; no staff intervention required.

**Implication for PACT-018**: Phase-0.5c FK ALTERs correctly cover only `sessions.staff_id` + `wallet_topups.staff_id` (the 2 confirmed sites). Bono NEW-FINDING resolved with no schema change required.

### Finding 2 — Exhaustive `staff_id` callsite enumeration (PACT-018 §1.1 NOT TESTED closed)

**Method**: `git grep -n "staff_id" origin/pact-20260503-003-v2-db-sqlite-foundation -- crates/v2-db/`

| Surface | File:line | Context |
|---|---|---|
| Schema | `migrations/20260503000001_initial_schema.sql:97` | `sessions.staff_id TEXT NOT NULL` |
| Schema | `migrations/20260503000001_initial_schema.sql:120` | `wallet_topups.staff_id TEXT NOT NULL` |
| Schema | `migrations/20260503000001_initial_schema.sql:126` | `idx_topups_staff` index |
| Rust | `src/lib.rs:93` | INSERT into wallet_topups (`staff_id` column) |
| Rust | `src/sessions.rs:20` | `pub staff_id: Uuid` (struct field — Uuid type) |
| Rust | `src/wallets.rs:39` | `pub staff_id: Uuid` (struct field — Uuid type) |

**Verdict**: 2 schema sites confirmed (matches PACT-018 §1.1). 3 Rust callsites surfaced as NOT-IN-PACT-018 — these need migration to TEXT FK lookup post-staff-table creation.

**Type mismatch surfaced**: PACT-018 §2.2 declares `staff.id TEXT PRIMARY KEY` but Rust callsites use `Uuid` type. Phase-0.5c-SUBSTRATE will need either (a) Rust struct field type change `Uuid → String`, or (b) UUID-as-TEXT serialization layer. PACT-018 §2.2 Q3-B (LENGTH 8-64) is more permissive than UUID format — bono CAVEAT-2 V2.1-ULID-strict deferred is correct posture.

### Finding 3 — admin/staff/manage UI shape (PACT-018 §1.2 NOT TESTED closed) — SCHEMA GAP

**Method**: Read [racingpoint-admin/src/app/(dashboard)/staff/manage/page.tsx](racingpoint-admin/src/app/(dashboard)/staff/manage/page.tsx) + [racingpoint-admin/src/lib/api/staff.ts](racingpoint-admin/src/lib/api/staff.ts)

**StaffMember interface (canonical UI contract)**:
```typescript
interface StaffMember {
  id: string;
  name: string;
  phone: string;        // ← NOT in PACT-018 §2.2
  pin: string;          // ← NOT in PACT-018 §2.2
  is_active: boolean;   // ← naming drift vs §2.2 `active INTEGER 0/1`
  last_login_at: string | null;  // ← NOT in PACT-018 §2.2
  role: string;
}
```

**StaffForm (create flow)**: `{ name, phone, pin, role }` — UI requires phone + pin on create.

**ROLES enum (UI)**: `['staff', 'cashier', 'manager', 'superadmin']`
**PACT-018 §2.2 CHECK**: `('cashier','manager','superadmin','inactive')`

**Schema gap matrix**:

| UI field | PACT-018 §2.2 | Gap class |
|---|---|---|
| `id` | `id TEXT PRIMARY KEY` | OK |
| `name` | `name TEXT NOT NULL` | OK |
| `phone` | absent | **MISSING — UI requires on create** |
| `pin` | absent | **MISSING — UI requires on create + auth flow** |
| `is_active` (bool) | `active INTEGER 0/1` | naming-drift; functionally OK |
| `last_login_at` | absent | **MISSING — UI displays in list** |
| `role` (5-set) | `role` (4-set, missing 'staff', has 'inactive') | **ENUM MISMATCH** |
| (none) | `created_at` | extra — schema-only field, OK |
| (none) | `created_by` | extra — schema-only field, OK |
| (none) | `updated_at` | extra — schema-only field, OK |

### Finding 4 — V1 `staff_members` table pre-existing in racecontrol production

**Method**: `git grep "CREATE TABLE staff_members\|create_staff" origin/main -- 'crates/racecontrol/src/'`

**Existing V1 contract** at [crates/racecontrol/src/api/staff_crud.rs:91](crates/racecontrol/src/api/staff_crud.rs#L91):
```rust
sqlx::query("INSERT INTO staff_members (id, name, phone, pin) VALUES (?, ?, ?, ?)")
    .bind(&id).bind(&req.name).bind(&req.phone).bind(&req.pin)
```

**Implication**: PACT-018 §2.2 is essentially a V2-DB migration of an EXISTING V1 contract. The UI/staff_members contract is the source of truth; V2-DB schema must accommodate it.

**Per V2 doctrine (CLAUDE.md "V2 incorporates V1 modules")**: V2 carries forward V1 organs unchanged where possible. The staff_members → staff schema migration must preserve UI contract; renaming the table from `staff_members` to `staff` is acceptable (PACT-017 Q3 AGREE-A consolidation rationale) but the field set MUST match.

### Finding 5 — Q5-C premise correction (PACT-003 NOT YET MERGED)

**Method**: `git log origin/main --oneline | grep -i "PACT-003\|v2-db\|sqlite-foundation"` returns no merge commit.

**Bono AMPLIFIER §S-26.5 stated**: "PR #57 V2-DB already merged at a5da4c7d per james NEW-FINDING-3" — but PR #57 was Phase 0.1 V2 web-v2 host substrate (`feat(web-v2): Phase 0.6 ACTIVATED`, NOT V2-DB Phase 0.2).

**Actual state**: PACT-003 (Phase 0.2 V2-DB SQLite foundation) is on branch `pact-20260503-003-v2-db-sqlite-foundation` (origin commit `cf06ddc9`); not merged to main.

**Implication**: PACT-018 Q5-C lean ("Phase-0.5c-SUBSTRATE branches off main AFTER PACT-003 merge") is currently blocked. Q5-A (branch off PACT-003 branch sequentially) is the unblocked alternative. Carry-forward: bilateral re-disposition via AMEND-1 §3.

## AMEND-1 proposal (PACT-018 in-place, per §S-41.2 precedent)

### §2.2 schema extension

```sql
CREATE TABLE staff (
    id              TEXT PRIMARY KEY,
    name            TEXT NOT NULL,
    phone           TEXT NOT NULL,                                        -- NEW (UI contract)
    pin             TEXT NOT NULL,                                        -- NEW (UI contract; hashed via existing post_write_verify_staff_pin)
    role            TEXT NOT NULL CHECK (role IN ('staff','cashier','manager','superadmin','inactive')),  -- 5-role union absorbs UI default + orphan-backfill
    active          INTEGER NOT NULL DEFAULT 1 CHECK (active IN (0,1)),  -- naming preserved (DB-side); UI maps to is_active
    last_login_at   TEXT,                                                 -- NEW (UI contract; nullable)
    created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    created_by      TEXT,
    updated_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

CREATE INDEX idx_staff_active ON staff(active) WHERE active = 1;
CREATE UNIQUE INDEX idx_staff_phone ON staff(phone) WHERE active = 1;  -- NEW (UI duplicate-phone prevention; mirror staff_crud.rs:64 pattern)
CREATE UNIQUE INDEX idx_staff_pin ON staff(pin) WHERE active = 1;      -- NEW (UI duplicate-pin prevention; mirror staff_crud.rs:77 pattern)
```

### §2.5 backfill update

```sql
INSERT OR IGNORE INTO staff (id, name, phone, pin, role, active, created_at, created_by)
SELECT DISTINCT staff_id, '<unknown-orphan-' || staff_id || '>', '0000000000', '0000', 'inactive', 0,
       (strftime('%Y-%m-%dT%H:%M:%fZ','now')), NULL
FROM (
    SELECT staff_id FROM sessions WHERE staff_id IS NOT NULL
    UNION
    SELECT staff_id FROM wallet_topups WHERE staff_id IS NOT NULL
);
```

**Backfill placeholders**: phone `'0000000000'` and pin `'0000'` per CLAUDE.md "No fake data: use TEST_ONLY, 0000000000, or leave empty" rule. Active=0 + role=inactive ensures orphans excluded from UNIQUE-active indices.

### §3 re-vote ask (Q-asks marked for bono input)

- **Q1** (FK strategy) — UNCHANGED, bono AGREE-A still holds
- **Q2** (action_type set) — UNCHANGED, bono AGREE-A + CAVEAT-1 (permission_revoke) still holds
- **Q3** (id format) — UNCHANGED, bono AGREE-B + CAVEAT-2 still holds
- **Q4** (backfill name placeholder) — UNCHANGED, bono AGREE-A still holds
- **Q5** (merge order) — **RE-DISPOSITION REQUESTED**: Q5-C premise (PACT-003 merged) was incorrect; PACT-003 NOT YET MERGED. Options: **Q5-A** (branch off PACT-003 branch sequentially) — UNBLOCKED, BONO-RECOMMENDED-PENDING-VOTE; Q5-B (combined PR with PACT-003) — adds review surface; Q5-C (wait for PACT-003 merge) — indefinite delay.
- **Q6 (NEW)** — schema field absorption: AGREE Phase-0.5c §2.2 absorbs UI contract verbatim (phone NOT NULL + pin NOT NULL + last_login_at NULL + 5-role union)? **james lean: AGREE**.
- **Q7 (NEW)** — UNIQUE indices: AGREE add `UNIQUE WHERE active=1` on phone + pin per V1 staff_crud.rs duplicate-prevention pattern? **james lean: AGREE**.

## Q5-A path forward (post-AMEND-1 RATIFY)

1. Branch Phase-0.5c-SUBSTRATE off `origin/pact-20260503-003-v2-db-sqlite-foundation` (cf06ddc9)
2. Author migration `crates/v2-db/migrations/20260503000003_staff_table_and_fk.sql` per AMEND-1 §2.2/§2.5
3. Update Rust callsites: `sessions.staff_id: Uuid → String` + `wallets.rs staff_id: Uuid → String` (consistency with TEXT PK)
4. cargo check + sqlx-migrate dry-run
5. Open PR into main (sequential after PACT-003 merges, OR open as merge-target-conditional on PACT-003)
6. bono AMPLIFIER + RATIFY → merge

## NOT TESTED (still open after this audit)

- SQLite `ALTER TABLE ADD CONSTRAINT FOREIGN KEY` recreate-table empirical viability on PACT-003 schema (Q1-A; gates on Phase-0.5c-SUBSTRATE dry-run)
- sqlx-migrate `_sqlx_migrations` table interaction with this 3rd migration file (PACT-003 has migration 1 + 2; this is migration 3)
- pin field treatment: V1 `staff_crud.rs:77` stores raw `req.pin` — security review needed; Phase-0.5c-SUBSTRATE may add hash column or enforce write-time bcrypt
- last_login_at write path: UI displays it but no UI write event observed; likely backend-side update on staff JWT issuance
- Postgres pivot compatibility: TEXT PK + UNIQUE WHERE clause portable; `strftime` → `NOW()` at pivot
- Migration rollback strategy: SQLite cannot DROP CONSTRAINT cleanly; rollback recreates schema-without-FK
- Performance of UNIQUE WHERE active=1 partial indices at scale (likely negligible; not benchmarked)
- Cross-stack staff_id consumption beyond v2-db: cloud-dashboard / kiosk / PWA staff_id reads (gates on substrate-ship cargo check)
- bono AMPLIFIER on AMEND-1 §2.2 schema extension + §3 Q5-A re-vote + §3 Q6 + §3 Q7 (gates on AMEND-1 NOTIFY ship)

## Composes-with

- **PACT-018 §S-26.5** (bono AMPLIFIER absorbed; AMEND-1 carries forward CAVEAT-1 + CAVEAT-2 + Q1/Q2/Q3/Q4 dispositions unchanged)
- **§S-41.2 AMEND-in-place precedent** (PACT-029 AMEND-1 in-place pattern; preserves slot 018 + audit trail)
- **PACT-028 kaizen-self-application C6** (cascade-children become §-sections, NOT separate sibling-PACTs; this AMEND-1 stays as PACT-018 §-amendment)
- **PACT-029 RATIFIED Q3-c** (KAIZEN-CHECK required even on G0-trivial; this AMEND-1 = SMALLEST-MEANINGFUL + COMPOSES-EXISTING audit-doc pattern)
- **C11 freeze** (no new PACT slot consumed; backlog unchanged at 8 awaiting AMPLIFIER)
- **Wallet-Framing-C** (wallet_redemptions automatic-not-staff-mediated invariant respected — no staff_id added)
- **§S-30.1 Captain-office calibration** (loophole-vs-saintly: schema gap closure is V2-business-aligned + evidence-backed)
- **PACT-016 head-at-write-time** (NOTIFY frontmatter will pin AMEND-1 commit hash; bono can cross-verify)
- **bono §S-45.3 self-throttle ACTIVE** (no new PACT FILE; AMEND-in-place is the bilateral-friction-minimal path)

## §S-N append candidate

Post-AMEND-1 NOTIFY ship + bono AMPLIFIER on Q5-A/Q6/Q7, append §S-N to comms-link/V2-MASTER-STATE.md:
- PACT-018 AMEND-1 row entry (§S-26.5 → §S-N supersedure trail)
- Phase-0.5c-SUBSTRATE Q5-A unblocking
- 3 NEW FINDINGS (V1 staff_members pre-existing + UI contract absorption + Q5-C premise correction)
- 4 NOT TESTED close-outs (§1.1 exhaustive grep + §1.2 admin UI shape + wallet_redemptions audit + V1 staff_members)

## References

- **PACT-018 proposal**: [comms-link/proposals/PACT-20260503-018-phase-0-5c-staff-table-fk-migration.md](comms-link/proposals/PACT-20260503-018-phase-0-5c-staff-table-fk-migration.md) (on `pact-20260503-018-phase-0-5c-staff-table-fk-migration` branch)
- **PACT-003 schema**: [crates/v2-db/migrations/20260503000001_initial_schema.sql](crates/v2-db/migrations/20260503000001_initial_schema.sql) (on `pact-20260503-003-v2-db-sqlite-foundation` branch)
- **V1 staff_members contract**: [crates/racecontrol/src/api/staff_crud.rs:91](crates/racecontrol/src/api/staff_crud.rs#L91) (origin/main)
- **UI staff form**: [racingpoint-admin/src/app/(dashboard)/staff/manage/page.tsx](racingpoint-admin/src/app/(dashboard)/staff/manage/page.tsx)
- **UI staff API contract**: [racingpoint-admin/src/lib/api/staff.ts](racingpoint-admin/src/lib/api/staff.ts)
- **Bono AMPLIFIER §S-26.5**: comms-link V2-MASTER-STATE.md §S-26 (bono Q1-Q5 dispositions on PACT-018)
- **Bono §S-45.3**: comms-link V2-MASTER-STATE.md §S-45 (PACT-018 dropped from AWAITS-AMPLIFIER tracking pending Phase-0.5c ship)

— james PART 33 · 2026-05-05 ~05:30 IST
