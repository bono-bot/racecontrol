# RCA — `cloud_sync` surface trust gaps (Q2 cursor + Q3 verify + Q5 enforcement)

**Class:** §S-146 V1↔V2 RCA · 5-section · foundational integration boundary (bilateral with bono)
**Author:** james (claude opus 4.7) · 2026-05-13 IST
**Surface:** racecontrol crate `cloud_sync` family + comms-link relay `/sync/*` + Bono VPS `/sync/push` handler
**Status:** RCA AUTHORED — NO source edit applied · code change PENDING Captain per-PR auth
**Trigger:** [Mechanism-trust check 2026-05-13](../specs/v2/MECHANISM-TRUST/cloud-sync-20260513.json) returned FAIL on Q2/Q3/Q5 in service of [content_drift TZ-mislabel RCA](RCA-2026-05-13-content-drift-detected-at-tz-mislabel.md). Doctrine: "FAIL → infrastructure surface gets its own §S-146 RCA before fix RCA proceeds."
**Composes-with:** [feedback_mechanism_trust_check_upstream_of_fix_rca_20260510.md], `comms-link/briefings/bono/memory/cloud-sync.md` (bono-side substrate)

---

## Symptom (the trust-check FAIL items)

Mechanism-trust check on cloud_sync 2026-05-13 returned three blocking concerns:

| Q | Question | Verdict | Evidence |
|---|---|---|---|
| Q2 | TTL-bounded sentinels | PARTIAL | `last_push` cursor straddles TZ-format boundary; if a fix-forward writes correct UTC values, monotonic ordering breaks (5h30m gap). |
| Q3 | Behavioral verify (binary hash / mtime / ws_uptime — NOT echo string) | NO | `cloud_sync.rs` logs the count pushed but never reads back from Bono. `tracing::info!("Cloud sync push: {} content_drift_events")` is the proxy-not-evidence anti-pattern (CGP H3 violation by code). |
| Q5 | Guards have written contracts with delivery script | NO | No clippy lint, no pre-commit, no PreToolUse hook for the timestamp-anti-pattern OR for missing-column-in-cloud-sync class. Soft rules in CLAUDE.md only. |

These three gaps are present TODAY in production code, independent of the content_drift TZ bug they were detected through.

---

## Section 1 — Boundary map

### Transport surfaces (2 modes)

| Mode | Path | Cadence | Health gate |
|---|---|---|---|
| Relay | localhost `:8766` comms-link → Bono VPS WS `:8765` | 2s tick when up; 30s push interval | `/relay/health` → `connected: true` (`cloud_sync.rs:125-148`) |
| HTTP fallback | direct GET/POST against `cloud.api_url` (Tailscale `100.70.177.44:8080`) | 30s; circuit breaker 5 fails → 60s open | `cloud_sync.rs:380-407` |

Hysteresis between modes: 3 fails → declare relay down; 2 successes → declare relay up (`RELAY_DOWN_THRESHOLD` / `RELAY_UP_THRESHOLD`).

### Auth surface

