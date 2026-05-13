# RCA — `detected_at` TZ-mislabel in `content_drift_events` (and sibling sites)

**Class:** §S-146 V1↔V2 RCA · 5-section · foundational boundary (cloud_sync to Bono VPS)
**Author:** james (claude opus 4.7) · 2026-05-13 IST
**Surface:** racecontrol crate · cloud_sync transmit path
**Status:** RCA AUTHORED — code change PENDING Captain per-PR auth · NO source edit applied
**Composes-with:** mechanism-trust check at `.planning/specs/v2/MECHANISM-TRUST/cloud-sync-20260513.json`

---

## Symptom (observed 2026-05-13 ~11:05 IST)

Direct SQLite read of `content_drift_events.detected_at` on Server .23 returned values labeled `+00:00` (UTC) but numerically equal to IST wall time:

```
Server wall:    2026-05-13T05:35:26Z (= 11:05 IST)
Latest row:     2026-05-13T10:45:05.358...+00:00   ← claims UTC, value is IST
                                                      (5h30m in the future if UTC label honored)
First row:      2026-04-11T15:39:31.242...+00:00   ← same anti-pattern, 32 days back
Total rows:     36,199 (avg 1,131/day · 4 active pods of 8)
```

Reading the value at face-value gives a timestamp ~5h30m in the future, breaking every downstream time-window query.

---

## Section 1 — Boundary map

### Write-side (the bug)

**Bug A · `crates/racecontrol/src/content_drift.rs:282-283`**
```rust
let now_ist = (Utc::now() + chrono::Duration::hours(5) + chrono::Duration::minutes(30))
    .to_rfc3339();
```
- Type pipeline: `DateTime<Utc>` + `Duration` → `DateTime<Utc>` (offset metadata unchanged)
- `.to_rfc3339()` on `DateTime<Utc>` always emits the `+00:00` literal regardless of numeric value
- Variable name `now_ist` confirms author intent was IST display — implementation produced IST-numeric/UTC-labelled hybrid
- Used at line 294 (DB INSERT) AND line 317 (WS broadcast `ContentDriftDetected.detected_at`)

**Bug B · `crates/racecontrol/src/fleet_intelligence.rs:207-208`**
```rust
let now = (Utc::now() + chrono::Duration::hours(5) + chrono::Duration::minutes(30))
    .to_rfc3339();
```
- Identical anti-pattern, assigned to `FleetIntelligenceResponse.generated_at` (line 212)
- Surfaces in `GET /api/v1/fleet/intelligence` admin endpoint response

### Compensating-pattern sites (manual correction · NOT a bug, but fragile)

