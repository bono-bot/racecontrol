# MMA Step 2 PLAN — W1-S5 + W1-S6 + W3 Triplet (Wave A — substrate authoring)

**Scope**: W1-S5 + W1-S6 + W3 RCA triplet (foundational-boundary class — auth + auth + wallet)
**Authored**: 2026-05-09 ~12:25 IST · **Authored-by**: james (Claude Opus 4.7 1M)
**Substrate-class**: foundational-boundary triplet PLAN
**Status**: REFERENCE-ONLY-SUPERSEDED-BY-WAVE-A2 — successor `MMA-STEP-2-W1S5-W1S6-W3-PLAN-A2.md` shipped 2026-05-09 ~19:15 IST integrating §13 amendment as primary, absorbing PR #67 `7dcedd00` amended RCAs (Captain Option 4 zero-spend substrate path), and consolidating N=2 supplementary PR-breakdown. Wave A retained as reference for design-history-trail per §13.3 promise.

**Original Status (preserved for trail)**: DRAFT-REVISE-PENDING-SUPPLEMENTARY-ABSORPTION — Wave A authorial substrate authored against canonical §S-153 14-CONSENSUS THEN supplementary run at racecontrol `MMA-W1-S5-W1-S6-DIAGNOSE/SYNTHESIS.md` (slot-collision N=4 hit 2026-05-09 ~12:12 IST) returned REVISE disposition with 4 newly-promoted CONSENSUS items + 3 NEW Captain Q-DECISIONs. Per supplementary §7 recommended workflow: W1-S5 + W1-S6 RCAs amend → MMA Step 1 re-run → Step 2 PLAN re-author (NOT H1 PLAN direct from this Wave A). This Wave A substrate is reference-only until RCA-amendment cycle completes. See §13 supplementary absorption.
**Filed under**: §S-153 MMA Step 1 closure + §S-153.8 forward gate-action + supplementary REVISE disposition (slot-collision-N=4 anchor)
**Authorization chain**: user-shipped G33-statement (drafted by james-this-session, returned verbatim 12:20 IST) → Step 2 PLAN authoring authorized + Steps 3+4 cascade authorized + class-level per-PR merge auth pre-grant for foundational-boundary (auth + billing + wallet) at PR-open with evidence pack
**V2 doctrine alignment**: §S-146 V1↔V2 RCA rule SECOND end-to-end pipeline application · §AMEND-3.II D12 Foundation/Strategy/Config separation · Wallet Framing C · Pod-display state-channel premise · F-05 anti-pattern codification candidate

---

## §1 — Step 1 inputs absorbed

| Class | Count | Treatment |
|---|---|---|
| CONSENSUS (≥3/5) | 14 | MANDATORY design inputs — disposition each per §2 below |
| MINORITY (2/5) | 5 | Evaluated at Step 4 VERIFY adversarial models — included in PLAN scope only if Step 4 score ≥ 4.0 |
| SINGLETON (1/5) | 20 | Listed for traceability per Protocol v3.0 — not gating |

**Source**: `.planning/specs/v2/MMA-STEP-1-W1S5-W1S6-W3-CONSENSUS.md` (713 lines, racecontrol commit `f599c316`).

---

## §2 — Per-finding disposition (CONSENSUS, 14 items)

Disposition codes:
- **IN-PLAN** — included in W1-S5/W1-S6/W3 PR-A scope; concrete mitigation specified
- **IN-PLAN-WITH-DOC** — code comment + observability addition; no behavior change
- **IN-PLAN-AS-PRECONDITION** — probe/check before PR-A code-author begins
- **IN-PLAN-AS-CROSS-RCA-DOCTRINE** — touches all three PRs at shared boundary (audit_log, state-machine)
- **DEFER** — explicit defer with rationale (post-Wave-1 / sibling-PACT / etc.)
- **RESOLVED-BY-PRIOR-§S-N** — finding state already changed by prior session-state ship; no further action

### F-CONS-1 [P0] [W1-S5] create_staff_jwt cashier-default role-downgrade — IN-PLAN
- **Mitigation**: Extract `mint_refreshed_jwt(claims) -> Result<String>` helper using `claims.role` explicitly. Never `create_staff_jwt` default.
- **Test**: `manager_superadmin_role_preserved_across_100_refresh_cycles`
- **Lint candidate** (NEW-Q-DECISION sibling): forbid direct `create_staff_jwt` in new code (post-W1-S5 cleanup PACT)
- **PR**: W1-S5 PR-A · file: `crates/racecontrol/src/auth/middleware.rs` (refresh post-handler) + `auth.rs` (helper extraction)

### F-CONS-2 [P0] [W3] F-05 UPDATE-then-SELECT-same-column anti-pattern in capture path — IN-PLAN
- **Mitigation**: `WalletService::capture` snapshot-read BEFORE any wallet UPDATE. Compute delta from snapshot. Never read `balance_credits` after UPDATE. Code comment at capture site referencing `ROOT-CAUSE-ANALYSIS-F05-2026-03-28.md` + ₹162.50 incident.
- **Test**: `f05_anti_pattern_regression_check_capture_path`
- **NEW-Q-DECISION-1** (post-W3 follow-up): codify as Standing Rule sub-PACT for clippy lint (`clippy::update_then_select_same_column`). DEFER-TO-CAPTAIN-G33-NEXT-BILATERAL with default-YES per ACCEPT-DEFAULTS-by-precedent-extrapolation pattern (§S-152.3).
- **PR**: W3 PR-A · file: `crates/v2-db/src/wallets.rs::capture`

### F-CONS-3 [P2] [W1-S5] PHASE-1-WAVE-1-PLAN.md '7-min' vs '30-min' — IN-PLAN
- **Mitigation**: Amend `PHASE-1-WAVE-1-PLAN.md` rows 21+33 from "7-min fixed-window" to "30-min sliding-window" in W1-S5 ship commit. Code comment referencing Captain G33 Q-S5-7 ACCEPT-DEFAULT (plan-author typo class).
- **Test**: assert `idle_timeout_secs=1800` in sliding-window path (config-parity regression test)
- **PR**: W1-S5 PR-A · file: `.planning/PHASE-1-WAVE-1-PLAN.md` (text edit) + middleware.rs comment

