# MMA Step 2 PLAN — W1-S5 + W1-S6 + W3 RCA Triplet
> **CLASS: SUPPLEMENTARY-DUPLICATE-RUN-N=2 (slot-collision class N=5)**
>
> This artifact is NOT the canonical Step 2 PLAN. Canonical Wave A shipped at
> `MMA-STEP-2-W1S5-W1S6-W3-PLAN.md` per V2-MASTER-STATE §S-155 (~12:30 IST 2026-05-09)
> by a parallel autonomous james-session.
>
> **Self-G9** (this session): Verify-Before-Generate violated. Did not grep V2-MASTER-STATE.md
> for §S-155 or check `racecontrol/.planning/specs/v2/MMA-STEP-2-W1S5-W1S6-W3-PLAN.md`
> existence before authoring prompt + runner + spending API budget. Same exact failure mode
> that `46d612ca` self-G9'd 5 minutes earlier (slot-collision class N=4 PROMOTE trigger).
> Slot-collision class **PROMOTE-N=5** trips per §S-153.6 CANDIDATE-N1 + §S-153 deferred
> structural-fix (pre-MMA grep `data/openrouter-spend-*.jsonl` + V2-MASTER-STATE §S-N for
> same-RCA same-step in last 60min).
>
> **Net-signal value**: see §1.5 below — per-finding comparison vs canonical Wave A.
> If canonical covered finding fully, my model entries are corroboration; if canonical
> deferred or partial, my entries may add design substrate for Step 4 VERIFY adversarial
> cross-check.

---


**Scope**: W1-S5 + W1-S6 + W3 RCA triplet (foundational-boundary class)

**Run**: 2026-05-09 (per Unified MMA Protocol v3.0 Step 2 PLAN)
**Models**: deepseek/deepseek-r1-0528 / qwen/qwen3-coder / xiaomi/mimo-v2-pro / google/gemini-2.5-flash / mistralai/mistral-small-2603
**Wall-clock**: ~593.4s parallel
**Total spend**: $0.0457

**Inputs consumed (16 mandatory):**
- 14 CONSENSUS findings from Step 1 canonical batch (`.planning/specs/v2/MMA-STEP-1-W1S5-W1S6-W3-CONSENSUS.md`, racecontrol `f599c316`)
- 2 supplementary-promotable findings from parallel-session run (CF-S1 cross-RCA sliding-window-bypasses-PIN-LOCKOUT + CF-S2 inter-host clock-skew; racecontrol `46d612ca`)

**Cross-reference**: Step 1 MINORITY (5) findings deferred to Step 4 VERIFY adversarial evaluation per Protocol v3.0. Step 1 SINGLETON (20) findings noted for traceability.

---

## §0 — Per-model coverage + best-plan selection

| Model | Coverage (of 16) | Total Actions | Total Tests | Score |
|---|---|---|---|---|
| mistral | 16 | 32 | 15 | 207 |
| qwen | 16 | 13 | 12 | 185 |
| gemini | 12 | 27 | 10 | 157 |
| r1 | 0 | 0 | 0 | 0 |
| mimo | 0 | 0 | 0 | 0 |

**Highest-scoring single-model plan**: `mistralai/mistral-small-2603` (score 207). Per Protocol v3.0 Step 3 EXECUTE selects this as the candidate baseline; consensus-merged actions from all 5 models layer in via §1 Per-finding consensus below.

## §0.1 — PR breakdown (consensus PR-assignment rollup)

Aggregated from per-finding `pr_assignment` consensus across all 5 models.

- **deferred-PACT-024b** — CF-11, CF-14
- **deferred-V2.1** — CF-9
- **PR-A-W3-schema** — CF-2, CF-8, CF-13
- **PR-C-W1-S5-auth-refresh** — CF-1, CF-3, CF-S2
- **PR-D-W1-S6-pin-lockout** — CF-4, CF-5, CF-6, CF-7, CF-S1
- **PR-E-cross-RCA-audit-log-doctrine** — CF-10, CF-12

---

## §1 — Per-finding consensus + per-model dispositions

Each of the 16 mandatory inputs MUST be dispositioned per Protocol v3.0. Consensus disposition + PR assignment emerges from cross-model agreement; per-model entries preserved for Step 3 EXECUTE selection + Step 4 VERIFY adversarial review.

### CF-1: [P0][W1-S5] create_staff_jwt cashier-default role-downgrade on sliding-window refresh

- **Models covered**: 3/5
- **Consensus disposition**: in-PLAN-scope (3/3)
- **Consensus PR assignment**: PR-C-W1-S5-auth-refresh (2/3)
- **Captain Q-DECISION required (per-model count)**: 0/5

**Per-model plans:**

#### qwen
- _Disposition_: in-PLAN-scope
- _PR_: PR-C-W1-S5-auth-refresh
- _Blast radius_: single-module; _v1-dependent_: true
- _Deploy targets_: server-23, bono-vps
- _Actions_:
  - **Extract mint_refreshed_jwt helper using claims.role explicitly** @ `crates/racecontrol/src/auth/middleware.rs:144-161` (✓ smallest-reversible)
- _Tests_: manager_superadmin_role_preserved_across_refresh [unit]
- _Rollback_: Revert to direct create_staff_jwt usage in refresh path.

#### gemini
- _Disposition_: in-PLAN-scope
- _PR_: PR-C-W1-S5-auth-refresh
- _Blast radius_: single-module; _v1-dependent_: true
- _Deploy targets_: server-23, bono-vps
- _Actions_:
  - **Extract `mint_refreshed_jwt(claims)` helper using `claims.role` via `create_staff_jwt_with_role(claims.role)`.** @ `crates/racecontrol/src/auth/middleware.rs:144-161,251-273` (✓ smallest-reversible)
  - **Add optional lint forbidding direct `create_staff_jwt` use in new auth-refresh code.** @ `crates/racecontrol/src/auth/mod.rs (or similar lint config)` (✓ smallest-reversible)