**`crates/rc-sentry/src/mi_tier_engine.rs:497-499`** and **`mi_debug_state.rs:89-91`**
```rust
let now = chrono::Utc::now();
let ist = now + chrono::Duration::hours(5) + chrono::Duration::minutes(30);
ist.format("%Y-%m-%dT%H:%M:%S+05:30").to_string()
```
- Workaround: format with literal `+05:30` string after the +5h30m shift
- Output is correct ISO8601-IST, but the technique is fragile (literal string can drift; doesn't survive type-system migration to `DateTime<Tz>`)
- Note: drops sub-second precision (no `%.f`)

**`crates/racecontrol/src/fleet_deploy.rs:140-142`**
```rust
let utc = Utc::now();
let ist = utc + Duration::hours(5) + Duration::minutes(30);
// Build a FixedOffset-aware datetime for RFC 3339 with correct offset.
```
- Comment shows author was aware of the trap; full FixedOffset machinery follows

### Idiomatic-correct site (the V2 reference pattern)

**`crates/weekly-report/src/main.rs:2,29`**
```rust
use chrono_tz::Asia::Kolkata;
let now_ist = Utc::now().with_timezone(&Kolkata);
```
- Uses `chrono_tz` already in the workspace (no new dep)
- Type-safe: `DateTime<Tz>` carries the Asia/Kolkata zone through every method
- `.to_rfc3339()` on `DateTime<chrono_tz::Tz>` correctly emits `+05:30`
- Survives DST transitions (n/a for IST but matters as principle)

### Read-side / blast radius

| File:line | Operation | Effect of TZ-mislabel |
|---|---|---|
| `cloud_sync_payload.rs:381` | `WHERE detected_at > ? ORDER BY detected_at ASC LIMIT 500` (push to Bono VPS) | Lex string compare — works WHILE all rows share the same wrong offset; corrupts sort if a fix-forward lands without backfill |
| `db/migrate_ops.rs:280` | `CREATE INDEX idx_content_drift_detected ON content_drift_events(detected_at)` | Lex sort — same caveat |
| `fleet_kb_crud.rs:425` | `chrono::DateTime::parse_from_rfc3339(&r.detected_at)` (different table — `fleet_incidents`, but same anti-pattern class everywhere this appears) | Parses successfully, returns `DateTime<FixedOffset>` 5h30m off from true UTC for any rows written via the buggy pattern |
| `crates/rc-common/src/protocol.rs:1501` | `ContentDriftDetected { detected_at: String, ... }` WS payload to admin dashboard | Frontend `new Date(detected_at)` will misparse — admin UI shows wrong time |
| Bono VPS replica DB (`content_drift_events` mirror via cloud_sync) | Same value transmitted as JSON string | Bilateral propagation — Bono reads + displays same wrong value |

### Cross-system contracts

| Contract | Specified | Actual | Match? |
|---|---|---|---|
| ISO8601 RFC 3339 timestamp | UTC offset reflects actual offset | Numeric IST + literal `+00:00` | NO |
| `cloud_sync_payload` shape (Bono ingestion) | RFC 3339 timestamps | RFC 3339-shaped but offset-lying | NO |
| `DashboardEvent::ContentDriftDetected` schema | `detected_at: String` (no offset constraint) | `+00:00` label | Schema permissive — data semantically wrong |

---

## Section 2 — Inherited-issue catalogue

### Direct V1 anti-pattern lineage

The `Utc::now() + Duration::hours(5) + Duration::minutes(30)` pattern is V1-class — it predates the V2 `chrono_tz`-via-workspace adoption (visible in `weekly-report` crate). It is the natural reflex when:
- Rust's `DateTime<Utc>` arithmetic feels easier than registering a `chrono_tz` zone
- Author wants "IST wall time" without thinking about how it'll be SERIALIZED

This anti-pattern is documented retroactively in racecontrol/CLAUDE.md under "Project Identity / Timezone":

> **CRITICAL: Git Bash `TZ=Asia/Kolkata` silently fails on Windows** — returns UTC unchanged, no error. NEVER use `TZ=Asia/Kolkata date` for IST. Instead use: `bash scripts/ist-now.sh` (computes UTC+5:30 manually)

The CLAUDE.md note is the *shell-script* version of the same anti-pattern (manual UTC+5:30 arithmetic). The Rust call-sites are an isomorphic mistake in a different language layer.

### Related V1 failure-mode classes (per `session_notes_20260506_v1_process_mess_audit_for_v2_blockers.md`)

| Class | Touch-point | Status |
|---|---|---|
| Audit-blind proxy checking | `build_id` matches but actual semantics broken | The `+00:00` label is a proxy that LOOKS like UTC; consumers trust the label, not the actual numeric semantics. Same anti-pattern class. |
| Schema drift | TOML inventory vs disk content vs Bono mirror | `detected_at` semantic schema (UTC-vs-IST) drifts between writers within the same crate (5 sites, 3 different patterns) |
| Persisted-data corruption | 36k rows written over 32 days | Not a one-time bug — a continuous corruption rate of ~1,131 bad rows/day |

### Past-bug grep results

- racecontrol LOGBOOK: matches present (omitted by display) but no prior fix entries for `detected_at` TZ-mislabel
- bono memory (comms-link/briefings/bono/memory/): zero matches for `TZ`/`timezone`/`detected_at`/`+05:30` — this finding has NOT been raised by bono before
- racecontrol/.planning/audits/: no `ROOT-CAUSE-*TZ*` or `ROOT-CAUSE-*timestamp*` files

This is a NEW finding. The 36,199 rows + 32-day age means it's been silently producing corrupt data since Phase 366 GLD-F-03 shipped.

---

## Section 3 — Past-bug review (per-bug disposition)

| Past-bug entry | Disposition | Reasoning |
|---|---|---|
| Bug A (`content_drift.rs:282`) | **UNRESOLVED** — first surface 2026-05-13 | 36,199 corrupt rows persisted; no prior fix attempt |
| Bug B (`fleet_intelligence.rs:207`) | **UNRESOLVED** — first surface 2026-05-13 | Affects every `/api/v1/fleet/intelligence` admin response; sibling discovered during this RCA |
| `mi_tier_engine.rs` / `mi_debug_state.rs` literal-`+05:30`-format | **PATCHED-ONLY** — semantically correct but fragile | Output is right; technique relies on literal string concat. Sub-second precision lost. Should converge on V2 idiom in same migration. |
| `fleet_deploy.rs:140-142` FixedOffset construction | **PATCHED-ONLY** — verbose but correct | Manual FixedOffset machinery; works. Should converge on V2 idiom. |
| `weekly-report/main.rs` `chrono_tz::Asia::Kolkata` | **NOT-APPLICABLE-TO-V2** — already V2-correct | Reference pattern. No change needed. |
| CLAUDE.md UTC-vs-IST shell-script warning | **PATCHED-ONLY** at doctrine layer | Documents the trap for shell scripts; doesn't enforce on Rust call-sites. Hook-class enforcement gap (sibling-of `feedback_network_map_before_ip_probe_20260512.md` pre-go-live escalation pattern). |

---

## Section 4 — V2-alignment delta

### What V2 doctrine says this should look like

Per the V2 substrate principle of *foundation/strategy/config separation* (§AMEND-3.II D12) and the kaizen-discipline rule of "smallest invariant change":

1. **Single timestamp idiom across the racecontrol crate**: `chrono_tz::Asia::Kolkata` + `Utc::now().with_timezone(&Kolkata)` for IST display strings; `Utc::now().to_rfc3339()` for storage.
2. **Storage convention**: persist as UTC `+00:00` (canonical, sortable, machine-friendly). Display layer converts to IST. Current `content_drift.rs` violates this by storing IST-numeric in a column whose other readers expect UTC.
3. **Type-system enforcement**: prefer `DateTime<chrono_tz::Tz>` over string-based offset gymnastics. Rust's type system catches misuse at compile time vs runtime semantic corruption.
4. **Cloud-sync invariant**: data crossing the venue→Bono boundary must round-trip identically. `+00:00`-labeled IST values violate this at the wire-format level — Bono's reader cannot distinguish "real UTC" from "IST-mislabeled UTC" without out-of-band knowledge.

### Where the boundary IS today vs WHERE it SHOULD be

| Dimension | Today | V2-target |
|---|---|---|
| Timestamp idiom | 5 patterns across 6 files (1 V2-correct, 2 buggy, 3 compensating) | 1 pattern (V2-correct) everywhere |
| Storage column semantic | Mixed: most tables store UTC; `content_drift_events` stores IST-mislabeled | Single semantic — UTC for all storage |
| Cloud-sync wire format | Lying about offset | Honest about offset (UTC at wire, IST only at presentation layer) |
| Doctrine enforcement | CLAUDE.md note for shell scripts only | clippy-lint OR pre-commit grep gate for `Utc::now() + chrono::Duration::hours(5)` pattern across Rust crates |

### Foundation/strategy/config separation lens

- **Foundation**: timestamp serialization is a foundation-class invariant. Bug here corrupts every dependent layer.
- **Strategy**: how to display IST vs UTC is a strategy choice (presentation-layer concern, not storage).
- **Config**: timezone is fixed (Asia/Kolkata venue) — no runtime config knob needed.

Today the bug LIVES at the foundation layer, masquerading as a strategy concern (the `now_ist` variable name betrays the strategy/foundation conflation).

---

## Section 5 — Proposed change (V2-framed)

### Proposal

**Phase 1 — Stop the bleeding (smallest reversible)**
- Bug A fix: `content_drift.rs:282-283` → `Utc::now().to_rfc3339()` (store UTC; remove the +5h30m shift entirely)
- Bug B fix: `fleet_intelligence.rs:207-208` → same
- Test fixture at `content_drift.rs:371` already uses `+05:30` — update to `+00:00` to match storage convention
- WS event `ContentDriftDetected.detected_at` becomes correctly-UTC; admin dashboard frontend converts to IST at presentation layer (likely already does — `new Date(...)` with Indian browser locale displays IST)

**Phase 2 — Idiom convergence (separate PR; per-PR Captain auth)**
- Migrate `mi_tier_engine.rs`, `mi_debug_state.rs`, `fleet_deploy.rs` to `chrono_tz::Asia::Kolkata` idiom
- Add to workspace Cargo.toml if not present (already a dep in `weekly-report`; check if workspace-level)

**Phase 3 — Doctrine enforcement (separate, hook-class)**
- Pre-commit grep for `Utc::now() + .* hours(5).*minutes(30)` returning non-zero in Rust files = block (sibling-of network-map IP rule structural fix pattern)
- racecontrol/CLAUDE.md addition under "Code Quality" standing rules: timestamp idiom convention

### Backfill question (Captain decision needed)

The 36,199 existing rows are IST-labeled-as-UTC. Two options:

| Option | Action | Trade-off |
|---|---|---|
| A. Backfill | One-shot SQL: `UPDATE content_drift_events SET detected_at = datetime(detected_at, '-19800 seconds') || '+00:00'` (subtract 5h30m, fix label) | Single reversible migration; loses sub-second precision unless we use string manipulation; affects Bono mirror via next cloud_sync push |
| B. Mark + cutover | Add `_legacy_ist_labeled_utc` flag column; new rows correct, old rows flagged | Schema change; readers need flag-aware logic forever; no data loss |
| C. Drop + restart | `DELETE FROM content_drift_events;` (data is observational/non-critical per "fleet intelligence audit log" framing) | Loses 32d of audit history; cleanest forward state; Bono mirror also needs DELETE |

Recommendation: **A (backfill)** — drift events are observational diagnostic data, not customer-facing or financial. Single migration is reversible (apply +5h30m back if needed). Bono push next cycle replays corrected values via cloud_sync's `WHERE detected_at > ?` semantics (need to verify `last_push` cursor doesn't break — see mechanism-trust check).