### F-CONS-4 [P0] [W1-S6] V1 IP-keyed rate-limit unusable for per-staff-id semantics — IN-PLAN
- **Mitigation**: NEW per-staff-id primitive in `staff_auth.rs::PinLockoutTracker` with `ResetState { count: u32, window_start: DateTime<Utc> }`. Inline impl per Captain Q-S6-2 Option A ACCEPT-DEFAULT. Do NOT extend `tower_governor::PeerIpKeyExtractor`.
- **Composes-with Q-S6-8 FLAG-1** (DEFAULT Option C): extract `pin_lockout` module called from the 3 staff-PIN-validate endpoints listed in `rate_limit.rs:12-13`. Resolves Q-S6-2 vs Q-S6-8 tension by extracting lockout logic to its own module while keeping inline `ResetState` (Q-S6-2 Option A).
- **Test**: `per_staff_id_isolation_two_staff_same_ip_independent_counters`
- **PR**: W1-S6 PR-A · NEW file: `crates/racecontrol/src/auth/staff_auth.rs` + `crates/racecontrol/src/auth/pin_lockout.rs`

### F-CONS-5 [P1] [W1-S6] EmailAlerter unbounded HashMap growth — DISPOSITION-RESOLVED-BY-CONS-7
- **Mitigation**: Captain Q-S6-1 ACCEPT-DEFAULT (per §S-148.4 batch) bypasses cooldown HashMap entirely for PIN-rotate event-class. Bypass eliminates the unbounded-growth axis for staff-id keys (no staff-id keys ever inserted into HashMap). Per-pod HashMap unbounded-growth is V1-pre-existing (out-of-scope-of-W1-S6 mitigation per kaizen-min).
- **Defer-rationale**: per-pod HashMap pruning is V1 footgun separate from W1-S6 scope; sibling PACT candidate post-Wave-1 if V1 EmailAlerter remains in V2.
- **PR**: NONE — disposition resolved by F-CONS-7 mitigation pattern

### F-CONS-6 [P1] [W1-S6] SMTP transport + DKIM/SPF unverified — IN-PLAN-AS-PRECONDITION
- **Mitigation**: Pre-W1-S6-PR-A probe action — verify at Server .23 + Bono VPS:
  - `which sendmail` / `tail /var/log/mail.log`
  - `dig +short TXT racingpoint.in` (SPF check)
  - `dig +short TXT default._domainkey.racingpoint.in` (DKIM)
  - Google Workspace API auth scope verify (if needed)
- **Outcome branch**:
  - GREEN (sendmail/SMTP available + DKIM/SPF set): proceed to W1-S6 PR-A
  - YELLOW (sendmail available; DKIM/SPF absent): surface Captain Q-DECISION (ship-with-risk + monitor first delivery vs delay)
  - RED (no transport): block W1-S6 PR-A; surface Captain Q-DECISION immediately
- **PR**: ACTION-PRE-W1-S6 · gates substrate ship; no code

### F-CONS-7 [P1] [W1-S6] V1 cooldown semantics conflict with event-class — IN-PLAN
- **Mitigation**: NEW sibling method `EmailAlerter::send_pin_rotation(staff_id, new_pin) -> Result<()>` that bypasses cooldown HashMap entirely. Method comment: "PIN-rotate emails are event-class; always deliver per Captain Q-S6-1. Bypasses per-pod 1800s + venue-wide 300s alert-class cooldown by design."
- **Test**: `pin_rotation_email_delivered_even_when_cooldown_blocks_alert_class`
- **Composes-with F-CONS-5**: this mitigation eliminates F-CONS-5 (no staff-id keys inserted into cooldown HashMap)
- **PR**: W1-S6 PR-A · file: `crates/racecontrol/src/email_alerts.rs`

### F-CONS-8 [P1] [W3] wallet_redemptions hold_id column — IN-PLAN
- **Mitigation**: Migration `ALTER TABLE wallet_redemptions ADD COLUMN hold_id TEXT REFERENCES wallet_holds(id)` (W3 migration .sql). Update `WalletService::capture` to populate column from active hold. Add migration index for forensic queries.
- **Test**: `bonus_source_tag_preserved_through_hrc_capture` — assert hold_id populated through full hold→capture→redemption path
- **Doctrine alignment**: Wallet Framing C source-tag preservation invariant + PACT-024 §3 Q5 5-AGREE recommendation
- **PR**: W3 PR-A · file: `crates/v2-db/migrations/<datestamp>_wallet_redemptions_add_hold_id.sql` + `crates/v2-db/src/wallets.rs::capture`
- **Sibling-PACT-024a**: re-target paths per F-CONS-13

### F-CONS-9 [P2] [W1-S6] In-memory lockout state durability per CR-3 — IN-PLAN-WITH-DOC
- **Mitigation**: Accept restart-forgiveness per Captain Q-S6-6 ACCEPT-DEFAULT + CR-3 customer-service-priority. Code comment in `pin_lockout.rs` module-level: `//! In-memory only; restart-after-5-wrong acceptable per CR-3. DB-backed deferred to V2.1 if abuse pattern emerges.` Add Prometheus counter `auth.pin_lockout_restart_count` for observability without behavior change.
- **Test**: doc-test that asserts comment present (kaizen-min)
- **Defer-to-V2.1-trigger**: abuse pattern (≥3 lockout-restart-resets/30d at single staff_id)
- **PR**: W1-S6 PR-A · file: `crates/racecontrol/src/auth/pin_lockout.rs` (header comment) + `metrics.rs` (counter)

### F-CONS-10 [P0] [CROSS] Auth boundary changes preserve audit_log schema + state-machine consistency — IN-PLAN-AS-CROSS-RCA-DOCTRINE
- **Mitigation**: Define unified audit_log `action_type` vocabulary across triplet:
  - W1-S5: NO routine logging on JWT refresh (per Q-S5-disposition + finding F-SING-12 ~100x volume risk); LOG ONLY 401 idle-expiry rejections (action_type = `staff_jwt_idle_expiry`)
  - W1-S6: action_type = `staff_pin_auto_reset` (per PIN-rotate event)
  - W3: action_type ∈ {`wallet_hold_created`, `wallet_hold_captured`, `wallet_hold_released`}