- _Tests_: manager_superadmin_role_preserved_across_refresh [integration]
- _Rollback_: Revert the helper extraction and lint changes. The original `create_staff_jwt` path remains functional.

#### mistral
- _Disposition_: in-PLAN-scope
- _PR_: PR-B-W1-S5-auth-refresh
- _Blast radius_: single-module; _v1-dependent_: true
- _Deploy targets_: server-23
- _Actions_:
  - **Extract mint_refreshed_jwt helper preserving claims.role** @ `crates/racecontrol/src/auth/middleware.rs:144-161,251-273` (✓ smallest-reversible)
  - **Add test manager_superadmin_role_preserved_across_refresh** @ `tests/auth_refresh.rs:45-60` (✓ smallest-reversible)
- _Tests_: manager_superadmin_role_preserved_across_refresh [integration]
- _Rollback_: Revert PR-B-W1-S5-auth-refresh; middleware.rs reverts to original create_staff_jwt calls.

---

### CF-2: [P0][W3] F-05 UPDATE-then-SELECT anti-pattern reintroduction risk in W3 capture path

- **Models covered**: 3/5
- **Consensus disposition**: in-PLAN-scope (3/3)
- **Consensus PR assignment**: PR-A-W3-schema (2/3)
- **Captain Q-DECISION required (per-model count)**: 0/5

**Per-model plans:**

#### qwen
- _Disposition_: in-PLAN-scope
- _PR_: PR-A-W3-schema
- _Blast radius_: single-fn; _v1-dependent_: true
- _Deploy targets_: server-23
- _Actions_:
  - **Add invariant comment to WalletService::capture to prevent F-05 anti-pattern** @ `crates/v2-db/src/wallets.rs::WalletService::capture` (✓ smallest-reversible)
- _Tests_: f05_anti_pattern_regression_check [regression]
- _Rollback_: Revert to prior capture logic or remove invariant comment.

#### gemini
- _Disposition_: in-PLAN-scope
- _PR_: PR-B-W3-service
- _Blast radius_: single-fn; _v1-dependent_: true
- _Deploy targets_: server-23, bono-vps
- _Actions_:
  - **Add invariant comment to `WalletService::capture`.** @ `crates/v2-db/src/wallets.rs::WalletService::capture` (✓ smallest-reversible)
  - **Implement logic to read wallet snapshot BEFORE any UPDATE; compute delta from snapshot; never read `balance_credits` after UPDATE.** @ `crates/v2-db/src/wallets.rs::WalletService::capture` (⚠ not smallest-reversible)
  - **Add code comment referencing `ROOT-CAUSE-ANALYSIS-F05-2026-03-28.md` + ₹162.50 incident.** @ `crates/v2-db/src/wallets.rs::WalletService::capture` (✓ smallest-reversible)
- _Tests_: f05_anti_pattern_regression_check [regression]
- _Rollback_: Revert the logic changes in `WalletService::capture`. The previous (v1-inherited) behavior would be restored.

#### mistral
- _Disposition_: in-PLAN-scope
- _PR_: PR-A-W3-schema
- _Blast radius_: single-fn; _v1-dependent_: false
- _Deploy targets_: server-23
- _Actions_:
  - **Add invariant comment + regression test for F-05 anti-pattern** @ `crates/v2-db/src/wallets.rs::WalletService::capture comment + tests/wallet_capture.rs:110-130` (✓ smallest-reversible)
  - **Ensure credits_held snapshot read before UPDATE** @ `crates/v2-db/src/wallets.rs::WalletService::capture:240-260` (✓ smallest-reversible)
- _Tests_: f05_anti_pattern_regression_check [regression]
- _Rollback_: Revert PR-A-W3-schema wallet capture changes.

---

### CF-3: [P2][W1-S5] PHASE-1-WAVE-1-PLAN 7-min vs 30-min idle-timeout inconsistency

- **Models covered**: 3/5
- **Consensus disposition**: in-PLAN-scope (3/3)
- **Consensus PR assignment**: PR-C-W1-S5-auth-refresh (2/3)
- **Captain Q-DECISION required (per-model count)**: 0/5

**Per-model plans:**

#### qwen
- _Disposition_: in-PLAN-scope
- _PR_: PR-C-W1-S5-auth-refresh
- _Blast radius_: documentation-only; _v1-dependent_: false
- _Actions_:
  - **Amend PHASE-1-WAVE-1-PLAN.md to correct idle timeout from 7-min to 30-min** @ `PHASE-1-WAVE-1-PLAN.md rows 21+33` (✓ smallest-reversible)
- _Tests_: idle_timeout_secs_assert_30_min [unit]
- _Rollback_: Revert markdown change to restore 7-min value.

#### gemini
- _Disposition_: in-PLAN-scope
- _PR_: PR-C-W1-S5-auth-refresh
- _Blast radius_: single-doc; _v1-dependent_: true
- _Actions_:
  - **Amend `PHASE-1-WAVE-1-PLAN.md` rows 21+33 from '7-min' to '30-min sliding-window'.** @ `PHASE-1-WAVE-1-PLAN.md` (✓ smallest-reversible)
  - **Add comment referencing Q-S5-7 ACCEPT-DEFAULT.** @ `PHASE-1-WAVE-1-PLAN.md` (✓ smallest-reversible)
- _Tests_: idle_timeout_config_matches_plan [unit]
- _Rollback_: Revert the markdown file change.

#### mistral
- _Disposition_: in-PLAN-scope
- _PR_: PR-B-W1-S5-auth-refresh
- _Blast radius_: single-doc; _v1-dependent_: true
- _Actions_:
  - **Amend PHASE-1-WAVE-1-PLAN rows 21+33 from '7-min' to '30-min sliding-window'** @ `PHASE-1-WAVE-1-PLAN.md:21,33` (✓ smallest-reversible)
  - **Add regression test asserting idle_timeout_secs=1800** @ `tests/auth_sliding_window.rs:75-85` (✓ smallest-reversible)
