# W1-S6-PLAN — PIN-LOCKOUT auto-rotate (PR-A, Wave 1, opens FIRST per Q-W1-CROSS-2-a)

**Scope**: cascade #7 detail PLAN derived from `W1-S6-RCA.md` (post-PR #67 `7dcedd00`); incorporates Captain G33 v5 #3 + #5 + #6 ratifications + Wave A.2.1 substrate-flaw closures relevant to W1-S6 publisher boundary.

**Authored**: 2026-05-09 ~20:32 IST · **Authored-by**: james (Claude Opus 4.7 1M)
**Class**: H1 PLAN file derived per RCA; foundational-boundary class (auth)
**Status**: SHIPPED — Captain Option C hybrid (2026-05-09 ~20:28 IST) authorized cascade #7 authoring; per-PR Captain merge auth required at PR-open commit time per G33 v5 #9
**Sequencing**: PR-A FIRST (this PLAN) → PR-C W1-S5 (depends on PR-A merge) → PR-D W3 (independent of PR-A/C)

**Authoritative substrate**:
- `W1-S6-RCA.md` racecontrol `7dcedd00` — boundary map + inherited issues + V2-alignment delta + 5-section RCA
- `MMA-STEP-2-W1S5-W1S6-W3-PLAN-A2.md` Wave A.2 (META-PLAN; PR breakdown PR-A..PR-E)
- `MMA-STEP-2-W1S5-W1S6-W3-PLAN-A2-1.md` Wave A.2.1 surgical amendment (substrate-flaw closure)
- `MMA-STEP-1-W1S5-W1S6-W3-CONSENSUS.md` Step 1 canonical (F-CONS-15..18 + F-AMEND-CONS-4 + F-AMEND-CONS-8)
- `MMA-STEP-1-AMENDED-W1S5-W1S6-CONSENSUS.md` Step 1 amended (4/4 strong consensus)
- `MMA-STEP-4-VERIFY-W1S5-W1S6-W3-CONSENSUS.md` Step 4 VERIFY (FAIL → Option C hybrid disposition)

**V2 doctrine alignment**: §AMEND-3.II D12 Foundation/Strategy/Config separation · §S-158 V2 Audit-Log Doctrine `audit_log.action_type` CHECK constraint · NEW-Q-1 F-05 anti-pattern lint codification · §S-146 V1↔V2 RCA rule SECOND end-to-end pipeline application Step 3 EXECUTE input.

---

## §1 — Goal

Implement PIN-LOCKOUT auto-rotate on N-failed-attempts threshold with email/WhatsApp dispatch, retry queue per G33 v5 #6, and lockout-state predicate publisher consumed by W1-S5 refresh path per G33 v5 #2 cross-coupling.

**V2 customer impact:** staff (cashier / manager / superadmin) attempting login with wrong PIN N times triggers auto-lockout; PIN auto-rotated; helpdesk@ + Captain notified; lockout-state cross-coupled to refresh path so a held JWT cannot survive a lockout event.

**V1 inheritance:** existing `staff.lockout_count` column + `staff.lockout_until` semantics — V2 retains schema, replaces logic per RCA.

---

## §2 — Pre-PR-A operational checks (ACTION-PRE-W1-S6 from Wave A.2 §13)

Before PR-A code authoring, verify SMTP / DKIM / SPF infrastructure for racingpoint.in mail-from headers:

1. **SMTP probe:** `dig +short MX racingpoint.in` returns valid MX records
2. **DKIM probe:** `dig +short TXT default._domainkey.racingpoint.in` returns DKIM-Signature record OR document absence
3. **SPF probe:** `dig +short TXT racingpoint.in` returns SPF record with `include:_spf.google.com` or equivalent
4. **helpdesk@ mailbox provisioning:** Captain Q-DECISION pending (Wave A.2 §1 noted as Captain-reserve); **DEFAULT this PLAN: helpdesk@ goes to bono@racingpoint.in temporarily until provisioning** — explicit comment in code at dispatch site
5. **Result documented in:** `racecontrol/.planning/specs/v2/W1-S6-PRE-FLIGHT.md` (separate ship; not gating PR-A author but gating PR-A merge)

---

## §3 — Scope (this PLAN's PR-A deliverable)