- **State-machine consistency invariant**: HOLDs are session-bound, NOT auth-bound. Release fires only on session terminal-state (game-running stop / launch-fail / staff cancel), NOT staff JWT expiration. Per Captain Q-W3-13 default.
- **Cross-RCA regression test**: `triplet_audit_log_action_types_distinct_no_schema_conflict` + `holds_session_bound_not_auth_bound`
- **PR**: cross-cutting in all 3 PRs (W1-S5/W1-S6/W3) · scattered touches at audit_log call sites + state-machine boundary
- **Composes-with F-CONS-12**: V2 Audit-Log Doctrine

### F-CONS-11 [P0] [W3] PACT-024 Q1-Q5 dispositions outstanding — RESOLVED-BY-§S-151
- **Status**: PACT-024 already AMPLIFIED-2026-05-05 via canonical batch `c5529c1` msg=35096 with bono Q1-a/Q2-c/Q3-c/Q4-c/Q5-c 5-AGREE recommendations adopted. Stale-claim self-G9 absorbed at §S-152.5.
- **Net**: gate already cleared. Finding obsolete post-§S-151. **No further action required.**
- **PR**: NONE
- **Provenance note**: this finding surfaced in MMA Step 1 because individual model context lacked §S-151 self-G9 corrective. Step 2 disposition documents the resolution-by-prior-§S-N pattern for future MMA Step 1 audits.

### F-CONS-12 [P1] [CROSS] Inconsistent Audit-Log Discipline — IN-PLAN-AS-DOCTRINE
- **Mitigation**: Establish V2 Audit-Log Doctrine (NEW-Q-DECISION-2 candidate). Principles:
  - LOG: state-changing events (PIN-rotate, hold/release/capture, wallet writes, staff role changes, security boundary 401s)
  - DO NOT LOG: routine heartbeat events (JWT refresh, periodic re-fetches, health probes, game-running heartbeats)
  - Schema: `action_type` enum-class enforced by lint or test
  - Volume gate: per-event-type rate alarm > 100/min/staff
- **NEW-Q-DECISION-2**: ship as `comms-link/V2-MASTER-STATE.md §S-155` doctrine append (parallel to triplet PR ship or post-triplet). DEFER-TO-CAPTAIN-G33-NEXT-BILATERAL with default-YES per ACCEPT-DEFAULTS-by-precedent-extrapolation.
- **PR**: doctrine ship · file: `comms-link/V2-MASTER-STATE.md` §S-155
- **Composes-with F-CONS-10**: this is the doctrine layer; F-CONS-10 is the in-PR per-action-type implementation

### F-CONS-13 [P1] [W3] PACT-024a §A re-target to v2-db crate — IN-PLAN
- **Mitigation**: Re-target PACT-024a §A file paths from `crates/racecontrol/src/wallet.rs` + `billing.rs` + `routes.rs` to `crates/v2-db/src/wallets.rs` + `crates/v2-db/src/idempotency.rs`. SQL migration target `v2-db` schema. TODO comment referencing original PACT-024a commit for audit trail. NO semantic change — paths-only bump per Captain Q-W3-RECONCILE-1 ACCEPT-DEFAULTS extending to W3 11-Q batch.
- **Test**: NONE (paths-only; semantic equivalence ensured by F-CONS-2 + F-CONS-8 tests)
- **PR**: PACT-024a amendment in W3 PR-A · file: `comms-link/PACTS.md` PACT-024a entry update + W3 implementation files

### F-CONS-14 [P2] [W3] Orphan hold cleanup deferred to PACT-024b — IN-PLAN-WITH-DOC + SIBLING-PACT
- **Mitigation**:
  - W3 PR-A: document orphan-hold deferral in W3 spec (`crates/v2-db/src/wallets.rs` module comment); add `hold_timeout_secs` config (default deferred per Q-W3-DEFAULT); allow staff manual `release_hold` for orphan cases.
  - **Monitoring**: alert if open holds older than 24h accumulate (Prometheus + WhatsApp staff alert).
  - **PACT-024b**: file as immediate follow-up sibling-PACT for time-based hold-expiration sweep ("Kidneys reconciliation worker"). Owner: bono-LEAD per first-mover precedent on PACT-024 family.
- **Test**: `orphan_hold_24h_alert_fires` + `staff_manual_release_hold_unblocks_customer_credits`
- **PR**: W3 PR-A doc + monitoring + sibling PACT-024b filing as separate substrate ship

---

## §3 — NEW Q-DECISIONs surfaced by §1 (not in original RCAs)

Per §S-153.8 item 5 preliminary scan: 0-2 likely. Found 2.

### NEW-Q-DECISION-1: codify F-05 anti-pattern as clippy lint
- **Source finding**: F-CONS-2 mitigation-extension; sibling-anchor F-SING-20 (cross-cutting cite)
- **Default**: YES — file `PACT-DRAFT-clippy-update-then-select-lint` as sibling to W3 PR-A. Composes-with §S-152.4 PR #66 §S-146 first end-to-end pipeline structural-fix-elevation precedent.
- **Disposition path**: DEFER-TO-CAPTAIN-G33-NEXT-BILATERAL per ACCEPT-DEFAULTS-by-precedent-extrapolation (§S-152.3 reading)
- **Class**: Standing Rule sub-PACT (charter-class diagnostic-only OR known-bug-hotfix)
- **Cost**: ~30-50 LOC clippy plugin + tests; ~$0 OpenRouter

### NEW-Q-DECISION-2: V2 Audit-Log Doctrine substrate ship
- **Source finding**: F-CONS-10 + F-CONS-12 mitigation
- **Default**: YES — ship as `comms-link/V2-MASTER-STATE.md §S-155` doctrine append parallel to triplet PR ship OR as post-triplet substrate
- **Disposition path**: DEFER-TO-CAPTAIN-G33-NEXT-BILATERAL per ACCEPT-DEFAULTS-by-precedent-extrapolation
- **Class**: doctrine substrate (V2-MASTER-STATE append; bilateral parity)
- **Composes-with**: §AMEND-3.II D12 Foundation/Strategy/Config separation — extends to audit-log-class
- **Cost**: ~80-120 LOC doctrine markdown; ~$0 OpenRouter

Both surfaced for Captain visibility; NEITHER blocks MMA Step 2 → Step 3 EXECUTE per ACCEPT-DEFAULTS-precedent.

---

## §4 — MINORITY findings deferred to Step 4 VERIFY

Listed for traceability. Step 4 VERIFY adversarial models (≥3 different from Steps 1-3, ≥4.0 score gate) evaluate inclusion in PLAN scope.