- _Tests_: sliding_window_idle_timeout_30min [unit]
- _Rollback_: Revert PHASE-1-WAVE-1-PLAN.md edits.

---

### CF-4: [P0][W1-S6] V1 IP-keyed rate-limit unusable for per-staff-id ≤3 resets/hr

- **Models covered**: 3/5
- **Consensus disposition**: in-PLAN-scope (3/3)
- **Consensus PR assignment**: PR-D-W1-S6-pin-lockout (2/3)
- **Captain Q-DECISION required (per-model count)**: 0/5

**Per-model plans:**

#### qwen
- _Disposition_: in-PLAN-scope
- _PR_: PR-D-W1-S6-pin-lockout
- _Blast radius_: single-module; _v1-dependent_: true
- _Deploy targets_: server-23
- _Actions_:
  - **Introduce StaffRateLimiter in staff_auth.rs with in-memory HashMap and 1hr sliding window** @ `crates/racecontrol/src/auth/staff_auth.rs` (✓ smallest-reversible)
- _Tests_: per_staff_id_rate_limit_isolation [unit]
- _Rollback_: Remove StaffRateLimiter and revert to IP-keyed rate limiter.

#### gemini
- _Disposition_: in-PLAN-scope
- _PR_: PR-D-W1-S6-pin-lockout
- _Blast radius_: single-module; _v1-dependent_: false
- _Deploy targets_: server-23, pos-130
- _Actions_:
  - **Introduce NEW per-staff-id primitive `PinLockoutTracker` (in-memory HashMap; sliding 1hr window; ≤3 resets cap).** @ `crates/racecontrol/src/auth/staff_auth.rs` (⚠ not smallest-reversible)
  - **Ensure NO reuse of V1 IP-keyed module (`auth/rate_limit.rs`).** @ `crates/racecontrol/src/auth/staff_auth.rs` (✓ smallest-reversible)
- _Tests_: per_staff_id_lockout_isolation [integration]
- _Rollback_: Remove the `PinLockoutTracker` module and revert calls to it. The system would revert to no per-staff-id lockout.

#### mistral
- _Disposition_: in-PLAN-scope
- _PR_: PR-C-W1-S6-pin-lockout
- _Blast radius_: single-module; _v1-dependent_: true
- _Deploy targets_: server-23
- _Actions_:
  - **Introduce StaffRateLimiter per-staff-id primitive** @ `crates/racecontrol/src/auth/staff_auth.rs:40-80` (✓ smallest-reversible)
  - **Add test per-staff-id isolation across same-IP** @ `tests/auth_rate_limit.rs:90-120` (✓ smallest-reversible)
- _Tests_: per_staff_id_rate_limit_isolation [integration]
- _Rollback_: Revert PR-C-W1-S6-pin-lockout staff_auth.rs changes.

---

### CF-5: [P1][W1-S6] EmailAlerter last_sent_per_pod HashMap unbounded growth

- **Models covered**: 3/5
- **Consensus disposition**: in-PLAN-scope (3/3)
- **Consensus PR assignment**: PR-D-W1-S6-pin-lockout (2/3)
- **Captain Q-DECISION required (per-model count)**: 0/5

**Per-model plans:**

#### qwen
- _Disposition_: in-PLAN-scope
- _PR_: PR-D-W1-S6-pin-lockout
- _Blast radius_: single-module; _v1-dependent_: true
- _Deploy targets_: server-23
- _Actions_:
  - **Add TTL purge or LRU eviction to EmailAlerter::last_sent_per_staff HashMap** @ `crates/racecontrol/src/email_alerts.rs:9-30` (✓ smallest-reversible)
- _Tests_: email_alerts_map_size_gauge_monitored [integration]
- _Rollback_: Remove TTL/eviction logic and revert to unbounded HashMap.

#### gemini
- _Disposition_: in-PLAN-scope
- _PR_: PR-D-W1-S6-pin-lockout
- _Blast radius_: single-module; _v1-dependent_: true
- _Deploy targets_: server-23, bono-vps
- _Actions_:
  - **Introduce sibling HashMap `last_sent_per_staff` with TTL purge (24h expiry) OR LRU eviction.** @ `crates/racecontrol/src/email_alerts.rs` (⚠ not smallest-reversible)
  - **Implement periodic cleanup task (5-min) for `last_sent_per_staff`.** @ `crates/racecontrol/src/email_alerts.rs (or background worker)` (⚠ not smallest-reversible)
  - **Add Prometheus gauge `email_alerts.last_sent_map_size`.** @ `crates/racecontrol/src/email_alerts.rs` (✓ smallest-reversible)
- _Tests_: email_alerter_map_growth_bounded [unit]
- _Rollback_: Revert the `last_sent_per_staff` HashMap and cleanup logic. The system would revert to the unbounded V1 behavior.
- _Depends-on_: CF-7

#### mistral
- _Disposition_: in-PLAN-scope
- _PR_: PR-C-W1-S6-pin-lockout
- _Blast radius_: single-module; _v1-dependent_: true
- _Deploy targets_: server-23
- _Actions_:
  - **Replace unbounded HashMap with TTL-purged LRU sibling** @ `crates/racecontrol/src/email_alerts.rs:9-30` (✓ smallest-reversible)
  - **Add Prometheus gauge email_alerts.last_sent_map_size** @ `comms-link/metrics.rs:110` (✓ smallest-reversible)
- _Tests_: email_alerts_map_size_stabilizes [integration]
- _Rollback_: Revert PR-C-W1-S6-pin-lockout email_alerts.rs changes.
- _Depends-on_: CF-7

---

### CF-6: [P1][W1-S6] SMTP transport + DKIM/SPF unverified at Server .23 / Bono VPS

