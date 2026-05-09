# MMA Step 2 PLAN — Wave A.2 — W1-S5 + W1-S6 + W3 RCA Triplet (substantive successor to Wave A)

**Scope**: W1-S5 + W1-S6 + W3 RCA triplet (foundational-boundary class — auth + auth + wallet)

**Authored**: 2026-05-09 ~19:15 IST · **Authored-by**: james (Claude Opus 4.7 1M)
**Substrate-class**: foundational-boundary triplet PLAN — substantive successor
**Status**: DRAFT-AWAITING-CAPTAIN-G33-Q-DECISIONS-AND-WAVE-B-OPTIONAL

**Supersedes**:
- `MMA-STEP-2-W1S5-W1S6-W3-PLAN.md` (Wave A canonical, racecontrol `8f512d29`) — its §13.3 explicit promise: "Wave A.2 (post-RCA-amendment + re-Step-1) is the substantive successor; this Wave A is reference-only for design-history-trail." This Wave A.2 fulfills that promise.
- `MMA-STEP-2-W1S5-W1S6-W3-PLAN-SUPPLEMENTARY-N2.md` (N=2 SUPPLEMENTARY-DUPLICATE-RUN-N=2, racecontrol `c3640229`) — promoted to corroboration source-of-PR-breakdown; baseline highest-coverage 16/16 from `mistralai/mistral-small-2603` (score 207) preserved as Step 3 EXECUTE candidate.