### V2-doctrine alignment statement

> **V2 doctrine alignment:** This change moves the racecontrol crate toward a single foundation-class timestamp idiom (`chrono_tz::Asia::Kolkata`), eliminates a persisted-data semantic corruption that propagates to Bono VPS via cloud_sync, and converges 3 compensating patterns onto the V2-correct reference site (`weekly-report`). Composes with foundation/strategy/config separation (§AMEND-3.II D12) and Verify-Before-Generate (the bug ships because no read-after-write verify ever ran on `detected_at` semantics).

### Smallest-reversible-change ordering

1. **Phase 1 only** in first PR — surface area: 2 files, ~6 LOC delta. Reversible via single-commit revert.
2. Backfill SQL as separate Captain-auth'd action AFTER Phase 1 deploys (so new writes are correct before historical fix).
3. Phase 2 as second PR (3 file convergence; ~15 LOC).
4. Phase 3 as third PR (hook + doctrine).

### Mechanism-trust check (separate file)

The cloud_sync_payload surface is shared infrastructure (per `feedback_mechanism_trust_check_upstream_of_fix_rca_20260510.md`). Trust check is at:
`.planning/specs/v2/MECHANISM-TRUST/cloud-sync-20260513.json`

This RCA is gated on the trust-check disposition. If trust-check FAILS, cloud_sync gets its own §S-146 RCA before this fix proceeds.

---

## Halt point

NO source code edited. Phase 1 is ready as a smallest-reversible-change. Captain per-PR auth required to proceed (foundational boundary class — bilateral with Bono).

**Captain ask (one of):**
- (A) Authorize Phase 1 only as one PR (2 files, ~6 LOC), backfill TBD after deploy
- (B) Authorize Phase 1 + backfill SQL as bundled
- (C) Defer all phases; this RCA stands as a backlog entry for later session
- (D) Escalate to MMA Step 1 DIAGNOSE on the trust-check question first (cloud_sync atomic primitives etc.)