- **Models covered**: 3/5
- **Consensus disposition**: in-PLAN-scope (3/3)
- **Consensus PR assignment**: PR-D-W1-S6-pin-lockout (1/3)
- **Captain Q-DECISION required (per-model count)**: 3/5

**Per-model plans:**

#### qwen
- _Disposition_: in-PLAN-scope
- _PR_: PR-D-W1-S6-pin-lockout
- _Blast radius_: cross-system; _v1-dependent_: true
- _Deploy targets_: server-23, bono-vps
- _Actions_:
  - **Verify SMTP/DKIM/SPF at Server .23 and Bono VPS; add Prometheus counter for delivery success** @ `comms-link/shared/send-email.js + crates/racecontrol/src/email_alerts.rs` (✓ smallest-reversible)
- _Tests_: smtp_dkim_spf_verified_at_deploy [integration]
- _Rollback_: Remove Prometheus counter and revert to unverified SMTP.

#### gemini
- _Disposition_: in-PLAN-scope
- _PR_: PR-F-W1-S6-email-infra-probe
- _Blast radius_: cross-system; _v1-dependent_: true
- _Deploy targets_: server-23, bono-vps
- _Actions_:
  - **Manual probe: Verify sendmail at Server .23, SMTP localhost:25 at Bono VPS.** @ `Server .23, Bono VPS` (✓ smallest-reversible)
  - **Manual probe: `dig +short TXT racingpoint.in` for SPF, DKIM lookup.** @ `DNS records for racingpoint.in` (✓ smallest-reversible)
  - **Add Prometheus counter for email delivery success/failure.** @ `crates/racecontrol/src/email_alerts.rs` (✓ smallest-reversible)
  - **Surface Captain Q-DECISION (ship-with-risk + monitor first delivery vs delay W1-S6 until DKIM/SPF up).** @ `N/A (governance)` (✓ smallest-reversible)
- _Rollback_: No code changes to rollback. If verification fails and Captain decides to delay, W1-S6 deployment is halted.

#### mistral
- _Disposition_: in-PLAN-scope
- _PR_: PR-C-W1-S6-pin-lockout
- _Blast radius_: cross-system; _v1-dependent_: true
- _Deploy targets_: server-23, bono-vps
- _Actions_:
  - **Session 5 probe: verify sendmail/SMTP/DKIM/SPF at Server .23 and Bono VPS** @ `docs/ops/email_verification.md + ansible/roles/email.yml` (✓ smallest-reversible)
  - **Add Prometheus counter email_delivery_success** @ `comms-link/metrics.rs:120` (✓ smallest-reversible)
- _Tests_: email_delivery_success_monitored [integration]
- _Rollback_: Revert ansible/roles/email.yml changes; revert metrics counter.

---

### CF-7: [P1][W1-S6] V1 cooldown semantics conflict with W1-S6 event-class always-deliver

- **Models covered**: 3/5
- **Consensus disposition**: in-PLAN-scope (3/3)
- **Consensus PR assignment**: PR-D-W1-S6-pin-lockout (2/3)
- **Captain Q-DECISION required (per-model count)**: 0/5

**Per-model plans:**

#### qwen
- _Disposition_: in-PLAN-scope
- _PR_: PR-D-W1-S6-pin-lockout
- _Blast radius_: single-module; _v1-dependent_: true
- _Deploy targets_: server-23
- _Actions_:
  - **Add EmailAlerter::send_pin_rotation method that bypasses cooldown HashMap** @ `crates/racecontrol/src/email_alerts.rs:69-83` (✓ smallest-reversible)
- _Tests_: pin_rotate_bypasses_cooldown [unit]
- _Rollback_: Remove send_pin_rotation method and revert to shared cooldown path.

#### gemini
- _Disposition_: in-PLAN-scope
- _PR_: PR-D-W1-S6-pin-lockout
- _Blast radius_: single-fn; _v1-dependent_: true
- _Deploy targets_: server-23, bono-vps
- _Actions_:
  - **Introduce new sibling method `EmailAlerter::send_pin_rotation` that bypasses cooldown HashMap entirely.** @ `crates/racecontrol/src/email_alerts.rs:9-30,69-83` (⚠ not smallest-reversible)
  - **Add invariant comment: 'PIN-rotate is event-class; always deliver.'** @ `crates/racecontrol/src/email_alerts.rs::send_pin_rotation` (✓ smallest-reversible)
- _Tests_: pin_rotate_email_always_sent_bypassing_cooldown [unit]
- _Rollback_: Revert the `send_pin_rotation` method. PIN rotation emails would then be subject to cooldowns, potentially blocking delivery.
- _Depends-on_: CF-5

#### mistral
- _Disposition_: in-PLAN-scope
- _PR_: PR-C-W1-S6-pin-lockout
- _Blast radius_: single-fn; _v1-dependent_: true
- _Deploy targets_: server-23
- _Actions_:
  - **Add EmailAlerter::send_pin_rotation bypassing cooldown HashMap** @ `crates/racecontrol/src/email_alerts.rs:69-83` (✓ smallest-reversible)
  - **Add invariant comment 'PIN-rotate is event-class; always deliver'** @ `crates/racecontrol/src/email_alerts.rs:70` (✓ smallest-reversible)
- _Tests_: pin_rotation_bypasses_cooldown [unit]
- _Rollback_: Revert PR-C-W1-S6-pin-lockout email_alerts.rs send_pin_rotation method.

---

### CF-8: [P1][W3] wallet_redemptions must include hold_id for bonus source-tag through HRC

- **Models covered**: 3/5
- **Consensus disposition**: in-PLAN-scope (3/3)
- **Consensus PR assignment**: PR-A-W3-schema (3/3)
- **Captain Q-DECISION required (per-model count)**: 0/5

**Per-model plans:**