| # | RCA | Sev | Title | Step 4 prompt focus |
|---|---|---|---|---|
| F-MIN-1 | W1-S5 | P0 | V2.1 PACT pin file stale after pull-forward | doc-amendment scope confirmation |
| F-MIN-2 | W3 | P0 | Concurrent hold atomicity hole under SQLite single-writer | optimistic-retry pattern soundness |
| F-MIN-3 | W1-S5 | P1 | Response-mutating middleware layer precedent risk | anti-precedent comment sufficiency |
| F-MIN-4 | W3 | P1 | Undefined Refund-During-HOLD Interaction | ordering invariant correctness |
| F-MIN-5 | W1-S5 | P1 | New idle-refresh cookie may collide with existing staff-PIN cookie | cookie-name uniqueness verification |

If Step 4 score ≥ 4.0 each: include in PR-A scope. F-MIN-1 + F-MIN-3 + F-MIN-5 likely auto-pass (already-aligned with Captain Q-S5 dispositions). F-MIN-2 + F-MIN-4 require concrete state-machine correctness review.

---

## §5 — SINGLETON findings traceability list

20 items noted for traceability per Protocol v3.0. Some may surface as Step 4 catches.

| # | RCA | Sev | Title | Model |
|---|---|---|---|---|
| F-SING-1 | CROSS | P0 | Auth blast-radius overlaps with wallet state machine | qwen3-coder |
| F-SING-2 | CROSS | P0 | Auth blast-radius: W1-S5 refresh + W1-S6 lockout interaction | mimo-v2-pro |
| F-SING-3 | CROSS | P0 | V1↔V2 Bridge Module Class Consistency and Auditability | gemini-2.5-flash |
| F-SING-4 | W1-S5 | P0 | Sliding-window token refresh introduces UPDATE-then-SELECT-same-column anti-pattern if not guarded | mistral-small-2603 |
| F-SING-5 | W1-S5 | P1 | Concurrent token refresh races | deepseek-r1-0528 |
| F-SING-6 | W3 | P1 | V1↔V2 event bridge single-point failure | deepseek-r1-0528 |
| F-SING-7 | CROSS | P1 | Asymmetric V1↔V2 bridge implementations | deepseek-r1-0528 |
| F-SING-8 | CROSS | P1 | Idempotency key scope collision | deepseek-r1-0528 |
| F-SING-9 | W1-S5 | P1 | JWT secret rotation grace + sliding-window refresh interaction | mimo-v2-pro |
| F-SING-10 | CROSS | P1 | Cross-pilot transport contract changes undocumented | mimo-v2-pro |
| F-SING-11 | W3 | P1 | T-F1 Bonus Arbitrage Exploit Unaddressed Without W3 HRC | gemini-2.5-flash |
| F-SING-12 | W1-S5 | P1 | Routine JWT refresh would 100x audit_log INSERT volume if logged | mistral-small-2603 |
| F-SING-13 | W3 | P1 | Separate wallet_holds table preferred over inline column for audit clarity + concurrent holds | mistral-small-2603 |
| F-SING-14 | CROSS | P1 | Cross-RCA root cause: V1↔V2 bridge modules must not break cross-pilot contracts | mistral-small-2603 |
| F-SING-15 | CROSS | P1 | Cross-RCA root cause: Wallet Framing C invariants must be preserved across W1-S5/W1-S6/W3 | mistral-small-2603 |
| F-SING-16 | CROSS | P1 | Cross-RCA root cause: Captain dispositions + per-PR Captain merge auth gates honored across PRs | mistral-small-2603 |
| F-SING-17 | CROSS | P2 | V1↔V2 bridge pattern inconsistency across triplet | mimo-v2-pro |
| F-SING-18 | W3 | P2 | Missing V1↔V2 Bridge for Game Heartbeat to Wallet Capture | gemini-2.5-flash |
| F-SING-19 | W3 | P2 | Idle-Timeout-During-HOLD Interaction Undefined | gemini-2.5-flash |
| F-SING-20 | CROSS | P2 | F-05 anti-pattern not codified as lint anywhere; W1-S5 + W3 at-risk | mistral-small-2603 |

**Auto-absorbed by other dispositions**:
- F-SING-4 → resolved by F-CONS-2 mitigation pattern (snapshot-read-before-write applies to W1-S5 refresh too)
- F-SING-12 → resolved by F-CONS-10 (no routine logging of JWT refresh)
- F-SING-13 → confirms F-CONS-8 separate-table choice (architectural anchor)
- F-SING-15 → resolved by F-CONS-10 + F-CONS-12 cross-RCA doctrine
- F-SING-16 → addressed by class-level per-PR merge auth pre-grant in user-shipped G33-statement
- F-SING-18 → in-scope of W3 PR-A wallet_hrc_bridge module per F-CONS-13 substrate
- F-SING-19 → resolved by F-CONS-10 state-machine invariant
- F-SING-20 → NEW-Q-DECISION-1 (parallel-track)

**Likely Step 4 additions to PR scope** (singleton candidates promoted to in-scope at Step 4):
- F-SING-2 (auth blast-radius interaction) — precise interaction test required
- F-SING-3 + F-SING-7 (V1↔V2 bridge doctrine) — doctrine substrate candidate
- F-SING-8 (idempotency key scope collision) — namespace-prefix design candidate
- F-SING-10 (cross-pilot transport contract changes) — substrate ship candidate

---

## §6 — Per-RCA H1 PLAN derivation

Each H1 PLAN file derives from this Step 2 substrate. Wave A authorial (this doc) + Wave B formal multi-model cross-check produces final PR-A scope; H1 PLAN files authored as next-gate-action.

### W1-S5 PLAN scope (auth/middleware.rs sliding-window refresh)
- F-CONS-1 (cashier-default fix) · F-CONS-3 (PHASE-1-WAVE-1-PLAN amend) · F-CONS-10 (audit-log discipline) · F-CONS-12 (audit-log doctrine cross-cut)
- Auto-absorbed: F-SING-4 (refresh path snapshot-read-before-write) · F-SING-12 (no routine refresh logging)

