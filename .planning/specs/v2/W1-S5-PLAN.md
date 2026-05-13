# W1-S5-PLAN — Sliding-window auth refresh + idle-timeout (PR-C, Wave 1, gates on PR-A merge)

**Scope**: cascade #7 detail PLAN derived from `W1-S5-RCA.md` (post-PR #67 `7dcedd00`); incorporates Captain G33 v5 #2 + #3 + #4 + #5 + #7 + #8 ratifications + Wave A.2.1 §2–§9 substrate-flaw closures (consumer side of F-CONS-15 cross-coupling).

**Authored**: 2026-05-09 ~20:34 IST · **Authored-by**: james (Claude Opus 4.7 1M)
**Class**: H1 PLAN file derived per RCA; foundational-boundary class (auth)
**Status**: SHIPPED — Captain Option C hybrid (2026-05-09 ~20:28 IST) authorized cascade #7 authoring; PR-C opens AFTER PR-A merges per Q-W1-CROSS-2-a sequencing
**Sequencing**: PR-A W1-S6 (publisher; merged) → **PR-C W1-S5 (this PLAN; consumer)** → PR-D W3 (independent)

**Authoritative substrate**:
- `W1-S5-RCA.md` racecontrol `7dcedd00` — boundary map at `auth/middleware.rs` 10 file/line rows + 11-issue inherited catalogue + V2-alignment delta + 5-section RCA
- `MMA-STEP-2-W1S5-W1S6-W3-PLAN-A2.md` Wave A.2 + `MMA-STEP-2-W1S5-W1S6-W3-PLAN-A2-1.md` Wave A.2.1 (substrate-flaw closure)
- `MMA-STEP-1-W1S5-W1S6-W3-CONSENSUS.md` + `MMA-STEP-1-AMENDED-W1S5-W1S6-CONSENSUS.md` Step 1 panels
- `MMA-STEP-4-VERIFY-W1S5-W1S6-W3-CONSENSUS.md` Step 4 VERIFY (FAIL → Option C remediation)
- `W1-S6-PLAN.md` (this turn ship) — publisher-side contract for cross-coupling

**V2 doctrine alignment**: §AMEND-3.II D12 Foundation/Strategy/Config separation (clock skew tolerance is config) · §S-158 V2 Audit-Log Doctrine · NEW-Q-1 F-05 anti-pattern lint codification · §S-146 V1↔V2 RCA rule SECOND end-to-end pipeline application Step 3 EXECUTE input.

---

## §1 — Goal

Implement V2 sliding-window auth refresh path with idle-timeout AND:
1. Q-W1-CROSS-1 lockout-state predicate consumption from W1-S6 publisher (security-class explicit)
2. Q-W1-S5-NEW-1 12h max-session-life HARD cap since `iat_original` claim
3. Q-S5-NEW-2 force-expire JWT on lockout-active (default-Y)
4. Wave A.2.1 §3 F-CONS-15 atomic TOCTOU fix between predicate-check and revocation
5. Wave A.2.1 §4 F-CONS-16 single-flight refresh mutex (single-node scoped V2.0)
6. Wave A.2.1 §2 F-CONS-17 multi-host clock skew tolerance ±5s
7. Wave A.2.1 §5 F-CONS-5 HashMap pruning (V1 inheritance closure)
8. Wave A.2.1 §6 F-CONS-2 F-05 regression test scope extension
9. Wave A.2.1 §7 novel deadlock proof — lock order (b) lockout-check THEN (a) refresh
10. F-AMEND-CONS-7 cashier-default role preservation on refresh
11. F-AMEND-SING-5 `refresh_lock_wait_time` histogram metric

**V2 customer impact:** staff JWT tokens refresh seamlessly while session is active; locked-out staff cannot refresh past the lockout boundary; max session life capped at 12h since first-issued; multi-host clock skew tolerated within ±5s; race conditions between lockout-check and revocation are atomic.

**V1 inheritance:** existing JWT refresh path at `auth/middleware.rs` (per W1-S5-RCA.md §1 boundary map); V2 retains JWT contract, replaces refresh logic per RCA.

---

## §2 — Pre-PR-C operational checks