#### qwen
- _Disposition_: in-PLAN-scope
- _PR_: PR-A-W3-schema
- _Blast radius_: schema; _v1-dependent_: false
- _Deploy targets_: server-23
- _Actions_:
  - **Add hold_id column to wallet_redemptions table with FK reference to wallet_holds** @ `migrations/w3_wallet_redemptions_hold_id.sql` (✓ smallest-reversible)
- _Tests_: bonus_source_tag_preserved_through_hrc [integration]
- _Rollback_: DROP COLUMN hold_id from wallet_redemptions.

#### gemini
- _Disposition_: in-PLAN-scope
- _PR_: PR-A-W3-schema
- _Blast radius_: cross-crate; _v1-dependent_: false
- _Deploy targets_: server-23, bono-vps
- _Actions_:
  - **Add `hold_id TEXT REFERENCES wallet_holds(id)` to `wallet_redemptions` table in W3 migration.** @ `crates/v2-db/migrations/W3_add_hold_id_to_redemptions.sql` (✓ smallest-reversible)
  - **Modify `WalletService::capture` to populate the new `hold_id` column.** @ `crates/v2-db/src/wallets.rs::WalletService::capture` (⚠ not smallest-reversible)
- _Tests_: bonus_source_tag_preserved_through_hrc [integration]
- _Rollback_: Revert the migration to drop the `hold_id` column and revert changes in `WalletService::capture`. This would break forensic traceability.

#### mistral
- _Disposition_: in-PLAN-scope
- _PR_: PR-A-W3-schema
- _Blast radius_: single-crate; _v1-dependent_: false
- _Deploy targets_: server-23
- _Actions_:
  - **Add hold_id TEXT REFERENCES wallet_holds(id) to wallet_redemptions** @ `migrations/20260515_w3_wallet_schema.sql:45-50` (✓ smallest-reversible)
  - **Populate hold_id in WalletService::capture** @ `crates/v2-db/src/wallets.rs::WalletService::capture:250-260` (✓ smallest-reversible)
- _Tests_: bonus_source_tag_preserved_through_hrc [integration]
- _Rollback_: Run rollback SQL; revert PR-A-W3-schema wallet capture changes.

---

### CF-9: [P2][W1-S6] Lockout state durability: in-memory acceptable per CR-3

- **Models covered**: 3/5
- **Consensus disposition**: explicit-defer-with-rationale (2/3)
- **Consensus PR assignment**: deferred-V2.1 (1/3)
- **Captain Q-DECISION required (per-model count)**: 1/5

**Per-model plans:**

#### qwen
- _Disposition_: explicit-defer-with-rationale
- _PR_: deferred-V2.1
- _Blast radius_: single-module; _v1-dependent_: false
- _Deploy targets_: server-23
- _Rollback_: N/A

#### gemini
- _Disposition_: explicit-defer-with-rationale
- _PR_: PR-D-W1-S6-pin-lockout
- _Blast radius_: single-module; _v1-dependent_: false
- _Deploy targets_: server-23, pos-130
- _Actions_:
  - **Document invariant in `PinLockoutTracker` module regarding in-memory nature and restart forgiveness.** @ `crates/racecontrol/src/auth/staff_auth.rs` (✓ smallest-reversible)
  - **Add Prometheus counter `auth.pin_lockout_restart_count` to monitor restarts that reset counters.** @ `crates/racecontrol/src/auth/staff_auth.rs` (✓ smallest-reversible)
- _Rollback_: N/A (deferral actions are documentation/monitoring, not core logic).

#### mistral
- _Disposition_: in-PLAN-scope
- _PR_: PR-C-W1-S6-pin-lockout
- _Blast radius_: single-module; _v1-dependent_: false
- _Deploy targets_: server-23
- _Actions_:
  - **Document restart-forgiveness invariant in PinLockoutTracker module** @ `crates/racecontrol/src/auth/staff_auth.rs:1-20 comments` (✓ smallest-reversible)
  - **Add Prometheus counter auth.pin_lockout_restart_count** @ `crates/racecontrol/src/auth/staff_auth.rs:25 + comms-link/metrics.rs:130` (✓ smallest-reversible)
- _Tests_: pin_lockout_restart_count_monitored [integration]
- _Rollback_: Revert PR-C-W1-S6-pin-lockout comments and metrics counter.

---

### CF-10: [P0][CROSS] Auth boundary changes preserve audit_log schema + state-machine consistency

- **Models covered**: 3/5
- **Consensus disposition**: in-PLAN-scope (3/3)
- **Consensus PR assignment**: PR-E-cross-RCA-audit-log-doctrine (2/3)
- **Captain Q-DECISION required (per-model count)**: 0/5

**Per-model plans:**

#### qwen
- _Disposition_: in-PLAN-scope
- _PR_: PR-E-cross-RCA-audit-log-doctrine
- _Blast radius_: cross-crate; _v1-dependent_: false
- _Deploy targets_: server-23, bono-vps
- _Actions_:
  - **Define unified audit_log action_type vocabulary: jwt_refresh, staff_pin_auto_reset, etc.** @ `V2-MASTER-STATE.md §S-N` (✓ smallest-reversible)
- _Tests_: no_duplicate_audit_log_action_types [regression]
- _Rollback_: Revert vocabulary definition and allow duplicate action_types.

#### gemini
- _Disposition_: in-PLAN-scope
- _PR_: PR-E-cross-RCA-audit-log-doctrine
- _Blast radius_: cross-crate; _v1-dependent_: false
- _Deploy targets_: server-23, bono-vps
- _Actions_:
  - **Define unified `audit_log.action_type` vocabulary: `jwt_refresh`, `staff_pin_auto_reset`, `wallet_hold_created`, `wallet_hold_captured`, `wallet_hold_released`.** @ `crates/v2-db/src/audit_log.rs (enum/constants) + documentation` (✓ smallest-reversible)
  - **Implement audit logging for `jwt_refresh` (non-routine, e.g., first refresh after login), `staff_pin_auto_reset`, `wallet_hold_created`, `wallet_hold_captured`, `wallet_hold_released` using `log_admin_action`.** @ `crates/racecontrol/src/auth/middleware.rs, crates/racecontrol/src/auth/staff_auth.rs, crates/v2-db/src/wallets.rs` (⚠ not smallest-reversible)
  - **Add invariant comment: 'HOLDs are session-bound NOT auth-bound (release fires on session terminal-state, NOT JWT expiration).'** @ `crates/v2-db/src/wallets.rs (or relevant wallet HRC module)` (✓ smallest-reversible)