### W1-S6 PLAN scope (staff_auth.rs PIN-LOCKOUT)
- F-CONS-4 (per-staff-id primitive) · F-CONS-5/F-CONS-7 (event-class email bypass) · F-CONS-6 (transport-substrate precondition probe) · F-CONS-9 (in-memory state per CR-3 doc) · F-CONS-10 (audit-log discipline) · F-CONS-12 (audit-log doctrine cross-cut)
- Auto-composes-with: Q-S6-8 FLAG-1 (extract pin_lockout module) · Q-S6-9 FLAG-2 (until-Captain-explicit-unfreeze)

### W3 PLAN scope (v2-db wallets.rs HOLD-RELEASE-CAPTURE)
- F-CONS-2 (F-05 anti-pattern guard) · F-CONS-8 (hold_id column) · F-CONS-13 (PACT-024a re-target) · F-CONS-14 (orphan-hold doc + PACT-024b sibling) · F-CONS-10 (audit-log discipline) · F-CONS-12 (audit-log doctrine cross-cut) · F-CONS-11 RESOLVED-BY-§S-151 (no action)
- Auto-absorbed: F-SING-13 (separate-table architectural anchor) · F-SING-18 (V1↔V2 bridge in-scope) · F-SING-19 (HOLDs session-bound)

---

## §7 — Implementation entry sequence

Per W2 PLAN scaffolding precedent + class-level per-PR merge auth pre-grant scope (user-shipped G33 12:20 IST):

| # | Action | Owner | Class | Gates |
|---|---|---|---|---|
| 1 | Pre-W1-S6-PR-A SMTP/DKIM/SPF probe (F-CONS-6) | james | substrate-ship | none — ACTION |
| 2 | W1-S5 PR-A: middleware.rs + PHASE-1-WAVE-1-PLAN.md amend + audit-log discipline | james-LEAD | foundational-auth | per-PR Captain merge auth at PR-open · evidence pack |
| 3 | W1-S6 PR-A: staff_auth.rs + pin_lockout.rs + email_alerts.rs + audit-log | james-LEAD | foundational-auth | per-PR Captain merge auth at PR-open · evidence pack · Q-S6-8/Q-S6-9 ACCEPT-DEFAULTS extension |
| 4 | W3 PR-A: v2-db wallets.rs HRC + migration + wallet_hrc_bridge.rs + audit-log | bono-LEAD (PACT-024 family) · james AMPLIFIER | foundational-wallet | per-PR Captain merge auth · PACT-024 §2.1 path-c remaining gates Steps 3+4 |
| 5 | NEW-Q-DECISION-1: PACT-DRAFT-clippy-update-then-select-lint | TBD-first-mover | sibling-PACT | DEFER-TO-CAPTAIN-G33-NEXT-BILATERAL · default-YES |
| 6 | NEW-Q-DECISION-2: §S-155 V2 Audit-Log Doctrine | TBD-first-mover | doctrine-substrate | DEFER-TO-CAPTAIN-G33-NEXT-BILATERAL · default-YES |
| 7 | PACT-024b orphan-hold reconciler ("Kidneys") | bono-LEAD per PACT-024 family | sibling-PACT | post-W3 PR-A; immediate-follow-up timeline |
| 8 | Wave 1 PR merge sequence: W1-S5 → W1-S6 → W3 (auth foundation before wallet HRC) | bilateral | merge-orchestration | per-PR Captain merge auth + CI green + RCA evidence pack |

**Ordering rationale**: auth foundation (W1-S5+W1-S6) lands before wallet HRC (W3) because W3 wallet operations rely on staff-PIN-validated paths that W1-S5 sliding-window touches.

---

## §8 — Step 4 VERIFY readiness

Per Protocol v3.0 Step 4 mandate (≥3-model adversarial different from Steps 1-3, ≥4.0 score gate).

**Step 1 models used** (avoid in Step 4): deepseek-r1-0528 · qwen3-coder · mimo-v2-pro · gemini-2.5-flash · mistral-small-2603

**Step 4 candidate pool** (≥3 different):
- Reasoner: kimi-k2.5 · gpt-5.4-nano
- Code expert: grok-code-fast · deepseek-v3.2
- SRE/Ops: nemotron-3-super · mimo-v2-flash
- Generalist: qwen3-235b · mistral-medium · gemini-2.5-pro

**Step 4 prompt scope**:
- 5 MINORITY findings (F-MIN-1..5) — score ≥ 4.0 includes in PLAN
- Step 2 PLAN structural soundness (this document)
- Per-finding disposition consistency (no contradictions)
- NEW-Q-DECISION-1 + NEW-Q-DECISION-2 default-YES soundness
- Cross-RCA doctrine (F-CONS-10 + F-CONS-12) coherence

**Cost estimate**: ~$0.15-0.25 (3-4 models × VERIFY prompt; smaller scope than Step 1 DIAGNOSE).

---

## §9 — Wave B formal multi-model adversarial Step 2 cross-check

This Wave A is authorial substrate. Wave B (deferred to discrete authorization, ~$0.30 of $4.78 budget) performs the formal MMA Step 2 5-model PLAN-design batch per Protocol v3.0 Step 2 requirement: "5 models design fix plans for consensus findings. JSON array with actions/risk/rollback."

**Wave B inputs**: this Wave A document + §S-153 CONSENSUS findings.
**Wave B outputs**: 5-model PLAN-design batch results aggregated; deltas vs Wave A surfaced as PLAN-amendments at §11 below.
**Wave B trigger**: discrete user authorization OR autonomous execution under user-shipped G33-statement Steps 2+3+4 cascade authorization (current default per Auto Mode + class-level V2-aligned auth umbrella).

---

## §10 — NOT TESTED

- Wave B formal multi-model Step 2 5-model PLAN-design cross-check (~$0.30 deferred)
- bono AMPLIFIER absorption on this Wave A substrate
- Captain G33 disposition on NEW-Q-DECISION-1 + NEW-Q-DECISION-2 (DEFAULT-YES-extended-by-precedent until explicit; surface at next bilateral)
- Per-RCA H1 PLAN files (W1-S5-PLAN.md, W1-S6-PLAN.md, W3-PLAN.md — derive from this Wave A + Wave B amendments)
- Step 4 VERIFY adversarial run (3-4 models, ≥4.0 gate)
- Per-RCA implementation entry: W1-S5/W1-S6/W3 PR opens + evidence pack + per-PR Captain merge auth at PR view
- Pre-W1-S6-PR-A SMTP/DKIM/SPF probe (action item)
- §S-155 V2 Audit-Log Doctrine substrate ship
- PACT-DRAFT-clippy-update-then-select-lint filing
- PACT-024b orphan-hold reconciler filing
- Server .23 venue rebuild (deferred per Halo V.2 Gate 6 venue cycle — pods 1-8 OFF + Tailscale 2d-offline)
- Cloud Bono VPS parity (deferred until Wave 1 PR-A merges land)
- Concurrent-MMA-deduplication slot-collision N=4 anchor (PROMOTE deferred ≤ 2026-06-08 per §S-153.6)