### §3.1 — File targets

Primary touch (V2 boundary):
- `crates/racecontrol-crate/src/auth/middleware.rs` — PIN-LOCKOUT logic + V1 boundary
- `crates/racecontrol-crate/src/auth/lockout.rs` — **NEW** — `LockoutManager` + lockout-state predicate publisher + retry queue
- `crates/racecontrol-crate/src/auth/dispatch.rs` — **NEW** — email + WhatsApp dispatch with timeout (F-AMEND-CONS-4)
- `crates/racecontrol-crate/src/auth/audit.rs` — **NEW** — audit-log row writer using `log_admin_action` per V2 Audit-Log Doctrine NEW-Q-2

Test targets:
- `crates/racecontrol-crate/tests/auth/lockout_test.rs` — **NEW**
- `crates/racecontrol-crate/tests/auth/dispatch_test.rs` — **NEW**
- `crates/racecontrol-crate/tests/auth/lockout_publisher_test.rs` — **NEW** (F-CONS-15 cross-coupling publisher contract)

DB schema (sqlx-migrate):
- `crates/v2-db/migrations/20260510000001_pin_lockout_state_v2.sql` — `pin_rotations` table (audit trail) + `lockout_alerts` queue table (in-memory primary; DB persistence forward-compat hook only)

Config:
- `crates/racecontrol-crate/src/config.rs` — add `auth.pin_lockout_threshold` (default 5), `auth.pin_lockout_window_secs` (default 300), `auth.dispatch_timeout_secs` (default 10), `auth.dispatch_retry_max_attempts` (default 3), `auth.dispatch_retry_backoff_secs` (default `[10, 60, 300]`), `auth.helpdesk_email` (default `bono@racingpoint.in`)

### §3.2 — Out of scope (deferred to W1-S5-PLAN.md)

- W1-S5 sliding-window refresh path (consumes lockout-state predicate; deps on PR-A merge)
- Q-W1-S5-NEW-1 12h max-session-life cap (W1-S5 path)
- Q-S5-NEW-2 force-expire JWT on lockout-active (W1-S5 path)
- F-CONS-15 atomic TOCTOU consumer side (W1-S5 path)
- F-CONS-16 single-flight refresh mutex (W1-S5 path)
- F-CONS-17 clock skew tolerance (W1-S5 token validation)

### §3.3 — Out of scope (deferred to W3-PLAN.md)

- Wallet HOLD-RELEASE-CAPTURE 2-phase commit
- W3 PR-D entirely

---

## §4 — Q-DECISION incorporation specs

### §4.1 — Q-W1-CROSS-1 (security-class explicit) — publisher-side contract

**Spec:**
- `LockoutManager::is_locked_out(staff_id) -> Result<LockoutPredicate, LockoutError>`
- `LockoutPredicate::Active { until_iso8601, reason }` | `LockoutPredicate::Inactive`
- W1-S5 refresh path consumes via shared `Arc<LockoutManager>` injected at racecontrol startup
- Code-comment at predicate site: `// Q-W1-CROSS-1: SECURITY-CLASS EXPLICIT — refresh path MUST consult this predicate; bypass is gate violation`
- Test `lockout_predicate_returns_active_when_locked_out` + `lockout_predicate_returns_inactive_when_not_locked_out`

### §4.2 — Q-W1-CROSS-2 (default-a sequencing) — PR-A FIRST