- _Tests_: audit_log_action_type_consistency [regression] · wallet_hold_release_on_session_terminal [integration]
- _Rollback_: Revert the audit_log vocabulary and all `log_admin_action` calls. This would remove critical audit trails.
- _Depends-on_: CF-12

#### mistral
- _Disposition_: in-PLAN-scope
- _PR_: PR-D-cross-RCA-audit-log-doctrine
- _Blast radius_: cross-crate; _v1-dependent_: false
- _Deploy targets_: server-23
- _Actions_:
  - **Define unified audit_log action_type vocabulary** @ `docs/v2-master-state/audit-log-doctrine.md:1-30` (✓ smallest-reversible)
  - **Cross-RCA regression test asserting no duplicate action_types** @ `tests/audit_log_cross_rca.rs:1-50` (✓ smallest-reversible)
- _Tests_: audit_log_action_type_uniqueness [regression]
- _Rollback_: Revert PR-D-cross-RCA-audit-log-doctrine.

---

### CF-11: [P0][W3] PACT-024 Q1-Q5 governance gate (RESOLVED via §S-151 — AMPLIFIED 2026-05-05)

- **Models covered**: 3/5
- **Consensus disposition**: RESOLVED-via-prior-action (3/3)
- **Consensus PR assignment**: deferred-PACT-024b (2/3)
- **Captain Q-DECISION required (per-model count)**: 0/5

**Per-model plans:**

#### qwen
- _Disposition_: RESOLVED-via-prior-action
- _PR_: deferred-PACT-024b
- _Blast radius_: governance-only; _v1-dependent_: false
- _Rollback_: N/A

#### gemini
- _Disposition_: RESOLVED-via-prior-action
- _PR_: N/A
- _Blast radius_: N/A; _v1-dependent_: false
- _Rollback_: N/A

#### mistral
- _Disposition_: RESOLVED-via-prior-action
- _PR_: deferred-PACT-024b
- _Blast radius_: none; _v1-dependent_: false
- _Actions_:
  - **Mark CF-11 RESOLVED-via-§S-151 (PACT-024 AMPLIFIED state confirmed)** @ `MMA-S2-W1S5-W1S6-W3-rust-axum-nextjs.json:11` (⚠ not smallest-reversible)
- _Rollback_: N/A

---

### CF-12: [P1][CROSS] Inconsistent audit-log discipline across foundational changes

- **Models covered**: 3/5
- **Consensus disposition**: in-PLAN-scope (3/3)
- **Consensus PR assignment**: PR-E-cross-RCA-audit-log-doctrine (2/3)
- **Captain Q-DECISION required (per-model count)**: 0/5

**Per-model plans:**

#### qwen
- _Disposition_: in-PLAN-scope
- _PR_: PR-E-cross-RCA-audit-log-doctrine
- _Blast radius_: cross-crate; _v1-dependent_: false
- _Deploy targets_: server-23, bono-vps
- _Actions_:
  - **Establish V2 Audit-Log Doctrine: log state-changing events, NOT routine refreshes** @ `V2-MASTER-STATE.md §S-N` (✓ smallest-reversible)
- _Tests_: audit_log_discipline_applied [regression]
- _Rollback_: Revert audit-log doctrine to inconsistent strategy.

#### gemini
- _Disposition_: in-PLAN-scope
- _PR_: PR-E-cross-RCA-audit-log-doctrine
- _Blast radius_: cross-crate; _v1-dependent_: false
- _Deploy targets_: server-23, bono-vps
- _Actions_:
  - **Establish V2 Audit-Log Doctrine: log state-changing events (PIN-rotate, hold/release/capture) but NOT routine token refreshes.** @ `V2-MASTER-STATE §S-N (new section in documentation)` (✓ smallest-reversible)
  - **Ensure all three RCAs use `log_admin_action` with distinct `action_type` values per CF-10.** @ `crates/racecontrol/src/auth/middleware.rs, crates/racecontrol/src/auth/staff_auth.rs, crates/v2-db/src/wallets.rs` (⚠ not smallest-reversible)
- _Tests_: audit_log_doctrine_adherence [integration]
- _Rollback_: Revert the V2 Audit-Log Doctrine documentation and any changes to `log_admin_action` calls. This would revert to inconsistent logging.
- _Depends-on_: CF-10

#### mistral
- _Disposition_: in-PLAN-scope
- _PR_: PR-D-cross-RCA-audit-log-doctrine
- _Blast radius_: cross-crate; _v1-dependent_: false
- _Deploy targets_: server-23
- _Actions_:
  - **Establish V2 Audit-Log Doctrine: log state-changing events only** @ `docs/v2-master-state/audit-log-doctrine.md:31-60` (✓ smallest-reversible)
  - **Update all RCAs to use distinct action_type values per CF-10** @ `crates/racecontrol/src/auth/middleware.rs:103-110, crates/racecontrol/src/auth/staff_auth.rs:110-120, crates/v2-db/src/wallets.rs:300-320` (✓ smallest-reversible)
- _Tests_: audit_log_discipline_state_changes_only [regression]
- _Rollback_: Revert PR-D-cross-RCA-audit-log-doctrine.
- _Depends-on_: CF-10

---

### CF-13: [P1][W3] PACT-024a §A SQL/Rust patterns must re-target to v2-db crate

- **Models covered**: 2/5
- **Consensus disposition**: in-PLAN-scope (2/2)
- **Consensus PR assignment**: PR-A-W3-schema (1/2)
- **Captain Q-DECISION required (per-model count)**: 1/5