**Authoritative substrate**:
- `W1-S5-RCA.md` (post-PR #67 `7dcedd00`) — absorbed F-CONS-15, F-CONS-16, F-CONS-17 + Q-W1-CROSS-1, Q-W1-CROSS-2, Q-W1-S5-NEW-1, Q-S5-NEW-2
- `W1-S6-RCA.md` (post-PR #67 `7dcedd00`) — absorbed F-CONS-15 (cross), F-CONS-18 + Q-W1-CROSS-1, Q-W1-CROSS-2
- `W3-WALLET-HRC-RCA.md` (`78f82654`) — unchanged by PR #67 (W3 not touched)
- `MMA-STEP-1-W1S5-W1S6-W3-CONSENSUS.md` (`f599c316`) — 14 canonical CONSENSUS Step 1 inputs
- `MMA-W1-S5-W1-S6-DIAGNOSE/SYNTHESIS.md` — supplementary N=4 promoted findings (now substrate via PR #67)

**Path traversed**:
1. Wave A authored `8f512d29` 12:25 IST against canonical 14 CONSENSUS
2. Slot-collision N=4 hit at 12:12 IST — supplementary run promoted F-CONS-15..18 + 3 Q-DECISIONs
3. N=2 supplementary `c3640229` 12:55 IST — corroborated PR breakdown PR-A..PR-E + mistral 16/16 baseline
4. Captain Option 4 zero-spend substrate path 12:49 IST — bypasses re-Step-1 MMA spend
5. PR #67 OPENED 13:25 IST `21848746` → MERGED 19:11 IST `7dcedd00` — RCAs absorbed all 4 promoted CONSENSUS + 4 Q-DECISIONs
6. Wave A.2 (this doc) re-author 19:15 IST — substrate consolidation; integrates §13 amendment as primary; no MMA spend

**Authorization chain**: user-shipped G33-statement 12:20 IST (Steps 2+3+4 cascade authorization + class-level per-PR merge auth pre-grant for foundational-boundary auth+billing+wallet) → Captain Option 4 12:49 IST (zero-spend substrate-only RCA absorption) → PR #67 merged 19:11 IST closes RCA amendment gate → Wave A.2 author authorized as final Step 2 substrate before per-RCA H1 PLAN derivation.

**V2 doctrine alignment**: §S-146 V1↔V2 RCA rule SECOND end-to-end pipeline application · §AMEND-3.II D12 Foundation/Strategy/Config separation · Wallet Framing C source-tag preservation · F-05 anti-pattern codification candidate · §S-159 pre-mma-duplicate-check hook (slot-collision class N=5 PROMOTE-COMPLETE-IMPLEMENTED).

**Spend posture**: Wave A.2 = $0 OpenRouter (synthesis-only re-author; no model calls). Cumulative MMA-day spend baseline at `7dcedd00`: ~$0.405 of $5 session budget (Step 1 $0.1065 + N=4 supp $0.067 + N=2 supp $0.0457 + prior session amortized).

---

## §1 — Step 1 inputs absorbed (delta vs Wave A)

| Class | Wave A count | Wave A.2 count | Delta |
|---|---|---|---|
| CONSENSUS (≥3/5) | 14 | **18** | +4 promoted via §13/PR #67 absorption |
| MINORITY (2/5) | 5 | 5 | unchanged |
| SINGLETON (1/5) | 20 | 16 | -4 (4 SINGLETONs promoted to CONSENSUS via supp run) |

**Source**: `.planning/specs/v2/MMA-STEP-1-W1S5-W1S6-W3-CONSENSUS.md` (713 lines, `f599c316`) + `.planning/specs/v2/MMA-W1-S5-W1-S6-DIAGNOSE/SYNTHESIS.md` supplementary findings absorbed via amended RCAs at PR #67.

**Promoted findings** (1/5 in canonical → 3/5 in supplementary, now CONSENSUS):
- F-CONS-15 promoted-from F-SING-2 (mimo-v2-pro 1/5 → gemini+qwen+kimi 3/5)
- F-CONS-16 promoted-from F-SING-5 (deepseek-r1 1/5 → r1+qwen+kimi 3/5)
- F-CONS-17 NEW from supplementary (not in canonical)
- F-CONS-18 NEW from supplementary (not in canonical)

---

## §2 — Per-finding disposition (CONSENSUS, 18 items)

Disposition codes: **IN-PLAN** · **IN-PLAN-WITH-DOC** · **IN-PLAN-AS-PRECONDITION** · **IN-PLAN-AS-CROSS-RCA-DOCTRINE** · **DEFER** · **RESOLVED-BY-PRIOR-§S-N**

### F-CONS-1 [P0] [W1-S5] create_staff_jwt cashier-default role-downgrade — IN-PLAN
- **Mitigation**: Extract `mint_refreshed_jwt(claims) -> Result<String>` helper using `claims.role` explicitly via `create_staff_jwt_with_role(claims.role)`. Never default to cashier role on refresh.
- **Test**: `manager_superadmin_role_preserved_across_100_refresh_cycles` [integration]
- **Lint candidate** (NEW-Q-1 sibling): forbid direct `create_staff_jwt` in new auth-refresh code
- **PR**: PR-C-W1-S5-auth-refresh · file: `crates/racecontrol/src/auth/middleware.rs:144-161,251-273` + `auth.rs` helper extraction
- **Cross-model coverage**: 3/3 (qwen + gemini + mistral)

### F-CONS-2 [P0] [W3] F-05 UPDATE-then-SELECT-same-column anti-pattern in capture path — IN-PLAN
- **Mitigation**: `WalletService::capture` snapshot-read BEFORE any wallet UPDATE; compute delta from snapshot; never read `balance_credits` after UPDATE. Code comment referencing `ROOT-CAUSE-ANALYSIS-F05-2026-03-28.md` + ₹162.50 incident.
- **Test**: `f05_anti_pattern_regression_check_capture_path` [regression]
- **NEW-Q-1 sibling**: codify `clippy::update_then_select_same_column` lint
- **PR**: PR-A-W3-schema · file: `crates/v2-db/src/wallets.rs::WalletService::capture` + `tests/wallet_capture.rs:110-130`
- **Cross-model coverage**: 3/3 (qwen + gemini + mistral); supplementary corroboration P0 confidence 0.98

### F-CONS-3 [P2] [W1-S5] PHASE-1-WAVE-1-PLAN.md '7-min' vs '30-min' — IN-PLAN
- **Mitigation**: Amend `PHASE-1-WAVE-1-PLAN.md` rows 21+33 from "7-min fixed-window" to "30-min sliding-window" in W1-S5 ship commit; code comment referencing Captain G33 Q-S5-7 ACCEPT-DEFAULT (plan-author typo class).
- **Test**: assert `idle_timeout_secs=1800` in sliding-window path (config-parity regression test)
- **PR**: PR-C-W1-S5-auth-refresh · file: `.planning/PHASE-1-WAVE-1-PLAN.md:21,33` + middleware.rs comment
- **Cross-model coverage**: 3/3 (qwen + gemini + mistral)

### F-CONS-4 [P0] [W1-S6] V1 IP-keyed rate-limit unusable for per-staff-id semantics — IN-PLAN
- **Mitigation**: NEW per-staff-id primitive in `staff_auth.rs::PinLockoutTracker` with `ResetState { count: u32, window_start: DateTime<Utc> }`. Inline impl per Captain Q-S6-2 Option A ACCEPT-DEFAULT. Do NOT extend `tower_governor::PeerIpKeyExtractor`.
- **Composes-with Q-S6-8 FLAG-1** (DEFAULT Option C): extract `pin_lockout` module called from the 3 staff-PIN-validate endpoints listed in `rate_limit.rs:12-13`.
- **Test**: `per_staff_id_isolation_two_staff_same_ip_independent_counters` [integration]
- **PR**: PR-D-W1-S6-pin-lockout · NEW file: `crates/racecontrol/src/auth/staff_auth.rs` + `crates/racecontrol/src/auth/pin_lockout.rs`
- **Cross-model coverage**: 3/3

### F-CONS-5 [P1] [W1-S6] EmailAlerter unbounded HashMap growth — DISPOSITION-RESOLVED-BY-CONS-7
- **Mitigation**: Captain Q-S6-1 ACCEPT-DEFAULT (per §S-148.4 batch) bypasses cooldown HashMap entirely for PIN-rotate event-class. Bypass eliminates the unbounded-growth axis for staff-id keys (no staff-id keys ever inserted into HashMap). Per-pod HashMap unbounded-growth is V1-pre-existing (out-of-scope of W1-S6 mitigation per kaizen-min).
- **Defer-rationale**: per-pod HashMap pruning is V1 footgun separate from W1-S6 scope; sibling PACT candidate post-Wave-1 if V1 EmailAlerter remains in V2.
- **PR**: NONE — disposition resolved by F-CONS-7 mitigation pattern

### F-CONS-6 [P1] [W1-S6] SMTP transport + DKIM/SPF unverified — IN-PLAN-AS-PRECONDITION
- **Mitigation**: Pre-W1-S6-PR-A probe action — verify at Server .23 + Bono VPS:
  - `which sendmail` / `tail /var/log/mail.log`
  - `dig +short TXT racingpoint.in` (SPF)
  - `dig +short TXT default._domainkey.racingpoint.in` (DKIM)
  - Google Workspace API auth scope (if needed)
- **Outcome branch**:
  - GREEN (sendmail/SMTP available + DKIM/SPF set): proceed to W1-S6 PR-A
  - YELLOW (sendmail available; DKIM/SPF absent): surface Captain Q-DECISION
  - RED (no transport): block W1-S6 PR-A; surface Captain Q-DECISION immediately
- **PR**: ACTION-PRE-W1-S6 · gates substrate ship; no code
- **Cross-model coverage**: 3/3 (qwen + gemini + mistral); 3/5 surface Captain Q-DECISION on outcome

### F-CONS-7 [P1] [W1-S6] V1 cooldown semantics conflict with event-class — IN-PLAN
- **Mitigation**: NEW sibling method `EmailAlerter::send_pin_rotation(staff_id, new_pin) -> Result<()>` that bypasses cooldown HashMap entirely. Method comment: "PIN-rotate emails are event-class; always deliver per Captain Q-S6-1. Bypasses per-pod 1800s + venue-wide 300s alert-class cooldown by design."
- **Test**: `pin_rotation_email_delivered_even_when_cooldown_blocks_alert_class` [unit]
- **Composes-with F-CONS-5**: this mitigation eliminates F-CONS-5 (no staff-id keys inserted into cooldown HashMap)
- **PR**: PR-D-W1-S6-pin-lockout · file: `crates/racecontrol/src/email_alerts.rs:69-83`

### F-CONS-8 [P1] [W3] wallet_redemptions hold_id column — IN-PLAN
- **Mitigation**: Migration `ALTER TABLE wallet_redemptions ADD COLUMN hold_id TEXT REFERENCES wallet_holds(id)` (W3 migration .sql). Update `WalletService::capture` to populate column from active hold. Add migration index for forensic queries.
- **Test**: `bonus_source_tag_preserved_through_hrc_capture` — assert hold_id populated through full hold→capture→redemption path
- **Doctrine alignment**: Wallet Framing C source-tag preservation invariant + PACT-024 §3 Q5 5-AGREE recommendation
- **PR**: PR-A-W3-schema · file: `crates/v2-db/migrations/<datestamp>_wallet_redemptions_add_hold_id.sql` + `crates/v2-db/src/wallets.rs::capture`
- **Sibling-PACT-024a**: re-target paths per F-CONS-13
- **Cross-model coverage**: 3/3 unanimous PR-A-W3-schema assignment

### F-CONS-9 [P2] [W1-S6] In-memory lockout state durability per CR-3 — IN-PLAN-WITH-DOC
- **Mitigation**: Accept restart-forgiveness per Captain Q-S6-6 ACCEPT-DEFAULT + CR-3 customer-service-priority. Code comment in `pin_lockout.rs` module-level: `//! In-memory only; restart-after-5-wrong acceptable per CR-3. DB-backed deferred to V2.1 if abuse pattern emerges.` Add Prometheus counter `auth.pin_lockout_restart_count` for observability without behavior change.
- **Test**: doc-test that asserts comment present (kaizen-min)
- **Defer-to-V2.1-trigger**: abuse pattern (≥3 lockout-restart-resets/30d at single staff_id)
- **PR**: PR-D-W1-S6-pin-lockout · file: `crates/racecontrol/src/auth/pin_lockout.rs` (header) + `metrics.rs` counter

### F-CONS-10 [P0] [CROSS] Auth boundary changes preserve audit_log schema + state-machine consistency — IN-PLAN-AS-CROSS-RCA-DOCTRINE
- **Mitigation**: Define unified audit_log `action_type` vocabulary across triplet:
  - W1-S5: NO routine logging on JWT refresh (per Q-S5-disposition + finding F-SING-12 ~100x volume risk); LOG ONLY 401 idle-expiry rejections (action_type = `staff_jwt_idle_expiry`)
  - W1-S6: action_type = `staff_pin_auto_reset` (per PIN-rotate event)
  - W3: action_type ∈ {`wallet_hold_created`, `wallet_hold_captured`, `wallet_hold_released`}
- **State-machine consistency invariant**: HOLDs are session-bound, NOT auth-bound. Release fires only on session terminal-state (game-running stop / launch-fail / staff cancel), NOT staff JWT expiration. Per Captain Q-W3-13 default.
- **Cross-RCA regression test**: `triplet_audit_log_action_types_distinct_no_schema_conflict` + `holds_session_bound_not_auth_bound`
- **PR**: PR-E-cross-RCA-audit-log-doctrine · scattered touches at audit_log call sites + state-machine boundary
- **Composes-with F-CONS-12**: V2 Audit-Log Doctrine
- **Cross-model coverage**: 3/3

### F-CONS-11 [P0] [W3] PACT-024 Q1-Q5 dispositions outstanding — RESOLVED-BY-§S-151
- **Status**: PACT-024 already AMPLIFIED-2026-05-05 via canonical batch `c5529c1` msg=35096 with bono Q1-a/Q2-c/Q3-c/Q4-c/Q5-c 5-AGREE recommendations adopted. Stale-claim self-G9 absorbed at §S-152.5.
- **Net**: gate already cleared. **No further action required.**
- **PR**: NONE
- **Provenance note**: this finding surfaced in MMA Step 1 because individual model context lacked §S-151 self-G9 corrective. Step 2 disposition documents the resolution-by-prior-§S-N pattern for future MMA Step 1 audits.

### F-CONS-12 [P1] [CROSS] Inconsistent Audit-Log Discipline — IN-PLAN-AS-DOCTRINE
- **Mitigation**: Establish V2 Audit-Log Doctrine (NEW-Q-2 candidate). Principles:
  - LOG: state-changing events (PIN-rotate, hold/release/capture, wallet writes, staff role changes, security boundary 401s)
  - DO NOT LOG: routine heartbeat events (JWT refresh, periodic re-fetches, health probes, game-running heartbeats)
  - Schema: `action_type` enum-class enforced by lint or test
  - Volume gate: per-event-type rate alarm > 100/min/staff
- **NEW-Q-2**: ship as `comms-link/V2-MASTER-STATE.md §S-N` doctrine append (parallel or post-triplet). DEFER-TO-CAPTAIN-G33-NEXT-BILATERAL with default-YES per ACCEPT-DEFAULTS-by-precedent-extrapolation.
- **PR**: PR-E-cross-RCA-audit-log-doctrine · file: `comms-link/V2-MASTER-STATE.md` §S-N
- **Composes-with F-CONS-10**: this is the doctrine layer; F-CONS-10 is the in-PR per-action-type implementation

### F-CONS-13 [P1] [W3] PACT-024a §A re-target to v2-db crate — IN-PLAN
- **Mitigation**: Re-target PACT-024a §A file paths from `crates/racecontrol/src/wallet.rs` + `billing.rs` + `routes.rs` to `crates/v2-db/src/wallets.rs` + `crates/v2-db/src/idempotency.rs`. SQL migration target `v2-db` schema. TODO comment referencing original PACT-024a commit for audit trail. NO semantic change — paths-only bump per Captain Q-W3-RECONCILE-1 ACCEPT-DEFAULTS extending to W3 11-Q batch.
- **Test**: NONE (paths-only; semantic equivalence ensured by F-CONS-2 + F-CONS-8 tests)
- **PR**: PR-A-W3-schema · file: `comms-link/PACTS.md` PACT-024a entry update + W3 implementation files
- **Cross-model coverage**: 2/3 (qwen + mistral); gemini DEFER

### F-CONS-14 [P2] [W3] Orphan hold cleanup deferred to PACT-024b — IN-PLAN-WITH-DOC + SIBLING-PACT
- **Mitigation**:
  - W3 PR-A: document orphan-hold deferral in W3 spec; add `hold_timeout_secs` config (default deferred per Q-W3-DEFAULT); allow staff manual `release_hold` for orphan cases.
  - **Monitoring**: alert if open holds older than 24h accumulate (Prometheus + WhatsApp staff alert).
  - **PACT-024b**: file as immediate follow-up sibling-PACT for time-based hold-expiration sweep ("Kidneys reconciliation worker"). Owner: bono-LEAD per first-mover precedent on PACT-024 family.
- **Test**: `orphan_hold_24h_alert_fires` + `staff_manual_release_hold_unblocks_customer_credits`
- **PR**: PR-A-W3-schema doc + monitoring + sibling PACT-024b filing as separate substrate ship

### F-CONS-15 [P0] [CROSS] Sliding-window JWT refresh BYPASSES PIN-LOCKOUT — IN-PLAN-AS-CROSS-RCA-DOCTRINE [SECURITY-CRITICAL]
- **Promoted-from-canonical**: F-SING-2 (canonical 1/5 mimo-v2-pro) → 3/5 supplementary (gemini-flash + qwen3-235b + kimi-k2.5)
- **Absorbed-via-PR-#67**: substrate added to W1-S5-RCA + W1-S6-RCA per Captain Option 4 zero-spend path
- **Mechanism**: staff JWT pre-lockout remains valid until natural 24h `exp`; sliding-window REFRESHES it on subsequent non-privileged requests. PIN auto-rotate + Captain freeze blocks future PIN-based logins, NOT existing sessions. **W1-S6 lockout's security intent is undermined by W1-S5 refresh path.**
- **Mitigation**: W1-S5 sliding-window refresh path MUST check `staff_pin_lockout_state(staff_id)` BEFORE re-issuing JWT. On lockout-active: reject refresh + revoke existing JWT (return 401 + clear cookie). Implementation requires:
  - Persistent (or shared) "lockout-active" predicate that W1-S5 middleware reads on every refresh request — published by W1-S6 (`PinLockoutTracker::is_locked(staff_id) -> bool`)
  - Revocation mechanism for existing JWT — Captain Q-S5-NEW-2 default a: force-expire (downgrade `iat` such that idle-window check fails)
  - Composes-with F-CONS-9: in-memory PinLockoutTracker means restart resets predicate; refresh-on-restart will not re-block a locked staff until they next attempt PIN; acceptable per CR-3 + Q-W1-S5-NEW-1 default
- **Cross-coupling implication**: W1-S5 + W1-S6 NO LONGER independent ships. Wave 1 sequencing topology change required. **W1-S6 PR-A merges FIRST per Captain Q-W1-CROSS-2 default-a.**
- **Test**: `staff_jwt_refresh_rejected_when_pin_lockout_active` [integration] + `staff_jwt_revoked_on_lockout_transition` [integration]
- **PR**: cross-cutting in PR-D-W1-S6-pin-lockout (publisher) + PR-C-W1-S5-auth-refresh (consumer); ordering implication codified at §7
- **Captain blocker**: Q-W1-CROSS-1 (security-class explicit ratification required; default-YES is informational, not auto-pass per §S-152.3)

### F-CONS-16 [P0/P1] [W1-S5] Concurrency race in token re-issuance — IN-PLAN
- **Promoted-from-canonical**: F-SING-5 (canonical 1/5 deepseek-r1) → 3/5 supplementary (deepseek-r1 + qwen3-235b + kimi-k2.5)
- **Absorbed-via-PR-#67**: W1-S5-RCA §3 + §5 amended
- **Mechanism**: W1-S5 RCA assumes token re-issuance is atomic + side-effect-free. Two simultaneous requests from same `staff_id` could trigger duplicate re-issuance, write conflicting Set-Cookie headers, race in audit-log writes.
- **Mitigation**: W1-S5 PR-A — single-flight pattern via `tokio::sync::Mutex<HashMap<staff_id, Arc<OnceCell<Result<JWT>>>>>` OR idempotency-via-CSPRNG-jti for `mint_refreshed_jwt`. Decision per Captain Q-S5-NEW-3 (NOT YET surfaced in PR #67; flag for next bilateral).
- **Test**: `concurrent_refresh_N_requests_yields_single_set_cookie_and_single_audit_log_row` [integration]
- **PR**: PR-C-W1-S5-auth-refresh · file: `crates/racecontrol/src/auth/middleware.rs` (post-handler) + helper

### F-CONS-17 [P1/P2] [W1-S5] Inter-host clock-skew under sliding-window — IN-PLAN
- **NEW-not-in-canonical**: 3/5 supplementary (deepseek-r1 + qwen3-235b + kimi-k2.5)
- **Absorbed-via-PR-#67**: W1-S5-RCA §3 OPEN added
- **Mechanism**: Sliding-window check relies on `iat` and server-local `now`. RacingPoint runs racecontrol on Server .23 + Bono VPS (cloud). Tokens minted on one host evaluated against the other host's clock. Canonical RCA §2 row 8 mentions `saturating_sub` clock-skew tolerance (preserves V1 behavior) but doesn't address inter-host skew under sliding-window-refresh semantics.
- **Mitigation**: W1-S5 PR-A — bound max-allowed `iat`-skew between hosts; reject tokens where `iat > now + skew_tolerance` rather than silently treating-as-fresh. Default `max_iat_skew_secs=60`. Document tolerance value in `config/services.rs` + `docs/ops/ntp-discipline.md`.
- **Test**: `iat_skew_rejected_beyond_60s_tolerance` [unit] + `jwt_iat_skew_tolerance_60s` [integration]
- **PR**: PR-C-W1-S5-auth-refresh · file: `crates/racecontrol/src/auth/middleware.rs::is_idle_expired` + skew-bound check + test

### F-CONS-18 [P1] [W1-S6] EmailAlerter timeout/retry/error handling — IN-PLAN
- **NEW-not-in-canonical**: 3/5 supplementary (gemini-flash + qwen3-235b + kimi-k2.5)
- **Absorbed-via-PR-#67**: W1-S6-RCA §3 + §5 amended
- **Mechanism**: W1-S6 §1 reuses `EmailAlerter::send_alert` shell-out to `comms-link/shared/send-email.js`. Canonical RCA doesn't specify timeout/retry. Hanging SMTP connection blocks middleware chain. Same applies to WhatsApp Captain freeze dispatch (Evolution API hang).
- **Mitigation**: W1-S6 PR-A — wrap email + WhatsApp dispatch in `tokio::time::timeout(N_secs)` (default 5s). On dispatch failure: PIN-rotation + audit-log + lockout-counter MUST still complete. Dispatch failure is **decoupled** from lockout completion. Document failure-mode in code comment.
- **Test**: `email_dispatch_timeout_does_not_block_lockout_completion` [integration] + `whatsapp_dispatch_timeout_does_not_block_lockout_completion` [integration]
- **PR**: PR-D-W1-S6-pin-lockout · file: `crates/racecontrol/src/email_alerts.rs::send_pin_rotation` + WhatsApp dispatch site

---

## §3 — Captain G33 Q-DECISIONs (5 surfaced; CRITICAL — PLAN halts on these)

| # | ID | Source | Class | Default | Disposition path |
|---|----|--------|-------|---------|------------------|
| 1 | **NEW-Q-1** | F-CONS-2 (F-05 lint) | Standing-Rule sub-PACT | YES (PACT-DRAFT-clippy-update-then-select-lint) | DEFER-TO-CAPTAIN-G33-NEXT-BILATERAL · ACCEPT-DEFAULTS-by-precedent (§S-152.3) |
| 2 | **NEW-Q-2** | F-CONS-10 + F-CONS-12 (V2 audit-log doctrine) | doctrine-substrate | YES (V2-MASTER-STATE §S-N append) | DEFER-TO-CAPTAIN-G33-NEXT-BILATERAL · ACCEPT-DEFAULTS-by-precedent |
| 3 | **Q-W1-CROSS-1** | F-CONS-15 (lockout-check-on-refresh) | foundational-auth security | YES | **CAPTAIN EXPLICIT REQUIRED** — security-class supersedes ACCEPT-DEFAULTS per §S-152.3 |
| 4 | **Q-W1-CROSS-2** | F-CONS-15 (Wave 1 ordering) | Wave-1-orchestration | a (W1-S6 FIRST → W1-S5 SECOND) | **CAPTAIN EXPLICIT REQUIRED** — affects PR open + merge sequencing |
| 5 | **Q-W1-S5-NEW-1** | supplementary §6 (max-session-life cap) | foundational-auth UX/security | TBD | **CAPTAIN EXPLICIT REQUIRED** — intent-vs-security balance |
| 6 | **Q-S5-NEW-2** | F-CONS-15 mitigation (JWT revocation method) | foundational-auth | a (force-expire) | DEFER-TO-CAPTAIN — security-class secondary; default acceptable for Step 3 EXECUTE start |

**Q-W1-CROSS-1, Q-W1-CROSS-2, Q-W1-S5-NEW-1 are HARD blockers for H1 PLAN derivation.** PR opens halt until Captain ratifies these three.

---

## §4 — MINORITY findings deferred to Step 4 VERIFY (5)

Listed for traceability. Step 4 VERIFY adversarial models (≥3 different from Steps 1-3, ≥4.0 score gate) evaluate inclusion in PR scope.

| # | RCA | Sev | Title | Step 4 prompt focus |
|---|---|---|---|---|
| F-MIN-1 | W1-S5 | P0 | V2.1 PACT pin file stale after pull-forward | doc-amendment scope confirmation |
| F-MIN-2 | W3 | P0 | Concurrent hold atomicity hole under SQLite single-writer | optimistic-retry pattern soundness |
| F-MIN-3 | W1-S5 | P1 | Response-mutating middleware layer precedent risk | anti-precedent comment sufficiency |
| F-MIN-4 | W3 | P1 | Undefined Refund-During-HOLD Interaction | ordering invariant correctness |
| F-MIN-5 | W1-S5 | P1 | New idle-refresh cookie may collide with existing staff-PIN cookie | cookie-name uniqueness verification |

If Step 4 score ≥ 4.0 each: include in PR scope. F-MIN-1 + F-MIN-3 + F-MIN-5 likely auto-pass (already-aligned with Captain Q-S5 dispositions). F-MIN-2 + F-MIN-4 require concrete state-machine correctness review.

---

## §5 — SINGLETON traceability (16, post-promotion)

4 SINGLETONs promoted to CONSENSUS via supplementary (F-SING-2, F-SING-5, plus F-CONS-17, F-CONS-18 NEW). 16 remain.

| # | RCA | Sev | Title | Model |
|---|---|---|---|---|
| F-SING-1 | CROSS | P0 | Auth blast-radius overlaps with wallet state machine | qwen3-coder |
| F-SING-3 | CROSS | P0 | V1↔V2 Bridge Module Class Consistency and Auditability | gemini-2.5-flash |
| F-SING-4 | W1-S5 | P0 | Sliding-window token refresh introduces UPDATE-then-SELECT-same-column anti-pattern if not guarded | mistral-small-2603 |
| F-SING-6 | W3 | P1 | V1↔V2 event bridge single-point failure | deepseek-r1-0528 |
| F-SING-7 | CROSS | P1 | Asymmetric V1↔V2 bridge implementations | deepseek-r1-0528 |
| F-SING-8 | CROSS | P1 | Idempotency key scope collision | deepseek-r1-0528 |
| F-SING-9 | W1-S5 | P1 | JWT secret rotation grace + sliding-window refresh interaction | mimo-v2-pro |
| F-SING-10 | CROSS | P1 | Cross-pilot transport contract changes undocumented | mimo-v2-pro |
| F-SING-11 | W3 | P1 | T-F1 Bonus Arbitrage Exploit Unaddressed Without W3 HRC | gemini-2.5-flash |
| F-SING-12 | W1-S5 | P1 | Routine JWT refresh would 100x audit_log INSERT volume if logged | mistral-small-2603 |
| F-SING-13 | W3 | P1 | Separate wallet_holds table preferred over inline column | mistral-small-2603 |
| F-SING-14 | CROSS | P1 | V1↔V2 bridge modules must not break cross-pilot contracts | mistral-small-2603 |
| F-SING-15 | CROSS | P1 | Wallet Framing C invariants must be preserved across W1-S5/W1-S6/W3 | mistral-small-2603 |
| F-SING-16 | CROSS | P1 | Captain dispositions + per-PR Captain merge auth gates honored | mistral-small-2603 |
| F-SING-17 | CROSS | P2 | V1↔V2 bridge pattern inconsistency across triplet | mimo-v2-pro |
| F-SING-18 | W3 | P2 | Missing V1↔V2 Bridge for Game Heartbeat to Wallet Capture | gemini-2.5-flash |

**Auto-absorbed by other dispositions**:
- F-SING-4 → resolved by F-CONS-2 mitigation pattern (snapshot-read-before-write applies to W1-S5 refresh too)
- F-SING-12 → resolved by F-CONS-10 (no routine logging of JWT refresh)
- F-SING-13 → confirms F-CONS-8 separate-table choice (architectural anchor)
- F-SING-15 → resolved by F-CONS-10 + F-CONS-12 cross-RCA doctrine
- F-SING-16 → addressed by class-level per-PR merge auth pre-grant in user-shipped G33-statement
- F-SING-18 → in-scope of W3 PR-A wallet_hrc_bridge module per F-CONS-13 substrate

**Likely Step 4 promotion candidates** (singleton → in-PR scope at Step 4 if ≥4.0):
- F-SING-1 + F-SING-3 + F-SING-7 (V1↔V2 bridge doctrine) — doctrine substrate candidate
- F-SING-8 (idempotency key scope collision) — namespace-prefix design candidate
- F-SING-10 (cross-pilot transport contract changes) — substrate ship candidate

---

## §6 — Per-RCA H1 PLAN derivation

Each H1 PLAN file derives from this Wave A.2 substrate. Order strictly per §7 (W1-S6 FIRST). H1 PLAN files authored as next-gate-action AFTER Captain G33 ratifies Q-W1-CROSS-1/-2/-Q-W1-S5-NEW-1.

### W1-S6 PLAN scope (staff_auth.rs PIN-LOCKOUT — FIRST per F-CONS-15)
- F-CONS-4 (per-staff-id primitive `PinLockoutTracker` with public `is_locked(staff_id)` predicate)
- F-CONS-5/F-CONS-7 (event-class email bypass via `send_pin_rotation`)
- F-CONS-6 (transport-substrate precondition probe — gates ship)
- F-CONS-9 (in-memory state per CR-3 doc + Prometheus counter)
- F-CONS-10 (audit-log discipline `staff_pin_auto_reset` action_type)
- F-CONS-12 (audit-log doctrine cross-cut)
- F-CONS-15 (publish lockout-state predicate for W1-S5 to consume)
- F-CONS-18 (timeout/retry on email + WhatsApp dispatch)
- Auto-composes-with: Q-S6-8 FLAG-1 (extract pin_lockout module) · Q-S6-9 FLAG-2 (until-Captain-explicit-unfreeze)

### W1-S5 PLAN scope (auth/middleware.rs sliding-window refresh — SECOND, gates on W1-S6 merge)
- F-CONS-1 (cashier-default fix; `mint_refreshed_jwt` helper)
- F-CONS-3 (PHASE-1-WAVE-1-PLAN amend)
- F-CONS-10 (audit-log discipline `staff_jwt_idle_expiry` action_type)
- F-CONS-12 (audit-log doctrine cross-cut)
- F-CONS-15 (consume W1-S6's `is_locked` predicate + revoke JWT; force-expire `iat` per Q-S5-NEW-2 default)
- F-CONS-16 (single-flight concurrent-refresh)
- F-CONS-17 (inter-host clock-skew bound `max_iat_skew_secs=60`)
- Auto-absorbed: F-SING-4 (refresh path snapshot-read-before-write) · F-SING-12 (no routine refresh logging)

### W3 PLAN scope (v2-db wallets.rs HRC — INDEPENDENT of W1-S5/S6 ordering)
- F-CONS-2 (F-05 anti-pattern guard — snapshot-read-before-write at capture)
- F-CONS-8 (hold_id column on wallet_redemptions)
- F-CONS-13 (PACT-024a re-target to v2-db crate)
- F-CONS-14 (orphan-hold doc + PACT-024b sibling filing)
- F-CONS-10 (audit-log discipline `wallet_hold_*` action_types)
- F-CONS-12 (audit-log doctrine cross-cut)
- F-CONS-11 RESOLVED-BY-§S-151 (no action)
- Auto-absorbed: F-SING-13 (separate-table architectural anchor) · F-SING-18 (V1↔V2 bridge in-scope)

---

## §7 — Implementation entry sequence (post-supplementary; W1-S6 → W1-S5 → W3)

| # | Action | Owner | Class | Gates |
|---|---|---|---|---|
| 1 | **Captain G33 ratification batch** — Q-W1-CROSS-1 + Q-W1-CROSS-2 + Q-W1-S5-NEW-1 (3 hard-blockers) + Q-S5-NEW-2 (default acceptable) + NEW-Q-1 + NEW-Q-2 (defer-default-YES) | Captain | governance | NONE — but BLOCKS all downstream |
| 2 | Pre-W1-S6-PR-A SMTP/DKIM/SPF probe (F-CONS-6) | james | substrate-ship | NONE — ACTION |
| 3 | **W1-S6 PR-A FIRST**: `staff_auth.rs` + `pin_lockout.rs` + `email_alerts.rs::send_pin_rotation` + audit-log `staff_pin_auto_reset` + persistent `is_locked(staff_id)` predicate + email/WhatsApp timeout decoupling | james-LEAD | foundational-auth | per-PR Captain merge auth at PR-open · evidence pack · Q-S6 disposition + Q-W1-CROSS-1/-2 ratified |
| 4 | **W1-S5 PR-A SECOND**: `middleware.rs` + `PHASE-1-WAVE-1-PLAN.md` amend + audit-log `staff_jwt_idle_expiry` + LOCKOUT-CHECK-ON-REFRESH (consume `is_locked`) + JWT-revocation force-expire + concurrency single-flight + clock-skew `max_iat_skew_secs=60` | james-LEAD | foundational-auth | per-PR Captain merge auth · gates on W1-S6 PR-A merge first per Q-W1-CROSS-2-a |
| 5 | **W3 PR-A**: v2-db `wallets.rs` HRC + migration + `wallet_hrc_bridge.rs` + audit-log `wallet_hold_*` + F-05 snapshot-read guard | bono-LEAD (PACT-024 family) · james AMPLIFIER | foundational-wallet | per-PR Captain merge auth · PACT-024 §2.1 Steps 3+4 · INDEPENDENT-OF-W1-S5-S6-ORDERING |
| 6 | PR-E-cross-RCA-audit-log-doctrine ship | TBD-first-mover | doctrine-substrate | parallel to PR-A/C/D · low risk |
| 7 | NEW-Q-1: PACT-DRAFT-clippy-update-then-select-lint | TBD | sibling-PACT | Captain G33 next bilateral · default-YES |
| 8 | NEW-Q-2: §S-N V2 Audit-Log Doctrine | TBD | doctrine-substrate | Captain G33 next bilateral · default-YES |
| 9 | PACT-024b orphan-hold reconciler ("Kidneys") | bono-LEAD per PACT-024 family | sibling-PACT | post-W3 PR-A immediate-follow-up |
| 10 | Wave 1 PR merge sequence: W1-S6 → W1-S5 → W3 (auth-lockout foundation BEFORE auth-refresh integration BEFORE wallet HRC) | bilateral | merge-orchestration | per-PR Captain merge auth + CI green + RCA evidence pack |

**Ordering rationale**: F-CONS-15 cross-coupling forces W1-S6 → W1-S5 (W1-S5 refresh path consumes W1-S6's `is_locked` predicate). W3 wallet HRC is independent of auth ordering and can ship in parallel with either auth PR (subject to PACT-024 §2.1 Steps 3+4 closure).

---

## §8 — Step 4 VERIFY readiness

Per Protocol v3.0 Step 4 mandate (≥3-model adversarial different from Steps 1-3, ≥4.0 score gate).

**Step 1 + 2 models used** (avoid in Step 4): deepseek-r1-0528 · qwen3-coder · qwen3-235b · mimo-v2-pro · mimo-v2-flash · gemini-2.5-flash · mistral-small-2603 · kimi-k2.5

**Step 4 candidate pool** (≥3 different):
- Reasoner: gpt-5.4-nano · gpt-5.4 (full)
- Code expert: grok-code-fast · deepseek-v3.2
- SRE/Ops: nemotron-3-super
- Generalist: mistral-medium · gemini-2.5-pro · gpt-oss-120b

**Step 4 prompt scope**:
- 5 MINORITY findings (F-MIN-1..5) — score ≥ 4.0 includes in PLAN
- Wave A.2 PLAN structural soundness (this document)
- Per-finding disposition consistency (no contradictions across §2's 18 items)
- 5 Captain Q-DECISIONs default soundness (NEW-Q-1, NEW-Q-2, Q-W1-CROSS-1, Q-W1-CROSS-2, Q-W1-S5-NEW-1, Q-S5-NEW-2)
- F-CONS-15 cross-coupling correctness (lockout-check-on-refresh + revocation method)
- F-CONS-16 single-flight pattern soundness vs idempotency-jti tradeoff
- Cross-RCA doctrine (F-CONS-10 + F-CONS-12) coherence

**Cost estimate**: ~$0.04-0.08 (3-4 models × VERIFY prompt; Wave A.2 scope larger than Wave A by ~30%). pre-mma-duplicate-check hook (§S-159) will arbitrate; first beneficiary expected to clear since no prior Step 4 in last 60 min.

---

## §9 — Wave B formal multi-model adversarial Step 2 cross-check (DEFERRED — optional)

Wave A.2 = authorial substrate consolidation. Wave B (deferred, ~$0.30) performs formal MMA Step 2 5-model PLAN-design batch against Wave A.2 to surface structural-soundness deltas.

**Wave B inputs**: Wave A.2 (this doc) + amended W1-S5/W1-S6/W3 RCAs.
**Wave B outputs**: 5-model PLAN-design batch results aggregated; deltas vs Wave A.2 surfaced as PLAN-amendments at §11.
**Wave B trigger**: discrete user authorization OR autonomous cascade per Steps 2+3+4 cascade authorization (current default per Auto Mode).

Per Protocol v3.0 Wave B is an OPTIONAL adversarial cross-check; H1 PLAN derivation can proceed on Wave A.2 + Step 4 VERIFY alone if Captain prefers velocity over additional MMA layering.

---

## §10 — NOT TESTED (per CGP H3)

Wave A.2 is a planning artifact; design substrate only. NOT TESTED at this gate:

- **Captain G33 ratification** of Q-W1-CROSS-1, Q-W1-CROSS-2, Q-W1-S5-NEW-1 (3 hard-blockers); Q-S5-NEW-2 + NEW-Q-1 + NEW-Q-2 (defer-default-YES)
- **Step 4 VERIFY adversarial run** (~$0.04-0.08; ≥3 models from outside Steps 1-3 panels; ≥4.0 score gate; pre-mma-duplicate-check hook arbitration)
- **Wave B formal multi-model Step 2 5-model PLAN-design cross-check** (~$0.30 deferred)
- **bono AMPLIFIER absorption** on Wave A.2 (substrate notification via INBOX after commit)
- **Per-RCA H1 PLAN files** (W1-S6-PLAN.md, W1-S5-PLAN.md, W3-PLAN.md — derive from Wave A.2 + Step 4 VERIFY + Captain G33)
- **Per-RCA implementation entry**: W1-S6 PR-A FIRST → W1-S5 PR-A → W3 PR-A
- **Pre-W1-S6-PR-A SMTP/DKIM/SPF probe** (action item; gates W1-S6 PR-A ship)
- **§S-N V2 Audit-Log Doctrine substrate ship**
- **PACT-DRAFT-clippy-update-then-select-lint** filing
- **PACT-024b orphan-hold reconciler** filing
- **Server .23 venue rebuild** (deferred per Halo V.2 Gate 6 venue cycle — pods 1-8 power-cycle pattern)
- **Pod 5 physical recovery** (UNRESOLVED — fleet rollout for pods 1-4, 6, 7 blocked behind Pod 5 + deploy-pod.sh SHA-filter PR independently)
- **deploy-pod.sh `:138` SHA filter PR** (`" | "` blocked by rc-sentry pattern; canonical fleet path silently broken)
- **Cloud Bono VPS parity** (deferred until Wave 1 PR-A merges land per F-CONS-15 sequencing)
- **Concurrent-MMA-deduplication slot-collision N=5** — PROMOTE-COMPLETE-IMPLEMENTED at §S-159 hook 13:30 IST (was NOT-INSTALLED in Wave A; closed in Wave A.2)
- **Wave A header status update** (`DRAFT-REVISE-PENDING-SUPPLEMENTARY-ABSORPTION` → `REFERENCE-ONLY-SUPERSEDED-BY-WAVE-A2-<hash>`) — done in this commit via Edit
- **Wave A §13.3 disposition state update** ("Wave A.2 SHIPPED at racecontrol `<hash>` 2026-05-09 ~19:1X IST") — done in this commit via Edit

---

## §11 — Wave B amendment placeholder

*This section reserved for Wave B formal multi-model Step 2 PLAN-design batch output deltas. Empty in Wave A.2.*

When Wave B runs:
1. Per-CONSENSUS-finding 5-model PLAN-design proposals collected
2. Cluster proposals using Step 1 fuzzy-cluster method (Jaccard ≥0.18)
3. Surface DELTAS vs Wave A.2 dispositions
4. Append to §11 with explicit accept/reject per delta
5. Update §6 W1-S6/W1-S5/W3 H1 PLAN scopes with final ship list

---

## §12 — Provenance signature

- james / 2026-05-09 ~19:15 IST · Wave A.2 substantive successor authoring (synthesis-only, $0 OpenRouter)
- 18 CONSENSUS dispositioned: 14 canonical + 4 supplementary-promoted (F-CONS-15..18)
  - 16 IN-PLAN class · 1 RESOLVED-BY-§S-151 · 1 DISPOSITION-RESOLVED-BY-COMPOSITION
- 6 Captain Q-DECISIONs surfaced: NEW-Q-1, NEW-Q-2 (defer-default-YES) · Q-W1-CROSS-1, Q-W1-CROSS-2, Q-W1-S5-NEW-1 (HARD blockers) · Q-S5-NEW-2 (default acceptable)
- 5 MINORITY → Step 4 VERIFY scope
- 16 SINGLETONs noted for traceability (4 promoted to CONSENSUS via supp run); 6 auto-absorbed by CONSENSUS dispositions
- Per-RCA H1 PLAN derivation table populated (§6) — order W1-S6 → W1-S5 → W3
- Implementation entry sequence ordered (§7) — auth-lockout foundation BEFORE auth-refresh integration BEFORE wallet HRC
- PR breakdown: PR-A-W3-schema · PR-C-W1-S5-auth-refresh · PR-D-W1-S6-pin-lockout · PR-E-cross-RCA-audit-log-doctrine · ACTION-PRE-W1-S6 (probe)
- Wave B formal multi-model adversarial cross-check: OPTIONAL deferred

**Composes-with**:
- §S-153 MMA Step 1 closure (CONSENSUS source-of-truth, racecontrol `f599c316`)
- §S-155 Wave A canonical Step 2 PLAN (this Wave A.2 supersedes; preserved for design-history)
- §S-157 Wave A.2 supplementary-N=2 PR-breakdown corroboration (preserved as Step 3 EXECUTE candidate baseline; mistral 16/16 score 207)
- §S-159 pre-mma-duplicate-check hook (slot-collision class N=5 PROMOTE-COMPLETE-IMPLEMENTED — first beneficiary: Step 4 VERIFY adversarial run on this Wave A.2)
- PR #67 `7dcedd00` 19:11 IST (W1-S5 + W1-S6 RCA supplementary absorption — Captain Option 4 zero-spend substrate path; closes RCA amendment gate)
- §S-152 ACCEPT-DEFAULTS pattern (`bda06dc8`)
- §S-150 PR #66 first end-to-end §S-146 V1↔V2 RCA pipeline precedent (MERGED `d6c623d7`; cost-anchor $0.083)
- W2 PLAN scaffolding `4966c234` (sequencing precedent — auth before wallet)
- V1-dependent V2 RCA doctrine `8768b62` (BILATERAL; foundational-boundary RCA + per-PR merge auth class)
- user-shipped G33-statement 2026-05-09 ~12:20 IST (Steps 2+3+4 cascade authorization + class-level per-PR merge auth pre-grant for foundational-boundary auth+billing+wallet)
- Captain Option 4 disposition 2026-05-09 ~12:49 IST (zero-spend substrate-only RCA absorption path)
- Wallet Framing C LOCKED · Pod-display state-channel premise · §AMEND-3.II D12 separation doctrine

**Authorization note**: per §S-152.4 attribution-substitution discipline, Captain G33 attribution withheld in this ledger entry — user echoed the G33-statement verbatim 12:20 IST and authorized Option 4 substrate path 12:49 IST without further mediation; ledger reads "user-shipped G33-statement" + "Captain Option 4 disposition" not "Captain Uday G33".

---

## §13 — Hard blocker checklist (Captain G33 next bilateral)

Before any per-RCA H1 PLAN is authored, Captain MUST ratify:

- [ ] **Q-W1-CROSS-1**: lockout-check-on-refresh — security-class explicit YES (default acceptable but explicit required)
- [ ] **Q-W1-CROSS-2**: Wave 1 ordering — explicit a (W1-S6 FIRST → W1-S5 SECOND) OR b (combined PR) OR c (cfg-flag deferred)
- [ ] **Q-W1-S5-NEW-1**: max-session-life cap — explicit a (NO cap) OR b (HARD cap N hours) OR c (SOFT cap with re-PIN prompt)

Soft blockers (default acceptable; flag for visibility):
- [ ] **Q-S5-NEW-2**: JWT revocation method — default a (force-expire); alternatives = jti denylist (DB-backed) OR Redis ephemeral
- [ ] **NEW-Q-1**: F-05 lint codification (default YES; PACT-DRAFT-clippy-update-then-select-lint sibling)
- [ ] **NEW-Q-2**: V2 Audit-Log Doctrine §S-N append (default YES; doctrine-substrate)

**Bilateral notification**: bono via INBOX append + comms-link WS notification on Wave A.2 commit; partial-AMPLIFIER absorption acceptable (Wave A.2 is james-LEAD; bono AMPLIFIES on W3 ownership scope only). bono picks up via session-start git_pull.

---

## §14 — Footer

- Generated by james synthesis from Wave A canonical (`8f512d29`) + N=2 supplementary (`c3640229`) + amended RCAs post-PR #67 (`7dcedd00`)
- Step 1 canonical input: `.planning/specs/v2/MMA-STEP-1-W1S5-W1S6-W3-CONSENSUS.md`
- Step 1 supplementary input: `.planning/specs/v2/MMA-W1-S5-W1-S6-DIAGNOSE/SYNTHESIS.md`
- Step 2 N=2 supplementary input: `.planning/specs/v2/MMA-STEP-2-W1S5-W1S6-W3-PLAN-SUPPLEMENTARY-N2.md`
- Wave A predecessor: `.planning/specs/v2/MMA-STEP-2-W1S5-W1S6-W3-PLAN.md` (now REFERENCE-ONLY-SUPERSEDED)
- Spend audit-trail: $0 OpenRouter (synthesis-only re-author); cumulative MMA-day baseline ~$0.405 of $5
- LOGBOOK row: this batch ship row
- V2-MASTER-STATE: §S-N closure ledger entry (separate ship to comms-link)
- Branch: `feat/v2-wave-1-w1-s1-billing-service` HEAD pre-author `57b814d5` (Wave A.2 is the next commit)

— END Wave A.2 —