---

## §11 — Wave B amendment placeholder

*This section reserved for Wave B formal multi-model Step 2 PLAN-design batch output deltas. Empty in Wave A.*

When Wave B runs:
1. Per-CONSENSUS-finding 5-model PLAN-design proposals collected
2. Cluster proposals using Step 1 fuzzy-cluster method
3. Surface DELTAS vs Wave A dispositions
4. Append to §11 with explicit accept/reject per delta
5. Update §6 W1-S5/W1-S6/W3 H1 PLAN scopes with final ship list

---

## §12 — Provenance signature

- james / 2026-05-09 ~12:25 IST · Step 2 PLAN Wave A substrate authoring
- 14 CONSENSUS dispositioned: 12 IN-PLAN class + 1 RESOLVED-BY-§S-151 + 1 DISPOSITION-RESOLVED-BY-COMPOSITION
- 2 NEW-Q-DECISIONs surfaced (DEFER-TO-CAPTAIN-G33-NEXT-BILATERAL, default-YES)
- 5 MINORITY → Step 4 VERIFY scope
- 20 SINGLETONs noted for traceability; 8 auto-absorbed by CONSENSUS dispositions
- Per-RCA H1 PLAN derivation table populated (§6)
- Implementation entry sequence ordered (§7) — auth foundation before wallet HRC
- Wave B formal multi-model adversarial cross-check deferred to discrete authorization

**Composes-with**:
- §S-153 MMA Step 1 closure (CONSENSUS source-of-truth)
- §S-152 ACCEPT-DEFAULTS pattern (`bda06dc8` Captain G33 disposition shape)
- §S-150 PR #66 first end-to-end §S-146 V1↔V2 RCA pipeline precedent (MERGED; cost-anchor $0.083)
- W2 PLAN scaffolding `4966c234` (sequencing precedent — auth before wallet)
- V1-dependent V2 RCA doctrine `8768b62` (BILATERAL; foundational-boundary RCA + per-PR merge auth class)
- user-shipped G33-statement 2026-05-09 ~12:20 IST (Steps 2+3+4 cascade authorization + class-level per-PR merge auth pre-grant for foundational-boundary auth+billing+wallet)
- Wallet Framing C LOCKED · Pod-display state-channel premise · §AMEND-3.II D12 separation doctrine

**Authorization note**: per §S-152.4 attribution-substitution discipline, Captain G33 attribution withheld in this ledger entry — user echoed the drafted statement verbatim 12:20 IST without further mediation; ledger reads "user-shipped G33-statement" not "Captain Uday G33".

— END Wave A authorial draft —

---

## §13 — Supplementary Run Absorption (SLOT-COLLISION N=4)

**Source**: `.planning/specs/v2/MMA-W1-S5-W1-S6-DIAGNOSE/SYNTHESIS.md` shipped 2026-05-09 ~12:15 IST (parallel-session james-side, slot-collision N=4 self-G9 captured at synthesis §0).

**Cost**: $0.067 of $10 supplementary batch budget (Captain G33 batch ratified 11:23 IST; separate from $4.78 remaining of session $5).

**Panel** (5 vendor-families; 3 clean + 1 partial + 1 discarded): deepseek-r1-0528 ✓ · gemini-2.5-flash ✓ · mimo-v2-pro UNUSABLE (emitted reasoning not JSON) · kimi-k2.5 PARTIAL (titles recoverable; bodies truncated) · qwen3-235b ✓.

**Disposition**: **REVISE** (3/3 clean models REVISE; zero PASS). RCAs at racecontrol `bda06dc8` (W1-S5 + W1-S6) require amendment BEFORE H1 PLAN.

### §13.1 — Newly-promoted CONSENSUS findings (3+/5 in supplementary; ≤1/5 in canonical)

#### F-CONS-15 [P0] [CROSS] Sliding-window JWT refresh BYPASSES PIN-LOCKOUT — 3/5 (security-CRITICAL)
- **Promoted-from-canonical**: F-SING-2 (canonical 1/5 mimo-v2-pro) → 3/5 in supplementary (gemini-flash + qwen3-235b + kimi-k2.5)
- **Mechanism**: staff JWT pre-lockout remains valid until natural 24h `exp`; sliding-window REFRESHES it on subsequent non-privileged requests. PIN auto-rotate + Captain freeze blocks future PIN-based logins, NOT existing sessions. **W1-S6 lockout's security intent is undermined.**
- **IN-PLAN amendment**: W1-S5 sliding-window refresh path MUST check `staff_pin_lockout_state(staff_id)` BEFORE re-issuing JWT. On lockout-active: reject refresh + revoke existing JWT (return 401 + clear cookie). Requires:
  - Persistent (or shared) "lockout-active" predicate W1-S5 middleware reads on every request
  - Revocation mechanism for existing JWT (jti denylist OR force-expire)
  - Captain Q-DECISION on cross-feature integration (NEW-Q-DECISION-3 below)
- **Cross-coupling implication**: W1-S5 + W1-S6 NO LONGER independent ships. Wave 1 sequencing topology change required.
- **PR**: cross-cutting in W1-S5 PR-A + W1-S6 PR-A; ordering implication (§7 update below)

#### F-CONS-16 [P0/P1] [W1-S5] Concurrency race in token re-issuance — 3/5
- **Promoted-from-canonical**: F-SING-5 (canonical 1/5 deepseek-r1) → 3/5 in supplementary (deepseek-r1 + qwen3-235b + kimi-k2.5)
- **Mechanism**: RCA assumes token re-issuance is atomic + side-effect-free. Two simultaneous requests from same `staff_id` could trigger duplicate re-issuance, write conflicting Set-Cookie headers, race in audit-log writes. Canonical RCA §5 sketch items 3-5 don't address concurrent-request handling.
- **IN-PLAN amendment**: W1-S5 §3 disposition + §5 implementation sketch — single-flight pattern OR idempotency-via-CSPRNG-jti for `mint_refreshed_jwt`. Test: concurrent-refresh-N-requests-yields-single-Set-Cookie-and-single-audit-log-row.
- **PR**: W1-S5 PR-A · file: `crates/racecontrol/src/auth/middleware.rs` (post-handler) + helper