**Per-model plans:**

#### qwen
- _Disposition_: in-PLAN-scope
- _PR_: PR-A-W3-schema
- _Blast radius_: documentation-only; _v1-dependent_: false
- _Actions_:
  - **Re-target PACT-024a §A file paths to crates/v2-db/src/wallets.rs** @ `PACT-024a.md §A` (✓ smallest-reversible)
- _Rollback_: Revert file paths to reference racecontrol crate.
- _Depends-on_: Captain G33 Q-W3-RECONCILE-1

#### mistral
- _Disposition_: in-PLAN-scope
- _PR_: PR-E-governance-PACT-024
- _Blast radius_: single-crate; _v1-dependent_: false
- _Deploy targets_: server-23
- _Actions_:
  - **Re-target PACT-024a §A file paths to crates/v2-db/src/wallets.rs** @ `docs/pacts/PACT-024a.md:1-10` (✓ smallest-reversible)
  - **Update §A SQL migration to target v2-db schema** @ `migrations/20260515_pact024a_v2db.sql:1-30` (✓ smallest-reversible)
- _Tests_: pact024a_v2db_schema_migration_applies [integration]
- _Rollback_: Run rollback SQL; revert PR-E-governance-PACT-024.

---

### CF-14: [P2][W3] Orphan hold cleanup deferred to PACT-024b Kidneys reconciliation

- **Models covered**: 2/5
- **Consensus disposition**: explicit-defer-with-rationale (1/2)
- **Consensus PR assignment**: deferred-PACT-024b (1/2)
- **Captain Q-DECISION required (per-model count)**: 0/5

**Per-model plans:**

#### qwen
- _Disposition_: explicit-defer-with-rationale
- _PR_: deferred-PACT-024b
- _Blast radius_: schema; _v1-dependent_: false
- _Deploy targets_: server-23
- _Rollback_: N/A

#### mistral
- _Disposition_: in-PLAN-scope
- _PR_: PR-A-W3-schema
- _Blast radius_: single-doc; _v1-dependent_: false
- _Actions_:
  - **Document orphan-hold deferral to PACT-024b in W3 spec** @ `docs/v2-wave-1/w3-wallet-hold-release-capture.md:200-220` (✓ smallest-reversible)
  - **Add hold_timeout_secs config default deferred** @ `config/services.rs:550 + docs/v2-wave-1/w3-config.md:45` (✓ smallest-reversible)
  - **Add monitoring alert if open holds older than 24h accumulate** @ `comms-link/alerts/hold_orphan_alert.yml` (✓ smallest-reversible)
- _Tests_: orphan_hold_deferral_monitored [integration]
- _Rollback_: Revert PR-A-W3-schema docs and config changes.

---

### CF-S1: [P0][CROSS] Sliding-window JWT refresh outlives PIN-LOCKOUT auto-rotation

- **Models covered**: 2/5
- **Consensus disposition**: in-PLAN-scope (2/2)
- **Consensus PR assignment**: PR-D-W1-S6-pin-lockout (1/2)
- **Captain Q-DECISION required (per-model count)**: 0/5

**Per-model plans:**

#### qwen
- _Disposition_: in-PLAN-scope
- _PR_: PR-D-W1-S6-pin-lockout
- _Blast radius_: cross-module; _v1-dependent_: false
- _Deploy targets_: server-23, bono-vps
- _Actions_:
  - **Add jwt_generation counter to staff record; refresh checks and rejects if behind** @ `crates/racecontrol/src/auth/staff_auth.rs + middleware.rs` (✓ smallest-reversible)
- _Tests_: pin_rotate_invalidates_existing_jwt [integration]
- _Rollback_: Remove jwt_generation logic and revert to naive refresh.

#### mistral
- _Disposition_: in-PLAN-scope
- _PR_: PR-B-W1-S5-auth-refresh
- _Blast radius_: cross-crate; _v1-dependent_: false
- _Deploy targets_: server-23
- _Actions_:
  - **Add jwt_generation counter to staff record; refresh path checks and rejects if behind** @ `crates/racecontrol/src/auth/models.rs:30-40 + middleware.rs:260-270` (✓ smallest-reversible)
  - **Add integration test PIN rotate → next refresh → 401** @ `tests/auth_pin_rotate_refresh.rs:1-60` (✓ smallest-reversible)
- _Tests_: pin_rotate_invalidates_outstanding_jwts [integration]
- _Rollback_: Revert PR-B-W1-S5-auth-refresh staff model and middleware changes.
- _Depends-on_: CF-1 · CF-9

---

### CF-S2: [P1][W1-S5] Inter-host clock-skew under sliding-window between Server .23 + Bono VPS

- **Models covered**: 2/5
- **Consensus disposition**: in-PLAN-scope (2/2)
- **Consensus PR assignment**: PR-C-W1-S5-auth-refresh (1/2)
- **Captain Q-DECISION required (per-model count)**: 0/5

**Per-model plans:**

#### qwen
- _Disposition_: in-PLAN-scope
- _PR_: PR-C-W1-S5-auth-refresh
- _Blast radius_: single-module; _v1-dependent_: true
- _Deploy targets_: server-23, bono-vps
- _Actions_:
  - **Add max_iat_skew_secs config (default 60s); reject tokens where iat > now + skew** @ `crates/racecontrol/src/auth/middleware.rs::is_idle_expired` (✓ smallest-reversible)
- _Tests_: iat_skew_rejected_beyond_tolerance [unit]
- _Rollback_: Remove skew check and revert to saturating_sub behavior.