1. **PR-A merged confirmation:** `git log` shows W1-S6 PR-A merged on `feat/v2-wave-1-w1-s1-billing-service` OR cherry-picked into PR-C base
2. **`LockoutManager` API surface stable:** `Arc<LockoutManager>` injected at racecontrol startup; `is_locked_out(staff_id)` callable
3. **DB migration sequencing:** PR-A migration `20260510000001_pin_lockout_state_v2.sql` already run; PR-C migration `20260510000002_v2_refresh_token_v2.sql` adds `refresh_tokens` table fields
4. **Smoke test against PR-A artifacts:** PR-C local test harness imports `racecontrol::auth::lockout::LockoutManager` and instantiates a mock for tests

---

## §3 — Scope (this PLAN's PR-C deliverable)

### §3.1 — File targets

Primary touch (V2 boundary):
- `crates/racecontrol-crate/src/auth/middleware.rs` — V2 refresh path; V1 path retained behind feature flag for rollback (per Wave A.2 §F-CONS-7 bypass disposition)
- `crates/racecontrol-crate/src/auth/refresh.rs` — **NEW** — `RefreshHandler` + sliding-window state + idle-timeout + 12h cap
- `crates/racecontrol-crate/src/auth/refresh_mutex.rs` — **NEW** — F-CONS-16 single-flight mutex (per-staff-id, single-node scoped)
- `crates/racecontrol-crate/src/auth/refresh_pruner.rs` — **NEW** — F-CONS-5 HashMap pruning background task
- `crates/racecontrol-crate/src/auth/clock.rs` — **NEW** — F-CONS-17 clock skew tolerance helper (`now_with_skew`, `validate_iat_with_skew`)

Test targets:
- `crates/racecontrol-crate/tests/auth/refresh_test.rs` — **NEW**
- `crates/racecontrol-crate/tests/auth/refresh_mutex_test.rs` — **NEW**
- `crates/racecontrol-crate/tests/auth/refresh_pruner_test.rs` — **NEW**
- `crates/racecontrol-crate/tests/auth/clock_skew_test.rs` — **NEW**
- `crates/racecontrol-crate/tests/auth/lockout_consumer_test.rs` — **NEW** (F-CONS-15 cross-coupling consumer contract — verifies subscription to PR-A `LockoutManager`)
- `crates/racecontrol-crate/tests/auth/deadlock_proof_test.rs` — **NEW** (Wave A.2.1 §7 novel-finding load-simulation)
- `crates/racecontrol-crate/tests/auth/cashier_default_role_test.rs` — **NEW** (F-AMEND-CONS-7)

DB schema:
- `crates/v2-db/migrations/20260510000002_v2_refresh_token_v2.sql` — `refresh_tokens` table V2 schema (extends V1; adds `iat_original` column for 12h cap; adds `last_used_at` for sliding-window)

Config:
- `crates/racecontrol-crate/src/config.rs` — add `auth.idle_timeout_secs` (default 1800 / 30min), `auth.max_session_life_secs` (default 43200 / 12h), `auth.clock_skew_tolerance_secs` (default 5, hard-cap 30), `auth.refresh_mutex_distributed_keys` (default false; V2.1+ forward-compat hook)

### §3.2 — Out of scope

- W1-S6 publisher side (PR-A merged)
- W3 wallet HRC (PR-D separate)
- Distributed-lock implementation (V2.0 single-node only; V2.1+ if needed)

---

## §4 — Q-DECISION incorporation specs

### §4.1 — Q-W1-CROSS-1 (security-class explicit) — consumer side

**Spec (per W1-S6-PLAN.md §4.1 publisher contract):**
- `RefreshHandler::handle()` consumes `Arc<LockoutManager>` injected at startup
- BEFORE refresh: `lockout_mgr.is_locked_out(staff_id).await?` — if `Active` → 401 + `LockoutPredicate.until_iso8601` in response body
- Code-comment at consumption site: `// Q-W1-CROSS-1: SECURITY-CLASS EXPLICIT — refresh path consults publisher predicate per W1-S6-PLAN §4.1; bypass = gate violation`
- Test `refresh_blocked_when_lockout_predicate_active` + `refresh_proceeds_when_lockout_predicate_inactive`