#### F-CONS-17 [P1/P2] [W1-S5] Clock skew / clock drift between hosts — 3/5 (NEW; not in canonical)
- **NEW-not-in-canonical**: 3/5 in supplementary (deepseek-r1 + qwen3-235b + kimi-k2.5)
- **Mechanism**: Sliding-window check relies on `iat` and server-local `now`. RacingPoint runs racecontrol on Server .23 + Bono VPS (cloud). Tokens minted on one host evaluated against the other host's clock. Canonical RCA §2 row 8 mentions `saturating_sub` clock-skew tolerance (preserves V1 behavior) but doesn't address inter-host skew under sliding-window-refresh semantics.
- **IN-PLAN amendment**: W1-S5 §3 OPEN — bound max-allowed `iat`-skew between hosts; reject tokens where `iat > now + skew_tolerance` rather than silently treating-as-fresh. Document tolerance value (default ≤60s).
- **PR**: W1-S5 PR-A · file: middleware.rs `is_idle_expired` enum + skew-bound check + test

#### F-CONS-18 [P1] [W1-S6] EmailAlerter timeout/retry/error handling — 3/5 (NEW; not in canonical)
- **NEW-not-in-canonical**: 3/5 in supplementary (gemini-flash + qwen3-235b + kimi-k2.5)
- **Mechanism**: W1-S6 §1 reuses `EmailAlerter::send_alert` shell-out to `comms-link/shared/send-email.js`. Canonical RCA doesn't specify timeout/retry. Hanging SMTP connection blocks middleware chain. Same applies to WhatsApp Captain freeze dispatch (Evolution API hang).
- **IN-PLAN amendment**: W1-S6 §3 + §5 — wrap email + WhatsApp dispatch in `tokio::time::timeout(N_secs)` (default 5s). On dispatch failure: PIN-rotation + audit-log + lockout-counter MUST still complete. Dispatch failure is decoupled from lockout completion. Document failure-mode in code comment.
- **PR**: W1-S6 PR-A · file: `crates/racecontrol/src/email_alerts.rs::send_pin_rotation` + WhatsApp dispatch site

### §13.2 — NEW Q-DECISIONs surfaced by supplementary

#### NEW-Q-DECISION-3 (Q-W1-CROSS-1): Should W1-S5 sliding-window refresh check staff lockout-state on every refresh?
- **Source**: F-CONS-15 mitigation
- **Default**: YES per supplementary 3/5 consensus
- **Disposition path**: REQUIRES Captain explicit ratification BEFORE implementation (security-class boundary; supersedes ACCEPT-DEFAULTS-by-precedent-extrapolation per §S-152.3 — security-class explicit auth required)
- **Class**: foundational-auth boundary; per-PR Captain merge auth at PR-open expected

#### NEW-Q-DECISION-4 (Q-W1-CROSS-2): Implementation order — ship W1-S6 first → W1-S5 second? OR W1-S5+S6 combined? OR W1-S5 with no-op-lockout-check that activates when W1-S6 lands?
- **Source**: F-CONS-15 cross-coupling implication
- **Captain options**:
  - **a**: W1-S6 ships FIRST; W1-S5 ships SECOND with active lockout-check (cleanest; respects security ordering)
  - **b**: W1-S5 + W1-S6 ship as COMBINED PR (eliminates ordering question; larger blast radius per PR)
  - **c**: W1-S5 ships FIRST with no-op `lockout_check_disabled` cfg flag; activates when W1-S6 lands (deploy-order independence; risk = forgetting to flip flag)
- **Disposition path**: REQUIRES Captain explicit ratification (Wave 1 sequencing topology decision)
- **Class**: Wave-1-orchestration; affects PR open + merge sequencing

#### NEW-Q-DECISION-5 (Q-W1-S5-NEW-1): Max-session-life cap on sliding-window?
- **Source**: supplementary §6 single-voice-but-architecturally-important — gemini-flash 1/5
- **Mechanism**: Without max-session-life cap, an active staff member's session can extend INDEFINITELY as long as activity continues. RCA §1 Captain §S-82 Q3 sliding-window-vs-fixed-window dispositioned for 30-min idle-timeout but does NOT address cumulative session-life ceiling.
- **Captain options**:
  - **a**: NO cap (current sliding-window semantics; active staff stay logged in indefinitely; Captain-intent for staff convenience)
  - **b**: HARD cap at N hours since `iat_original` (e.g., 12h or 24h); refresh fails; re-PIN required
  - **c**: SOFT cap with re-PIN prompt (UX-preferred but multi-component)
- **Default**: TBD per Captain (intent vs security balance)
- **Disposition path**: REQUIRES Captain explicit ratification at next bilateral

### §13.3 — Disposition state change

Pre-supplementary (this Wave A authored): "Step 2 PLAN Wave A authored against canonical 14 CONSENSUS; Wave B formal multi-model deferred."

Post-supplementary: **"Step 2 PLAN Wave A is REFERENCE-ONLY. Per supplementary §7 recommended workflow, W1-S5 + W1-S6 RCAs require amendment FIRST before re-running MMA Step 1 (~$0.07; 4 consensus items resolved → expect PASS/REVISE-downgrade); after PASS, Step 2 PLAN re-author absorbs amended-RCA findings. THEN H1 PLAN derivation."**

**2026-05-09 ~19:15 IST disposition update**: Captain Option 4 zero-spend substrate path 12:49 IST bypassed re-Step-1 MMA spend; PR #67 `7dcedd00` 19:11 IST amended W1-S5 + W1-S6 RCAs absorbing all 4 promoted CONSENSUS + 4 Captain Q-DECISIONs (Q-W1-CROSS-1 + Q-W1-CROSS-2 + Q-W1-S5-NEW-1 + Q-S5-NEW-2). Wave A.2 SHIPPED at racecontrol pending-commit-this-author-turn (file: `MMA-STEP-2-W1S5-W1S6-W3-PLAN-A2.md`); supersedes this Wave A. RCA-amendment cycle CLOSED. Wave A.2 is the substantive successor referenced above.