#### mistral
- _Disposition_: in-PLAN-scope
- _PR_: PR-B-W1-S5-auth-refresh
- _Blast radius_: cross-system; _v1-dependent_: true
- _Deploy targets_: server-23, bono-vps
- _Actions_:
  - **Bound max-allowed iat-skew between hosts to 60s; reject tokens where iat > now + skew_tolerance** @ `crates/racecontrol/src/auth/middleware.rs:120-130 + config/services.rs:160` (✓ smallest-reversible)
  - **Document NTP discipline at Server .23 and Bono VPS** @ `docs/ops/ntp-discipline.md:1-20` (✓ smallest-reversible)
- _Tests_: jwt_iat_skew_tolerance_60s [integration]
- _Rollback_: Revert PR-B-W1-S5-auth-refresh middleware.rs and config/services.rs changes.

---

## §2 — Cross-finding design couplings

- (qwen) CF-5 + CF-7 share email_alerts.rs surface; resolve via CF-7 bypass-cooldown path
- (qwen) CF-1 + CF-S1 both involve JWT lifecycle; CF-S1 adds invalidation on PIN rotate
- (qwen) CF-10 + CF-12 define unified audit_log vocabulary and discipline respectively
- (mistral) CF-5 + CF-7 share email_alerts.rs surface; resolved via CF-7 bypass-cooldown path (send_pin_rotation method)
- (mistral) CF-10 + CF-12 share audit_log action_type vocabulary; resolved via unified doctrine doc and regression test
- (mistral) CF-1 + CF-S1 share auth refresh path; resolved via jwt_generation counter in staff model
- (mistral) CF-9 + CF-S1 share staff_auth.rs module; resolved via restart-forgiveness doc and PIN rotate invalidation

## §3 — Captain Q-DECISIONs surfaced by Step 2

- (qwen) Q-S6-6 lockout durability ACCEPT-DEFAULT (CF-9)
- (qwen) Q-S6-1 cooldown bypass ACCEPT-DEFAULT (CF-7)
- (qwen) Q-S5-7 plan-author typo ACCEPT-DEFAULT (CF-3)
- (mimo) Q-S2-1: SMTP probe failure → ship-with-risk vs delay W1-S6 (default: ship-with-risk + monitor)
- (mimo) Q-S2-2: JWT invalidation method (generation counter vs deny-list) (default: generation counter on staff record)
- (gemini) Q-S2-1: CF-6 SMTP/DKIM/SPF verification outcome (ship-with-risk vs delay W1-S6)
- (gemini) Q-S2-2: CF-S2 max_iat_skew_secs default value (60s proposed)
- (mistral) Q-S2-N: Accept restart-forgiveness for in-memory PIN-LOCKOUT (CF-9) per CR-3
- (mistral) Q-S6-6: Accept SMTP unverified risk with monitoring (CF-6)

## §4 — Open risks

- (qwen) SMTP/DKIM/SPF verification at Server .23 and Bono VPS requires Captain Q-DECISION if unverified
- (mistral) CF-6 requires Captain Q-DECISION on ship-with-risk vs delay until DKIM/SPF verified
- (mistral) CF-9 requires Captain Q-S2-N acceptance of restart-forgiveness per CR-3

## §5 — Step 2 → Step 3 EXECUTE + Step 4 VERIFY hand-off

Per Protocol v3.0:
1. **Step 3 EXECUTE** selects ONE plan as the candidate baseline (highest-coverage + smallest-reversible-change), then layers in consensus-merged actions from §1.
2. Per-PR Captain merge auth at PR-open (foundational-boundary class).
3. **Step 4 VERIFY** runs ≥3 adversarial models DIFFERENT from Step 1-3 panels (vendor-disjoint requirement carries forward); evaluates Step 1 MINORITY findings + adversarial review of selected plan; ≥4.0/5 score gate.
4. RATIFY trigger fires per PACT-024 §2.1 OPTION-A composite-#4 path-c on Steps 2+3+4-COMPLETE auto-cascade.

**MANDATORY for Step 4 VERIFY** (not ranking — must be evaluated):
- Step 1 MINORITY findings (5): V2.1 PACT pin stale, concurrent hold atomicity hole, response-mutating middleware precedent, refund-during-HOLD ordering, idle-refresh cookie collision
- Captain Q-DECISIONs surfaced by §3 above (if any) — Step 4 confirms default OR escalates
- Cross-finding couplings (§2) — Step 4 sweep confirms no design contradictions

**Branch**: `feat/v2-wave-1-w1-s1-billing-service` HEAD (this commit). No implementation yet — planning artifact only.

---

## §6 — NOT TESTED (per CGP H3)

This Step 2 PLAN is design substrate. NOT TESTED at this gate:
- Step 3 EXECUTE selection + smallest-reversible-change implementation
- Step 4 VERIFY adversarial scoring (≥3 models different from Steps 1-3, ≥4.0 gate)
- Captain G33 W3 batch disposition (11 Q-DECISIONs; ACCEPT-ALL-DEFAULTS-precedent established at `bda06dc8`)
- Per-PR Captain merge auth at every PR-open (4-5 PRs in scope per §0.1)
- F-05 anti-pattern lint codification (sub-PACT candidate; deferred to Standing Rule)
- Sub-step orchestration: Wave 1 closure (W1-S6/S7/S8 + Quality Gate + E2E + visual + venue Server .23 rebuild)
- Cross-pilot bono shared/wallet-client.js Idempotency-Key wrappers (PR-D depends; bono-side ship)
- Cumulative MMA-day spend audit (this batch additive)

---

## §7 — Footer

- Generated by `.tmp/mma-step2-synthesis.js` from `.tmp/mma-step2-results.json`
- Step 1 canonical input: `.planning/specs/v2/MMA-STEP-1-W1S5-W1S6-W3-CONSENSUS.md`
- Step 1 supplementary input: `.planning/specs/v2/MMA-W1-S5-W1-S6-DIAGNOSE/SYNTHESIS.md`
- Spend audit-trail: `comms-link/data/openrouter-spend-james.jsonl` (this batch entry appended)
- LOGBOOK row: this batch ship row
- V2-MASTER-STATE: §S-N closure ledger (this commit)