- **HMAC-SHA256** signing per `sign_sync_request` ([cloud_sync.rs:57-66](C:/Users/bono/racingpoint/racecontrol/crates/racecontrol/src/cloud_sync.rs#L57-L66))
- 5-minute timestamp drift window (replay window, [line 78](C:/Users/bono/racingpoint/racecontrol/crates/racecontrol/src/cloud_sync.rs#L78))
- Nonce replay protection with 5-min purge ([line 86](C:/Users/bono/racingpoint/racecontrol/crates/racecontrol/src/cloud_sync.rs#L86))
- AUTH-05 fail-CLOSED on mutex poison ([line 106](C:/Users/bono/racingpoint/racecontrol/crates/racecontrol/src/cloud_sync.rs#L106))
- Fallback header `x-terminal-secret` when `sync_hmac_key` not configured ([line 218](C:/Users/bono/racingpoint/racecontrol/crates/racecontrol/src/cloud_sync.rs#L218))

### Push tables (venue → Bono)

Collected from [cloud_sync_payload.rs `collect_push_payload`](C:/Users/bono/racingpoint/racecontrol/crates/racecontrol/src/cloud_sync_payload.rs):

| Table | Cursor field | Cadence | Notes |
|---|---|---|---|
| laps | `created_at > ?` | per-cycle, LIMIT 500 | venue-authoritative |
| track_records | (push all — small) | every cycle | full-table push |
| personal_bests | (push all — small) | every cycle | full-table push |
| billing_sessions | per session | per-cycle | venue-authoritative |
| content_drift_events | `detected_at > ?` LIMIT 500 | per-cycle | **affected by TZ-mislabel bug** |
| metrics_rollups | `updated_at` | per-cycle | push-only per SYNC-FIX-2 (cloud handler crash on pull) |
| model_evaluations | per cycle | per-cycle | |
| (more — file truncated read; not exhaustive enumeration) | | | |

### Pull tables (Bono → venue)

Per `SYNC_TABLES` const ([cloud_sync.rs:42](C:/Users/bono/racingpoint/racecontrol/crates/racecontrol/src/cloud_sync.rs#L42)):

`drivers,wallets,pricing_tiers,pricing_rules,billing_rates,kiosk_experiences,kiosk_settings,auth_tokens,reservations,debit_intents,staff_members,driver_ratings,fleet_solutions,model_evaluations,launch_notes`

**Asymmetric:** push side has tables NOT in pull (drift_events, metrics_rollups, laps). Pull side has tables NOT in push (drivers — venue creates them but pull is the canonical re-sync path; same for wallets et al).

### Cursor mechanism

`last_push` cursor stored in `sync_state` table; normalized via [`normalize_timestamp`](C:/Users/bono/racingpoint/racecontrol/crates/racecontrol/src/cloud_sync.rs#L114) to convert ISO `T` separator → SQLite `space` separator (lex-sort fix from a prior incident — see §2).

**Failure mode:** cursor is text comparison (`WHERE created_at > ?` with text cursor). Format drift between writer (ISO `T+05:30`) and cursor (SQLite `space`) caused historical "updated records invisible" bug — `normalize_timestamp` is the mitigation. But the mitigation is point-in-time and doesn't generalize: any new column added to push without normalization can re-trigger the same class.

### Cross-system contracts (specified vs actual)

| Contract | Specified | Actual | Match? |
|---|---|---|---|
| HMAC over body+nonce+timestamp | `mac.update(timestamp.to_be_bytes() + nonce + body)` | Same on both sides | YES |
| Timestamp format in payload columns | RFC 3339 with honest offset | Mixed (ISO `T+05:30`, SQLite ` `, `+00:00`-mislabeled IST per [content_drift.rs:282](C:/Users/bono/racingpoint/racecontrol/crates/racecontrol/src/content_drift.rs#L282)) | NO — mismatch documented at [`normalize_timestamp:114-121`](C:/Users/bono/racingpoint/racecontrol/crates/racecontrol/src/cloud_sync.rs#L114-L121) |
| Push receipt by Bono | (unspecified) | Not verified — only push-count logged | Schema says nothing; behavior says no |
| Pull / push table parity | (unspecified) | Asymmetric tables on each side | Schema says nothing; behavior shows asymmetry |
| New column added to push | (unspecified) | No automated check that all consumers ALTER simultaneously | "ALTER TABLE for ALL cloud sync tables (10 tables, not just 2)" was a 2026-03-23 fix-forward (5fdbd14f). Same class can recur. |

---

## Section 2 — Inherited-issue catalogue

### Existing same-class bugs in cloud_sync (the trust-check is detecting a real pattern)

The `normalize_timestamp` doc comment at [cloud_sync.rs:111-114](C:/Users/bono/racingpoint/racecontrol/crates/racecontrol/src/cloud_sync.rs#L111-L114) is *itself* the catalogue entry for prior bugs in this surface:

> Normalize ISO timestamps ("2026-03-07T23:48:38.123+00:00") to SQLite format ("2026-03-07 23:48:38"). SQLite's datetime('now') uses space separator, but sync_state stores ISO with 'T'. String comparison: space (0x20) < 'T' (0x54), causing updated records to be invisible.

This is the SAME mistake-class as the content_drift TZ-mislabel: data persisted in one format, compared in another, silent semantic corruption. The fix was a string-normalization helper rather than a structural one (e.g., type-system enforcement of `DateTime<Utc>` end-to-end).

### Past incidents (chronological, from racecontrol/LOGBOOK.md grep)

| Date | Commit | Bug class | Fix class |
|---|---|---|---|
| 2026-03-23 | daaa9298 | Push error storm — 315 errors / 3hrs (no backoff) | Exponential backoff added (5s→300s) |
| 2026-03-23 | 5fdbd14f | Schema gap — `updated_at` migration only on 2 of 10 cloud_sync tables | One-shot ALTER TABLE on remaining 8 |
| 2026-04-07 | 3bf1dfcc | Bidirectional gap — staff_members + billing_rates pushed venue→cloud but never pulled cloud→venue | Added missing pull paths |
| 2026-04-09 (no commit) | Phase 343 plan | **PIN 0009 silent-revert** — "cloud sync overwrote venue write within 30s" — last-writer-wins on writes that crossed the 30s sync window | Triggered v47.0 staff PIN hardening sprint; not closed at cloud_sync layer |
| 2026-04-11 | (audit reference) | "cloud_sync empty-body" handler crash on metrics_rollups pull | SYNC-FIX-2 — metrics_rollups removed from pull list (push-only workaround) |
| 2026-04-22 | c6b4d7c5 | Schema-class — `venue_id` missing on `offline_auto_end` INSERT; DB fallback hardcoded Hyderabad → would break multi-venue cloud sync | Added column on the one missing call-site (1 of 22) |
| 2026-05-13 (this RCA) | n/a | Q3 no-behavioral-verify · Q5 no-enforcement · Q2 cursor-fragility | UNRESOLVED — this RCA is the surfacing |

### Class signal

7 past incidents in 60 days. All share root cause = **cloud_sync as an unverified-by-construction integration surface**. Each fix was a point-in-time patch; none introduced a structural verify mechanism. The trust-check Q3 and Q5 gaps are the doctrine-level naming of this pattern.

### Bono-side substrate

Bono has `comms-link/briefings/bono/memory/cloud-sync.md` (read for bilateral context — NOT re-read in this RCA to avoid scope creep). Other bono memory mentions of cloud-sync: `websocket-sync-phase.md`, `james-completed-tasks.md`, `project_v2_pilot_workload_split_q5.md`, `project_pact_113_resolved_via_pact_002.md`, `project_session_20260427_handoff_pact_001_continue.md`, `MEMORY.md`. Bono is aware of cloud_sync as a doctrine-tracked surface; this RCA composes-with that substrate.

---

## Section 3 — Past-bug review (per-bug disposition)

| Past-bug | Disposition | Reasoning |
|---|---|---|
| 2026-03-23 daaa9298 push storm | **ROOT-CAUSED-AND-FIXED** | Exponential backoff is structural fix; same class can't recur with current code. |
| 2026-03-23 5fdbd14f updated_at migration gap | **PATCHED-ONLY** | Migration applied; no automated check prevents same class on next new table. Composes with Q5 enforcement gap. |
| 2026-04-07 3bf1dfcc bidirectional gap | **PATCHED-ONLY** | Specific tables fixed; no automated audit that every push table has a corresponding pull (or explicit asymmetry doctrine). Drift events + metrics_rollups + laps are TODAY push-only with no pull-side mirror — by design? Undocumented if so. |
| 2026-04-09 PIN 0009 silent-revert | **UNRESOLVED at cloud_sync layer** | Triggered upstream sprint (v47.0 staff PIN hardening); cloud_sync's last-writer-wins semantic for cross-side concurrent writes still in place. Same class can hit any other table. |
| 2026-04-11 metrics_rollups handler crash | **PATCHED-ONLY (workaround, not fix)** | SYNC-FIX-2 made metrics_rollups push-only. Cloud-side handler crash never root-caused per LOGBOOK. Same crash could hit any other pull table. |
| 2026-04-22 c6b4d7c5 venue_id INSERT | **PATCHED-ONLY** | One call-site of 22 fixed; structural audit test added. Doesn't generalize to other multi-venue-required columns. |
| 2026-05-13 (Q2/Q3/Q5 trust gaps) | **UNRESOLVED — this RCA** | First doctrine-level surfacing of the pattern that the 7 above incidents all share. |

---

## Section 4 — V2-alignment delta

### What V2 says cloud_sync should be

V2 doctrine for foundational integration boundaries (per `feedback_mechanism_trust_check_upstream_of_fix_rca_20260510.md` + `feedback_v1_dependent_v2_root_cause_before_proceeding.md` + V2-MASTER-STATE §S-203 V2-LBAC):

1. **Behavioral verify by construction** — every push must have a deterministic check that the receiver persisted the intended row(s). Not "200 OK from Bono"; an actual row-count read-back or hash agreement.
2. **Cursor invariants type-enforced** — `last_push` cursor format must be the same as the column it queries against. The Rust type system can carry this contract.
3. **Schema-evolution gate** — adding a new push table requires (a) updated_at migration, (b) pull-side decision (mirror or explicit-asymmetric), (c) SCHEMA_VERSION bump, (d) integration test. Hook-class enforcement.
4. **Last-writer-wins is a doctrine choice, not an accident** — cross-side concurrent writes need explicit conflict resolution per table (timestamp-priority OR side-priority OR merge function). Currently silent / accidental.
5. **Same timestamp idiom as the rest of the codebase** — UTC at storage, IST at presentation. cloud_sync currently has 3 timestamp formats in flight (ISO `T+05:30`, SQLite ` `, `+00:00`-mislabeled IST).
6. **Drift detector for cloud_sync itself** — meta-observability. The cloud_sync mirror at Bono VPS should be drift-checked vs venue source-of-truth for the canonical tables (drivers / wallets / billing) on a rolling cadence.

### Today vs target

| Dimension | Today | V2-target |
|---|---|---|
| Push verify | Log-only count | Receiver-confirmed row-count or hash |
| Cursor format | Text comparison via `normalize_timestamp` mitigation | Type-enforced `DateTime<Utc>` end-to-end |
| Schema gate | None | Pre-commit + CI test on `cloud_sync_payload.rs` deltas |
| Conflict resolution | Implicit last-writer-wins | Per-table doctrine (documented + tested) |
| Timestamp idiom | 3 formats in flight | 1 (UTC stored, IST displayed) |
| Meta-observability | None | Mirror-drift detector with `_mirror_drift_events` table |

### Foundation/strategy/config separation lens

- **Foundation**: HMAC + nonce (auth) + retry/backoff (resilience) — already V2-clean
- **Strategy**: WHICH tables sync, WHICH direction(s), WHICH conflict resolution per table — currently buried in code, should be config-driven OR explicit doctrine
- **Config**: relay vs HTTP, intervals, thresholds — already in `racecontrol.toml [cloud]`

The Q3/Q5 gaps live at the foundation/strategy boundary — cloud_sync's verify mechanism is foundation, but it has no implementation. Q2 cursor lives at foundation/config boundary — format is implicit in the column choice, not declared anywhere.

---

## Section 5 — Proposed remediation (V2-framed, ranked)

### Phase 1 — Verify-by-construction for push (closes Q3) — SMALLEST REVERSIBLE

**Change:** After each push to Bono, issue a follow-up `GET /sync/echo?table=X&from=<cursor>&to=<new_cursor>` that returns the row count Bono persisted in that window. If mismatch, log ERROR + revert cursor advance (re-push next cycle).

- Code surface: `cloud_sync_push.rs` after `push_via_relay` success
- Bono-side: new `/sync/echo` endpoint returning `SELECT COUNT(*) FROM <table> WHERE <cursor_col> > ? AND <cursor_col> <= ?`
- LOC estimate: ~30 (Rust) + ~20 (Bono Node)
- Risk: low — additive, fail-open if `/sync/echo` 404 (Bono not yet upgraded)
- Reversibility: single-commit revert

### Phase 2 — Type-enforced cursor (closes Q2)

**Change:** Migrate `last_push` storage and all cursor queries to use `DateTime<Utc>` with `chrono` serialization (not raw text comparison). `normalize_timestamp` becomes obsolete; type system enforces format match.

- Code surface: `cloud_sync.rs` cursor read/write + every `WHERE created_at > ?` site in `cloud_sync_payload.rs`
- LOC estimate: ~80
- Risk: medium — wide touch, but each site is mechanical rewrite
- Reversibility: single-commit revert; data forward-compatible (cursor table unchanged)

### Phase 3 — Schema-evolution gate (closes Q5)

**Change:** Pre-commit hook (parser-not-regex) on `cloud_sync_payload.rs` deltas: any new `INSERT INTO sync_payload[...]` requires (a) presence of `updated_at` column in the SELECT JSON, (b) explicit `// PUSH-ONLY: <reason>` comment OR matching pull-side handler, (c) SCHEMA_VERSION bump, (d) integration test entry.

- Code surface: `racecontrol/scripts/hooks/cloud-sync-schema-gate.sh` + pre-commit registration
- LOC estimate: ~60 (hook script)
- Risk: low — additive enforcement; doesn't touch runtime
- Reversibility: hook removal

### Phase 4 — Conflict resolution doctrine (addresses PIN 0009 incident class)

**Change:** Per-table conflict resolution table (config or code-doc). Each pull/push table tagged: `last_writer_wins` / `venue_authoritative` / `cloud_authoritative` / `merge: <function>`. Code reads tag; documented in CLAUDE.md.

- Code surface: new module `cloud_sync_conflict.rs` + per-table tag table + read in upsert path
- LOC estimate: ~120
- Risk: medium — changes upsert semantics; needs MMA Step 1 DIAGNOSE since auth/wallet are foundational sub-boundaries
- Reversibility: rollback restores last-writer-wins (current state)

### Phase 5 — Mirror-drift detector (addresses meta-observability gap)

**Change:** Background task on Bono VPS that periodically (1h) computes hash of canonical tables (drivers / wallets / billing_sessions over last 7d window) and pushes back to venue for comparison. Drift fires WS event + WhatsApp alert.

- Code surface: new module on both sides
- LOC estimate: ~200
- Risk: low — observational only, no behavior change
- Reversibility: stop the task

### Recommended ordering

**Phase 1 first** — closes the Q3 trust-check fail with smallest blast radius. Once Phase 1 is shipped + soaked, content_drift TZ fix can proceed (RCA Phase 1 from the parent RCA) under the new verify mechanism (any backfill-induced cursor confusion will surface as an echo-mismatch, not silent corruption).

Phase 2/3 can run in parallel to content_drift fix (independent surfaces).

Phase 4/5 are larger and should wait for Captain prioritization vs other V2 LBAC items.

### V2-doctrine alignment statement

> **V2 doctrine alignment:** This RCA names the cloud_sync surface as a foundational integration boundary lacking behavioral-verify (Q3) and enforcement-gate (Q5) mechanisms. Phase 1 closes the most acute gap (verify-by-construction) using the smallest reversible additive change. Composes with: mechanism-trust-check upstream of fix RCA (this surface is the upstream surface for content_drift fix and any future cloud_sync-touching work) · V2-LBAC bilateral close-loop (4-leg ledger entry on remediation) · V1-dep V2 RCA (catalogues 7 past patched-only incidents whose root cause is the absence of these mechanisms).

---

## Halt point

NO source code edited. NO Bono-side change requested. RCA + remediation plan are complete deliverables; Phase 1 is ready for Captain per-PR auth.

**Captain ask (one of):**
- (A) Authorize Phase 1 (verify-by-construction `/sync/echo`) — smallest, closes Q3, unblocks content_drift TZ fix
- (B) Authorize Phase 1 + Phase 2 (cursor type-enforcement) bundled — closes Q3 + Q2
- (C) Authorize Phase 1 + Phase 3 (verify + enforcement gate) — closes Q3 + Q5
- (D) Defer; this RCA stands as backlog. Note: defers content_drift TZ fix indefinitely (gated on this).
- (E) MMA Step 1 DIAGNOSE on Phase 4 (conflict resolution doctrine) before any phase ships — biggest blast radius, foundational sub-boundary

Bilateral note: This RCA touches a bilateral canonical surface. Bono needs to author the corresponding `/sync/echo` Node handler + signoff before Phase 1 ships. Per V2-LBAC §3 step 8, push this RCA to bono via comms-link AFTER Captain disposition (not before — Q3 boundary 2 push gate).