W3 RCA at `78f82654` is NOT touched by supplementary run (W1-S5 + W1-S6 only). W3 dispositions in §2 (F-CONS-2, F-CONS-8, F-CONS-11, F-CONS-13, F-CONS-14) remain valid; W3 PR-A can proceed on its own track.

### §13.4 — Updated implementation entry sequence (§7 amendment)

**Replaces §7 ordering**:

| # | Action | Owner | Class | Gates |
|---|---|---|---|---|
| 1 | **Amend W1-S5 + W1-S6 RCAs** to absorb F-CONS-15..18 (4 items) + Captain Q-DECISIONs Q-W1-CROSS-1..2 + Q-W1-S5-NEW-1 | next-mover-LEAD | RCA-amendment-class | NONE — substrate-ship |
| 2 | **Re-run MMA Step 1** on amended RCAs (~$0.07) | running-pilot | substrate-ship | next iteration ≥ PASS or REVISE-downgrade |
| 3 | **Step 2 PLAN re-author** (Wave A.2) absorbing amended-RCA + re-Step-1 CONSENSUS | next-mover-LEAD | substrate-ship | gates on Step 1 PASS gate |
| 4 | Pre-W1-S6-PR-A SMTP/DKIM/SPF probe (F-CONS-6) | james | substrate-ship | NONE — ACTION |
| 5 | **W1-S6 PR-A FIRST** (per Q-W1-CROSS-2 default-a): staff_auth.rs + pin_lockout.rs + email_alerts.rs + audit-log + persistent lockout-state-predicate that W1-S5 will read | james-LEAD | foundational-auth | per-PR Captain merge auth at PR-open · evidence pack · Q-S6-disposition + Q-W1-CROSS-* |
| 6 | **W1-S5 PR-A SECOND**: middleware.rs + PHASE-1-WAVE-1-PLAN.md amend + audit-log + LOCKOUT-CHECK-ON-REFRESH + JWT-revocation-mechanism + concurrency-single-flight + clock-skew-bound | james-LEAD | foundational-auth | per-PR Captain merge auth · gates on W1-S6 PR-A merge first per Q-W1-CROSS-2-a |
| 7 | W3 PR-A: v2-db wallets.rs HRC + migration + wallet_hrc_bridge.rs + audit-log | bono-LEAD (PACT-024 family) · james AMPLIFIER | foundational-wallet | per-PR Captain merge auth · PACT-024 §2.1 Steps 3+4 · INDEPENDENT-OF-W1-S5-S6-ORDERING |
| 8 | NEW-Q-DECISION-1: PACT-DRAFT-clippy-update-then-select-lint | TBD | sibling-PACT | Captain G33 next bilateral · default-YES |
| 9 | NEW-Q-DECISION-2: §S-155 V2 Audit-Log Doctrine | TBD | doctrine-substrate | Captain G33 next bilateral · default-YES |
| 10 | PACT-024b orphan-hold reconciler ("Kidneys") | bono-LEAD | sibling-PACT | post-W3 PR-A immediate-follow-up |
| 11 | Wave 1 PR merge sequence: W1-S6 → W1-S5 → W3 (auth-lockout foundation BEFORE auth-refresh integration BEFORE wallet HRC) | bilateral | merge-orchestration | per-PR Captain merge auth + CI green + RCA evidence pack |

**Ordering rationale change**: pre-supplementary I had W1-S5 → W1-S6 → W3. Post-supplementary the security-critical F-CONS-15 cross-coupling forces W1-S6 → W1-S5 (so persistent lockout-state-predicate exists for W1-S5 refresh-path lockout-check). W3 remains last and is order-independent vs S5/S6 swap.

### §13.5 — Updated NOT TESTED (replaces §10)

Items now NOT TESTED (post-supplementary):

- W1-S5 + W1-S6 RCA amendment cycle (per supplementary §7.1)
- Re-run MMA Step 1 on amended RCAs (~$0.07; PASS/REVISE-downgrade gate)
- Captain G33 disposition on Q-W1-CROSS-1 (lockout-check-on-refresh; security-class explicit)
- Captain G33 disposition on Q-W1-CROSS-2 (W1-S5+S6 ordering — a/b/c)
- Captain G33 disposition on Q-W1-S5-NEW-1 (max-session-life cap)
- Captain G33 disposition on NEW-Q-DECISION-1 (clippy lint) + NEW-Q-DECISION-2 (audit-log doctrine) — DEFER-default-YES still standing
- Wave A.2 Step 2 PLAN re-author absorbing amended-RCA CONSENSUS
- Step 4 VERIFY adversarial run on Wave A.2 PLAN
- Per-RCA H1 PLAN files (gates on Step 2 Wave A.2 + Step 4 VERIFY PASS)
- Per-RCA implementation entry: W1-S6 PR-A (FIRST per Q-W1-CROSS-2-a) → W1-S5 PR-A → W3 PR-A
- Pre-W1-S6-PR-A SMTP/DKIM/SPF probe (action item)
- §S-155 V2 Audit-Log Doctrine substrate ship
- PACT-DRAFT-clippy-update-then-select-lint filing
- PACT-024b orphan-hold reconciler filing
- Server .23 venue rebuild (deferred per Halo V.2 Gate 6 venue cycle)
- Cloud Bono VPS parity (deferred until Wave 1 PR-A merges land)
- Slot-collision N=4 PROMOTE structural-fix candidate (pre-MMA hook grep ledger; ≤ 2026-06-08 default-anchor; per supplementary §11)

### §13.6 — Provenance amendment

james-side parallel-session shipped supplementary at 12:12 IST during my Wave A authoring (12:25 IST). Self-G9 captured at supplementary §0: Verify-Before-Generate violation (did not grep `LOGBOOK.md` or `openrouter-spend-james.jsonl` for recent MMA runs before invoking). Supplementary's net signal value (CROSS-1 PIN-LOCKOUT-bypass promoted from 1/5 to 3/5) made the duplicate-spend ($0.067) cost-justified post-hoc despite discipline violation pre-hoc.

This Wave A absorbs supplementary findings via §13 amendment rather than full re-author. Wave A.2 (post-RCA-amendment + re-Step-1) is the substantive successor; this Wave A is reference-only for design-history-trail.

— END §13 supplementary absorption —