**Spec:**
- This W1-S6-PLAN.md = PR-A authoring substrate
- PR-A merge gates W1-S5-PLAN.md PR-C authoring start
- PR-A scope strict: lockout publisher + dispatch + retry queue + audit log + DB schema; NO refresh-path consumer code (that's PR-C)
- Forward-compat hook: `LockoutManager::subscribe(consumer_fn)` API present in PR-A; first consumer (W1-S5) wired in PR-C

### §4.3 — Q-W1-S6-NEW-2 (retry queue default-Y caveats)

**Spec (Captain G33 v5 #6 verbatim):** YES with caveats — exponential backoff 10s · 60s · 300s · 3 attempts · email-only (WhatsApp Captain-freeze stays fire-and-forget) · in-memory queue (kaizen-min) · restart-loses-queue acceptable per CR-3 customer-service-priority bounded blast radius.

**Implementation:**
- `tokio::sync::mpsc::UnboundedSender<DispatchTask>` channel; consumer is a single tokio-spawned task running `dispatch_retry_loop`
- `DispatchTask { recipient, subject, body, attempt: u8, retry_after: Instant }`
- Backoff schedule: attempt 1 immediate, attempt 2 after +10s, attempt 3 after +60s, attempt 4 (final) after +300s; cap at 3 attempts (= attempt 1 + 2 retries) per Captain spec — NOTE: Captain spec says "3 attempts" total; backoff sequence of `10s · 60s · 300s` applies to attempts 2 / 3 / [4 if extended]; for V2.0 this PLAN interprets as **3 total attempts** with backoffs `[0s, 10s, 60s]` applied between attempts; the `300s` slot in Captain spec is RESERVED for V2.1+ extension to 4 attempts (out of scope V2.0)
- Anti-precedent comment: `// Q-W1-S6-NEW-2: WhatsApp dispatch is FIRE-AND-FORGET (Captain-freeze); only email goes through retry queue`
- Restart loses queue — comment: `// Q-W1-S6-NEW-2 kaizen-min: in-memory queue; restart-loses-queue is acceptable per CR-3 bounded blast radius; DB persistence hook reserved for V2.1+`
- Test `retry_queue_first_attempt_succeeds` + `retry_queue_retries_on_timeout` + `retry_queue_gives_up_after_3_attempts` + `retry_queue_email_only_not_whatsapp` + `retry_queue_drains_on_shutdown_with_log` + `retry_queue_metrics_emit` (`dispatch_duration_seconds` histogram + `dispatch_outcome_total{outcome=ok|timeout|error}` counter per F-AMEND-CONS-4)

### §4.4 — F-CONS-18 dispatch timeout (Step 1 amended CONSENSUS)

**Spec:**
- `tokio::time::timeout(config.auth.dispatch_timeout_secs, async { send_email(...).await })` wraps each attempt
- On timeout: `dispatch_outcome=timeout` increment + retry per §4.3
- Test `dispatch_timeout_caught_and_retried` + `dispatch_timeout_metric_increments`

### §4.5 — Novel P1 retry-queue email-only safety

**Spec (per §S-160 mimo F-AMEND-SING-7):**
- Type-system enforcement: `DispatchTask::Email { ... }` ONLY enters retry queue; `DispatchTask::WhatsApp { ... }` is FIRE-AND-FORGET via direct `send_whatsapp_fire_and_forget` (not the retry-queue channel)
- Compile-time enum prevents accidentally retrying WhatsApp dispatch
- Test `whatsapp_dispatch_does_not_enter_retry_queue` (explicit type-system test)

### §4.6 — F-AMEND-CONS-8 anti-precedent comment

**Spec:**
- Per-staff-id rate-limit primitive used in retry queue: anti-precedent comment at site `// F-AMEND-CONS-8 ANTI-PRECEDENT: this rate-limit primitive is RETRY-QUEUE-SCOPE only; do NOT extend to general request rate-limiting without re-evaluating overscope risk`

### §4.7 — F-AMEND-SING-10 documentation-add

**Spec:**
- Code comment at dispatch timeout site: `// F-CONS-18 RCA: timeout protects against blocking SMTP server; retry queue per Q-W1-S6-NEW-2; anti-precedent F-AMEND-CONS-8 (rate-limit primitive scope)`

### §4.8 — NEW-Q-2 V2 Audit-Log Doctrine compliance

**Spec:**
- All lockout events logged via `log_admin_action(action_type, staff_id, payload)` — `action_type` is enum-bounded by DB CHECK constraint per §S-158
- Action types this PR-A introduces: `lockout_threshold_breach`, `lockout_pin_rotated`, `lockout_alert_dispatched_email`, `lockout_alert_dispatched_whatsapp`, `lockout_alert_dispatch_failed`
- DB migration `20260510000001_pin_lockout_state_v2.sql` extends `audit_log.action_type` CHECK constraint to include these 5 new values

---

## §5 — Test coverage requirements

### §5.1 — Unit tests (must exist + pass)

- `lockout_predicate_returns_active_when_locked_out`
- `lockout_predicate_returns_inactive_when_not_locked_out`
- `lockout_threshold_breach_locks_staff` (5 failed attempts in 300s → locked)
- `lockout_threshold_not_breached_within_window` (4 failed attempts then 1 success → not locked)
- `lockout_window_expiry_resets_count` (attempts outside window don't accumulate)
- `pin_auto_rotated_on_lockout` (new PIN generated, audit-logged)
- `lockout_state_persisted_to_db` (survives racecontrol restart)
- `cashier_role_lockout_separate_from_manager_lockout` (per-role boundary respected)

### §5.2 — Dispatch tests (must exist + pass)

- All §4.3 + §4.4 + §4.5 tests above

### §5.3 — Cross-coupling publisher contract test (W1-S5 deps prep)

- `lockout_publisher_emits_subscribe_event_on_lock` — W1-S5 PR-C will subscribe to this; PR-A test mocks consumer
- `lockout_publisher_emits_unsubscribe_event_on_unlock`

### §5.4 — Audit-log doctrine compliance test

- `audit_log_action_type_check_constraint_includes_new_lockout_types` (DB-level CHECK constraint test post-migration)

### §5.5 — F-05 anti-pattern regression test (extended scope per Wave A.2.1 §6)

- `f05_anti_pattern_w1_s6_lockout_count_path` — UPDATE `staff.lockout_count`; assert no UPDATE-then-SELECT-same-column anti-pattern

### §5.6 — Integration tests (must exist + pass)

- `e2e_lockout_5_failures_locks_staff_dispatches_email_audits`
- `e2e_lockout_dispatch_retry_succeeds_on_attempt_2`
- `e2e_lockout_dispatch_gives_up_after_3_attempts_logs_metric`

---

## §6 — Risk + rollback

### §6.1 — Risks

| Risk | Class | Mitigation |
|------|-------|-----------|
| SMTP server outage during lockout event | P1 ops | retry queue + WhatsApp fire-and-forget Captain-freeze fallback |
| `audit_log.action_type` CHECK constraint blocks logging if migration not run | P0 deploy | DB migration MUST run before binary deploy; migration smoke-test in `cargo test` |
| Lockout-state predicate consumer (W1-S5) not yet wired (PR-C deferred) | P2 forward | `LockoutManager::subscribe()` API placeholder; first consumer is no-op until PR-C lands |
| In-memory retry queue lost on restart | P2 acceptable | Captain Q-W1-S6-NEW-2 default accepts; CR-3 bounded blast radius |
| Per-staff-id rate-limit primitive overscope | P2 anti-precedent | F-AMEND-CONS-8 comment + lint forward-trigger |
| Email-only enforcement bypassed by future code | P1 type-system | `DispatchTask::WhatsApp` cannot enter retry queue (compile-time) |

### §6.2 — Rollback path

1. Revert PR-A merge (clean revert; no DB rows yet — migration adds columns/CHECK only)
2. DB rollback: `DROP TABLE pin_rotations; DROP TABLE lockout_alerts; ALTER TABLE audit_log DROP CONSTRAINT audit_log_action_type_check; ALTER TABLE audit_log ADD CONSTRAINT audit_log_action_type_check CHECK (action_type IN (<original-list>))`
3. Rollback migration file: `crates/v2-db/migrations/20260510000001_pin_lockout_state_v2.down.sql`
4. Pre-cutover smoke test: rollback migration + restart racecontrol + verify PIN-LOCKOUT V1 path remains functional (V1 code path is left UNTOUCHED in PR-A — `auth/middleware.rs` V1 lockout retained until PR-C cutover)

### §6.3 — Cross-PR rollback (per Step 4 VERIFY F-CONS-rollback-procedure missing-risk)

- PR-A rollback does NOT block PR-C / PR-D (which haven't merged yet)
- Captain may keep PR-A merged + roll W1-S5 + W3 separately if needed
- Any PR-A rollback MUST trigger comms-link `1e80c35e`-style §S-N entry documenting rollback rationale + bilateral notify

---

## §7 — Deploy section (per Deploy Manifest Protocol)

```yaml
deploy:
  rust_binary: racecontrol (Server .23 + Bono VPS cloud)
  frontend_rebuild: none (no UI surface this PR-A)
  config_change: racecontrol.toml — add `auth.pin_lockout_*` keys + `auth.dispatch_*` keys + `auth.helpdesk_email`
  db_migration: 20260510000001_pin_lockout_state_v2.sql (sqlx-migrate)
  infrastructure: SMTP / DKIM / SPF probe results from §2 must be ✓ before PR-A merge
  data_files: none
  bat_file: none
  cloud_parity: REQUIRED — racecontrol binary deploys to BOTH Server .23 + Bono VPS; DB migration runs on cloud as well (cloud has separate DB; both must migrate)
  targets: [Server .23, Bono VPS] (NOT Pods 1-8 — auth lives on racecontrol only)
  pre_deploy_smoke: SMTP probe + DKIM probe + SPF probe + DB migration smoke test in `cargo test`
  post_deploy_verify:
    - curl racecontrol /api/v1/health -> build_id matches
    - PIN-LOCKOUT V1 path still functional (V2 hot path not yet wired — PR-A is publisher only)
    - `audit_log` rows for new action types appear on test lockout event
    - Retry queue metrics `dispatch_duration_seconds` + `dispatch_outcome_total` emit
```

---

## §8 — Q-DECISION compliance map (G33 v5 ratifications applied this PLAN)

| Ratification | This W1-S6-PLAN.md status |
|--------------|--------------------------|
| Q-W1-CROSS-1 | §4.1 — publisher contract spec |
| Q-W1-CROSS-2 | §4.2 — PR-A FIRST sequencing locked |
| Q-W1-S5-NEW-1 | NOT APPLICABLE (W1-S5 detail PLAN; this is W1-S6) |
| Q-S5-NEW-2 | NOT APPLICABLE (W1-S5 detail PLAN) |
| Q-W1-S6-NEW-2 | §4.3 — retry queue spec verbatim |
| NEW-Q-1 | §5.5 — F-05 anti-pattern regression test for W1-S6 lockout_count path |
| NEW-Q-2 | §4.8 — V2 Audit-Log Doctrine `audit_log.action_type` CHECK constraint extension |

---

## §9 — Cascade transition

| Cascade item | Pre-this-PLAN | Post-this-PLAN |
|--------------|---------------|----------------|
| #7 W1-S6-PLAN.md authoring | gates on Captain G33 v6 Option C | **SHIPPED** |
| #7 W1-S5-PLAN.md authoring | gates on Captain G33 v6 Option C | concurrent (this turn) |
| #7 W3-PLAN.md authoring | gates on Captain G33 v6 Option C | concurrent (this turn) |
| #8 PR-A opens W1-S6 FIRST | gates on #7 | **NOW UNBLOCKED — PR-A authoring next; PR-open auth required at commit time per G33 v5 #9** |

---

## §10 — NOT TESTED

- §2 SMTP / DKIM / SPF probes — DEFERRED to PR-A pre-merge (probe results not in this PLAN)
- helpdesk@ mailbox provisioning — Captain Q-DECISION pending; default to `bono@racingpoint.in` until provisioned
- F-05 lint feasibility — see Wave A.2.1 §6 (custom clippy lint OR `cargo-careful`; deferred to V2.1 if not feasible at PR-A time)
- Cloud parity DB migration runtime — racecontrol cloud DB schema state at PR-A merge time NOT YET surveyed (gates on PR-A pre-merge cloud-DB migration smoke test)
- Bono VPS cloud-side `pm2 racecontrol` restart sequencing — to be specified in PR-A deploy script
- Customer-service-priority CR-3 escalation runbook (when retry queue gives up after 3 attempts) — separate runbook deferred to ops phase
- Per-PR Captain merge auth — required at PR-open commit time per G33 v5 #9
- bono AMPLIFIER absorption — deferred to next bilateral cycle per W1-S5 RCA precedent

---

— james / 2026-05-09 ~20:32 IST · W1-S6-PLAN.md SHIPPED · cascade #7 detail PLAN derived from W1-S6-RCA.md `7dcedd00` · PR-A FIRST per Q-W1-CROSS-2-a · Q-W1-S6-NEW-2 retry queue spec verbatim · Captain Option C hybrid authorization · per-PR Captain merge auth required at PR-open per G33 v5 #9 · 0 G9 self-caught