### §4.2 — Q-W1-S5-NEW-1 (option-b 12h HARD cap since `iat_original`)

**Spec (Captain G33 v5 #4 verbatim):** option-b accepted · max-session-life HARD cap = 12h since `iat_original` claim · refresh path MUST verify and reject beyond cap; no extension by re-issuance.

**Implementation:**
- DB schema: `refresh_tokens.iat_original BIGINT NOT NULL` (Unix timestamp of FIRST issuance; preserved across refreshes — never overwritten)
- JWT claim: `iat_original` (in addition to standard `iat`); first-issued claim survives refresh
- Validation in `RefreshHandler::handle()`:
  ```
  let now = chrono::Utc::now().timestamp();
  let iat_original = claims.iat_original;
  if now - iat_original > config.auth.max_session_life_secs {
      // 12h HARD cap exceeded; reject; require re-login
      return Err(RefreshError::MaxSessionLifeExceeded);
  }
  ```
- Code-comment: `// Q-W1-S5-NEW-1 option-b: 12h HARD cap since iat_original; rejection forces re-login; no extension via refresh`
- Test `refresh_succeeds_within_12h_cap` + `refresh_rejects_at_12h_boundary` + `refresh_rejects_beyond_12h_cap` + `refresh_iat_original_preserved_across_refreshes`

### §4.3 — Q-S5-NEW-2 (force-expire JWT on lockout-active default-Y)

**Spec (Captain G33 v5 #5 verbatim):** YES · audit-log row per F-AMEND-CONS-6 · code-comment blast-radius statement at the rejection site.

**Implementation:**
- When `lockout_mgr.is_locked_out(staff_id)` returns `Active`:
  1. Force-expire ALL active JWTs for `staff_id` via `RefreshTokenMap::revoke_all_for_staff(staff_id)`
  2. Audit-log row via `log_admin_action("refresh_force_expired_lockout_active", staff_id, payload)` — `action_type` enum-bounded by V2 Audit-Log Doctrine
  3. Return 401 + `LockoutPredicate.reason` to caller
- Code-comment (blast-radius statement per Captain spec): `// Q-S5-NEW-2: force-expire on lockout-active. BLAST RADIUS: ALL active JWTs for this staff_id revoked across racecontrol (including parallel sessions on POS / kiosk / admin); customer-impact = staff must re-login on every surface; this is INTENTIONAL — lockout is security-critical`
- Test `force_expire_revokes_all_staff_tokens` + `force_expire_audit_log_row_written` + `force_expire_action_type_passes_check_constraint`

### §4.4 — Wave A.2.1 §3 F-CONS-15 atomic TOCTOU fix

**Spec (per Wave A.2.1 §3):**
- TWO single-flight mutex layers; lock order: (b) lockout-check-and-revoke FIRST, (a) refresh SECOND
- `LockoutCheckGuard` RAII guard implements (b)
- `RefreshSingleFlight` implements (a) per Wave A.2.1 §4
- Refresh-path control flow exactly as Wave A.2.1 §7 spec:
  ```
  1. ACQUIRE (b) per-staff lockout-check-and-revoke mutex
  2. CHECK lockout predicate
  3. IF locked-out: revoke active tokens; RELEASE (b); RETURN 401
  4. RELEASE (b)
  5. ACQUIRE (a) per-staff refresh single-flight mutex
  6. PERFORM refresh
  7. RELEASE (a)
  ```
- Code-comment: `// F-CONS-15+novel-deadlock proof: lock order = (b) lockout-check THEN (a) refresh; (b) released before (a) acquired; lockout-revocation from W1-S6 acquires (b) only`
- Test `lockout_toctou_concurrent_check_and_revoke` + `lockout_lock_order_verified` + `lockout_lock_release_on_revoke_error`

### §4.5 — Wave A.2.1 §4 F-CONS-16 single-flight mutex (single-node scoped V2.0)

**Spec (per Wave A.2.1 §4):**
- `RefreshSingleFlight: tokio::sync::Mutex<HashMap<StaffId, Arc<tokio::sync::Mutex<()>>>>` — process-local
- Code-comment at declaration: `// F-CONS-16: single-flight scoped to single-host racecontrol (V2.0 topology = .23 active OR Bono VPS failover, never both); distributed-coord NOT required. Re-evaluate at multi-active scope expansion.`
- Anti-precedent comment: `// F-CONS-16 ANTI-PRECEDENT: do not introduce multi-active racecontrol topology without distributed-lock keys`
- Forward-compat: `auth.refresh_mutex_distributed_keys` env-var (default `false`; V2.1+ activation triggers MMA Step 1 DIAGNOSE on multi-active)
- Test `single_flight_serializes_concurrent_refreshes_for_same_staff` + `single_flight_allows_parallel_refreshes_for_different_staff` + `single_flight_lock_wait_time_metric_emits`

### §4.6 — Wave A.2.1 §2 F-CONS-17 clock skew tolerance ±5s

**Spec (per Wave A.2.1 §2):**
- `validate_iat_with_skew(token_iat, server_now, tolerance) -> Result<(), ClockSkewError>`
- Tolerance default 5s; configurable via `auth.clock_skew_tolerance_secs`; hard-capped 30s
- Server-side NTP sync invariant documented (existing infra; not new component)
- Code-comment: `// F-CONS-17: clock skew tolerance — DO NOT remove without re-evaluating multi-host deployment topology`
- Test `clock_skew_within_tolerance` + `clock_skew_beyond_tolerance` + `clock_skew_negative_drift` + `clock_skew_at_boundary` + `clock_skew_metric_increments` (`auth_clock_skew_rejections_total` counter)

### §4.7 — Wave A.2.1 §5 F-CONS-5 HashMap pruning

**Spec (per Wave A.2.1 §5):**
- On revocation: retain non-revoked tokens; remove empty staff entries
- TTL background task `auth_refresh_token_pruner` — runs every 60s; removes expired tokens (with skew tolerance) + empty entries
- Bounded growth metric `auth_refresh_token_map_size` gauge
- Code-comment: `// F-CONS-5 V1-inheritance: bounded growth required; pruning on revoke + TTL background task; do not skip either path`
- Test `pruning_on_revoke_removes_token` + `pruning_on_revoke_removes_empty_staff_entry` + `pruning_background_task_removes_expired` + `bounded_growth_under_load_simulation`

### §4.8 — Wave A.2.1 §6 F-CONS-2 F-05 regression test scope extension (NEW-Q-1)

**Spec:**
- `f05_anti_pattern_w1_s5_refresh_path` test — UPDATE `refresh_tokens.last_used_at`; assert SELECT immediately after returns the original `last_used_at` value if a snapshot-read-before-write was taken (F-05 violation = SELECT returns just-written value)
- Code-comment at refresh path UPDATE site: `// F-05 OK: explicit snapshot-read-before-write; anti-pattern guard via f05_anti_pattern_w1_s5_refresh_path test`

### §4.9 — Wave A.2.1 §7 novel deadlock proof

**Spec (per Wave A.2.1 §7):**
- Test `concurrent_refresh_and_lockout_no_deadlock` — load simulation: 100 concurrent refreshes + 50 concurrent lockout-revocations across 10 staff_ids; no test exceeds 5s; no deadlock detected (`tokio::test` with `#[tokio::test(flavor = "multi_thread", worker_threads = 8)]`)
- Lock-order code-review checklist documented in `crates/racecontrol-crate/src/auth/LOCK-ORDER.md` (NEW)

### §4.10 — F-AMEND-CONS-7 cashier-default role preservation

**Spec (per Wave A.2 §5 item 4 + Wave A.2.1 §8):**
- `create_staff_jwt_with_role(claims.role)` — role pulled from EXISTING claim, NOT hardcoded default
- Test `cashier_default_role_preservation_on_refresh` + `manager_role_preservation_on_refresh` + `superadmin_role_preservation_on_refresh` + `role_downgrade_detected_and_rejected`

### §4.11 — F-AMEND-SING-5 + Wave A.2.1 §9 observability metrics

**Metrics emitted by this PR-C:**
- `refresh_lock_wait_time_seconds` histogram (F-AMEND-SING-5)
- `auth_refresh_token_map_size` gauge (F-CONS-5)
- `auth_clock_skew_rejections_total` counter (F-CONS-17)
- `auth_lockout_check_revoke_duration_seconds` histogram (F-CONS-15)
- `auth_max_session_life_rejections_total` counter (Q-W1-S5-NEW-1)
- `auth_force_expire_lockout_active_total` counter (Q-S5-NEW-2)
- `auth_refresh_outcome_total{outcome=ok|locked|max_life|skew|err}` counter

---

## §5 — Test coverage matrix

All tests in §4.* are mandatory. Aggregated count: ~28 unit tests + 4 integration tests + 1 deadlock-proof load-simulation. PR-C MUST have all green before merge.

### §5.1 — Integration tests

- `e2e_refresh_within_window_succeeds`
- `e2e_refresh_idle_timeout_rejects` (no refresh in 30min → next refresh 401)
- `e2e_refresh_12h_cap_rejects_at_boundary`
- `e2e_refresh_lockout_active_force_expires_all_tokens`

### §5.2 — F-05 anti-pattern regression test (extended scope per Wave A.2.1 §6)

- `f05_anti_pattern_w1_s5_refresh_path` (W1-S5 path)
- `f05_anti_pattern_w3_wallet_hrc_path` (W3 path; cross-PR — covered in W3-PLAN.md)
- `f05_anti_pattern_w1_s6_lockout_count_path` (W1-S6 path; cross-PR — covered in W1-S6-PLAN.md)

### §5.3 — V2 Audit-Log Doctrine compliance test (NEW-Q-2)

- `audit_log_action_type_check_constraint_includes_refresh_force_expired_lockout_active` (DB-level CHECK constraint)

---

## §6 — Risk + rollback

### §6.1 — Risks

| Risk | Class | Mitigation |
|------|-------|-----------|
| TOCTOU between lockout-check and revocation | **P0 closed** | F-CONS-15 atomic-fix per §4.4 |
| Multi-host clock skew rejecting valid tokens | **P1 closed** | F-CONS-17 ±5s tolerance per §4.6 |
| Multi-active racecontrol breaks single-flight | P2 forward | F-CONS-16 anti-precedent comment + V2.0 single-node scope |
| HashMap unbounded growth (V1 inheritance) | **P1 closed** | F-CONS-5 pruning per §4.7 |
| Refresh-mutex deadlock under PIN-LOCKOUT revocation | **P1 closed** | Wave A.2.1 §7 lock-order proof + load-sim test per §4.9 |
| Cashier role downgrade on refresh | **P1 closed** | F-AMEND-CONS-7 test per §4.10 |
| 12h cap rejection during active session | P2 user-facing | Captain Q-W1-S5-NEW-1 ratified; user-facing message must explain re-login required |
| F-05 anti-pattern reintroduction in refresh path | **P1 closed** | F-CONS-2 regression test per §4.8 |
| PR-A `LockoutManager` API surface change after PR-C author | P2 sequencing | PR-C author waits for PR-A merged; cherry-pick path documented in §2 |

### §6.2 — Rollback path

1. Feature-flag the V2 refresh path: `auth.use_v2_refresh` env-var (default `true`); set `false` to revert to V1
2. V1 path retained UNTOUCHED in `auth/middleware.rs` until V2 is in production for >7 days with zero P0/P1 incidents
3. Revert PR-C merge: clean revert; rollback migration `20260510000002_v2_refresh_token_v2.down.sql` drops `iat_original`, `last_used_at` columns
4. Cross-PR: PR-A (W1-S6) stays merged on rollback (publisher is no-op without consumer)

### §6.3 — Cross-PR rollback safety

- PR-C rollback does NOT revert PR-A
- PR-A `LockoutManager::subscribe()` API remains; no consumers wired (PR-A original behavior)

---

## §7 — Deploy section

```yaml
deploy:
  rust_binary: racecontrol (Server .23 + Bono VPS cloud)
  frontend_rebuild: none
  config_change: racecontrol.toml — add auth.idle_timeout_secs, auth.max_session_life_secs, auth.clock_skew_tolerance_secs, auth.refresh_mutex_distributed_keys, auth.use_v2_refresh
  db_migration: 20260510000002_v2_refresh_token_v2.sql (depends on 20260510000001 from PR-A)
  infrastructure: Server .23 NTP sync verified; Bono VPS NTP sync verified (existing infra; document in PR-C pre-merge probe)
  data_files: none
  bat_file: none
  cloud_parity: REQUIRED — both Server .23 and Bono VPS run V2 refresh code; both run migration; clock skew tolerance must be tested cross-host
  targets: [Server .23, Bono VPS]
  pre_deploy_smoke: NTP sync probe + DB migration smoke + PR-A LockoutManager API surface compatibility verified
  post_deploy_verify:
    - racecontrol /api/v1/health build_id matches
    - existing staff JWT refreshes succeed
    - lockout-active staff cannot refresh (V1 + V2 paths both enforce)
    - 12h cap rejections appear in metrics post-test-event
    - clock_skew_rejections counter increments under simulated skew
    - F-05 regression test passes in CI
```

---

## §8 — Q-DECISION compliance map

| Ratification | This W1-S5-PLAN.md status |
|--------------|--------------------------|
| Q-W1-CROSS-1 | §4.1 — consumer-side spec (with `LockoutManager` injection) |
| Q-W1-CROSS-2 | §1+§3 — gates on PR-A merge; sequencing locked |
| Q-W1-S5-NEW-1 | §4.2 — option-b 12h HARD cap since `iat_original` |
| Q-S5-NEW-2 | §4.3 — force-expire on lockout-active + audit-log row + blast-radius comment |
| Q-W1-S6-NEW-2 | NOT APPLICABLE (W1-S6 detail PLAN) |
| NEW-Q-1 | §4.8 + §5.2 — F-05 regression test scope extension |
| NEW-Q-2 | §4.3 — V2 Audit-Log Doctrine `action_type` CHECK constraint extension |

Plus Wave A.2.1 substrate-flaw closures: §4.4 (F-CONS-15) + §4.5 (F-CONS-16) + §4.6 (F-CONS-17) + §4.7 (F-CONS-5) + §4.8 (F-CONS-2) + §4.9 (novel deadlock) + §4.10 (F-AMEND-CONS-7) + §4.11 (F-AMEND-SING-5).

---

## §9 — Cascade transition

| Cascade item | Pre-this-PLAN | Post-this-PLAN |
|--------------|---------------|----------------|
| #7 W1-S5-PLAN.md authoring | gates on Captain G33 v6 Option C | **SHIPPED** (this turn) |
| #8 PR-A opens W1-S6 FIRST | NOW UNBLOCKED post-#7 | **NOW UNBLOCKED — PR-A authoring next; PR-open auth required at commit time per G33 v5 #9** |
| Future PR-C opens W1-S5 | gates on PR-A merge | gates on PR-A merge (same; sequencing preserved) |

---

## §10 — NOT TESTED

- F-CONS-16 V2.1+ multi-active topology — explicitly out of scope; forward-compat hook only
- F-05 lint feasibility study — see Wave A.2.1 §6
- helpdesk@ mailbox — see W1-S6-PLAN.md §10 (cross-PR dependency)
- 12h cap UX message — exact user-facing message text deferred to PR-C author time
- NTP sync state at Bono VPS — to be probed pre-PR-C-merge
- Per-PR Captain merge auth — required at PR-open per G33 v5 #9
- bono AMPLIFIER absorption — deferred to next bilateral cycle
- Distributed-lock activation under V2.1+ — gates on multi-active topology PACT (out of scope)

---

— james / 2026-05-09 ~20:34 IST · W1-S5-PLAN.md SHIPPED · cascade #7 detail PLAN derived from W1-S5-RCA.md `7dcedd00` + Wave A.2.1 substrate-flaw closures · 6 Captain G33 v5 ratifications incorporated (Q-W1-CROSS-1+2 + Q-W1-S5-NEW-1 + Q-S5-NEW-2 + NEW-Q-1 + NEW-Q-2) + 8 Wave A.2.1 substrate items (F-CONS-15/16/17/5/2 + novel deadlock + F-AMEND-CONS-7 + F-AMEND-SING-5) · Captain Option C hybrid authorization · per-PR Captain merge auth required at PR-open per G33 v5 #9 · 0 G9 self-caught
