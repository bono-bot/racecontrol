# W3-PLAN — Wallet HOLD-RELEASE-CAPTURE 2-phase commit (PR-D, Wave 1, INDEPENDENT)

**Scope**: cascade #7 detail PLAN derived from `W3-WALLET-HRC-RCA.md` (`78f82654`); incorporates Wave A.2 §F-CONS-2 NEW-Q-1 + Wave A.2.1 §6 F-05 anti-pattern regression test scope extension + V2 Audit-Log Doctrine NEW-Q-2.

**Authored**: 2026-05-09 ~20:36 IST · **Authored-by**: james (Claude Opus 4.7 1M)
**Class**: H1 PLAN file derived per RCA; foundational-boundary class (wallet)
**Status**: SHIPPED — Captain Option C hybrid (2026-05-09 ~20:28 IST) authorized cascade #7 authoring; PR-D opens INDEPENDENT of PR-A / PR-C sequencing per Q-W1-CROSS-2 default-a (W3 path orthogonal); per-PR Captain merge auth required at PR-open per G33 v5 #9
**Sequencing**: INDEPENDENT — may merge before/after PR-A/PR-C; bono-LEAD per Drift-Pilot first-mover doctrine; james-side this PLAN derives the racecontrol-crate side; bono ships v2-db Wallet schema in parallel per PACT-024

**Authoritative substrate**:
- `W3-WALLET-HRC-RCA.md` racecontrol `78f82654` — 12-row boundary map V1+V2 + 15-issue inherited catalogue + 13-sub-step proposal in 4 PRs
- `MMA-STEP-2-W1S5-W1S6-W3-PLAN-A2.md` Wave A.2 + `MMA-STEP-2-W1S5-W1S6-W3-PLAN-A2-1.md` Wave A.2.1
- `MMA-STEP-1-W1S5-W1S6-W3-CONSENSUS.md` Step 1 canonical (W3 findings unchanged by Step 1 amendment + Step 4 VERIFY)
- `PACT-20260504-024-wallet-concurrency-idempotency-extension.md` (bono-LEAD; james AMPLIFIER vote outstanding) — composes-with this PLAN

**V2 doctrine alignment**: §AMEND-3.II D12 Foundation/Strategy/Config separation · §S-158 V2 Audit-Log Doctrine · NEW-Q-1 F-05 anti-pattern lint codification · §S-146 V1↔V2 RCA rule SECOND end-to-end pipeline application Step 3 EXECUTE input · Wallet Framing C source-tag preservation (Single-Purpose Voucher) · §S-146 V1-dependent V2 sections RCA discipline (foundational wallet boundary).

---

## §1 — Goal

Implement V2 wallet HOLD-RELEASE-CAPTURE 2-phase commit pattern for sim/PS5 redemption flows (per Wallet Framing C 2026-05-03 LOCKED) AND:
1. F-CONS-5 wallet-side HashMap pruning (V1 inheritance for any in-memory wallet caches)
2. F-05 anti-pattern regression test for wallet capture path (UPDATE-then-SELECT same column avoidance)
3. V2 Audit-Log Doctrine `action_type` CHECK constraint extension for wallet HRC events
4. Cross-pilot wrapper compatibility with bono-side `shared/wallet-client.js` Idempotency-Key wrappers (PACT-024 §A scope)
5. Single-Purpose Voucher contract: 18% GST at top-up; sim+PS5 only redeemable; cafe always separate (per Wallet Framing C)

**V2 customer impact:** customer's wallet credit reservation is atomic across the session-billing flow; HOLD reserves balance at session-start, RELEASE returns balance on cancel, CAPTURE finalizes balance on session-end. Eliminates V1 race conditions (F-05 anti-pattern losses; F-CONS-5 unbounded growth).

**V1 inheritance:** existing `wallet.rs` / `wallet_refund.rs` / `billing_session_end.rs` driver_id-scoped paths (per W3-RCA §1 boundary map); V2 introduces customer_id-scoped `WalletService` in `crates/v2-db/src/wallets.rs` (W1-S2 already shipped on feat branch; this PLAN extends with HRC).

---

## §2 — Pre-PR-D operational checks

1. **PACT-024 substrate-pointer:** confirm bono-side state at FILED-AWAITS-AMPLIFIER OR ratified; james AMPLIFIER vote on Q1-Q5 outstanding per V2-MASTER-STATE history
2. **W1-S2 WalletService API surface stable:** `WalletService::reserve()` / `release()` / `capture()` callable per W1-S2 base implementation
3. **Q-W3-RECONCILE-1:** confirm canonical V2 wallet path location — `crates/v2-db/src/wallets.rs` (NOT `crates/racecontrol/src/wallet.rs` V1 path — V1 retained for deprecation but not extended)
4. **bono-LEAD coordination:** james-side this PLAN derives racecontrol-crate-side; bono ships v2-db schema + `shared/wallet-client.js` per PACT-024 §A; cross-pilot AMPLIFIER vote per Q1-Q5 disposition

---

## §3 — Scope (this PLAN's PR-D deliverable — racecontrol-crate side)

### §3.1 — File targets (james-side racecontrol-crate)

Primary touch (V2 boundary):
- `crates/racecontrol-crate/src/wallet/v2_bridge.rs` — **NEW** — `V2WalletBridge` calls into `crates/v2-db/src/wallets.rs::WalletService` (replaces V1 `wallet.rs` direct calls in V2-active code paths)
- `crates/racecontrol-crate/src/wallet/hrc.rs` — **NEW** — HOLD-RELEASE-CAPTURE state machine
- `crates/racecontrol-crate/src/wallet/audit.rs` — **NEW** — wallet audit-log writer per §S-158 V2 Audit-Log Doctrine
- `crates/racecontrol-crate/src/billing/billing_session_v2.rs` — **MODIFIED** — sim/PS5 session start invokes `wallet/hrc.rs::hold()`; session end invokes `capture()`; cancel invokes `release()`

Test targets:
- `crates/racecontrol-crate/tests/wallet/hrc_test.rs` — **NEW**
- `crates/racecontrol-crate/tests/wallet/v2_bridge_test.rs` — **NEW**
- `crates/racecontrol-crate/tests/wallet/audit_test.rs` — **NEW**
- `crates/racecontrol-crate/tests/wallet/f05_anti_pattern_w3_test.rs` — **NEW** (Wave A.2.1 §6 + NEW-Q-1)

DB schema (bono-LEAD; this PLAN documents but does NOT author):
- `crates/v2-db/migrations/<bono-authored>_wallet_hrc_v2.sql` — `wallet_holds` table + `wallet_redemptions` extensions per W3-RCA §5 PR-A scope (bono ships)
- `crates/v2-db/migrations/<bono-authored>_audit_log_wallet_action_types.sql` — extends `audit_log.action_type` CHECK constraint with wallet action types

Cross-pilot wrapper (bono-LEAD per PACT-024):
- `comms-link/shared/wallet-client.js` — **NEW (bono-side)** — Idempotency-Key wrapper for HRC operations (this PLAN documents james-side consumption contract; bono ships actual wrapper)

Config:
- `crates/racecontrol-crate/src/config.rs` — add `wallet.hrc_hold_ttl_secs` (default 3600 / 1h — hold expires + auto-release if not captured), `wallet.use_v2_bridge` (default `true`; rollback flag)

### §3.2 — Out of scope (deferred or cross-pilot)

- DB schema authoring (bono-LEAD)
- `shared/wallet-client.js` JS wrapper authoring (bono-LEAD)
- Cafe wallet flows (Wallet Framing C: cafe always separate; not in V2.0 scope)
- PS5 manual time-tracking integration (V2.1+ deferred)
- W1-S5 / W1-S6 paths (separate PLANs)

---

## §4 — HRC state machine spec

### §4.1 — States

```
                  ┌──────────┐
                  │   IDLE   │
                  └────┬─────┘
                       │ hold(amount)
                       ↓
                  ┌──────────┐
       ┌──────────┤  HELD    ├──────────┐
       │          └────┬─────┘          │
       │ release()     │ capture()      │ ttl_expiry
       │               │                │
       ↓               ↓                ↓
  ┌─────────┐    ┌──────────┐    ┌─────────────┐
  │RELEASED │    │ CAPTURED │    │AUTO_RELEASED│
  └─────────┘    └──────────┘    └─────────────┘
```

### §4.2 — Invariants

1. **At-most-once capture:** A HELD hold can transition to CAPTURED OR RELEASED OR AUTO_RELEASED — never multiple terminal states
2. **Idempotent release:** `release(hold_id)` on already-RELEASED OR AUTO_RELEASED → no-op; on CAPTURED → error
3. **Idempotent capture:** `capture(hold_id)` on already-CAPTURED → no-op; on RELEASED OR AUTO_RELEASED → error
4. **TTL auto-release:** background task `wallet_hold_ttl_task` runs every 60s; releases held holds where `now > held_at + hrc_hold_ttl_secs`
5. **F-05 anti-pattern guard:** capture path MUST NOT UPDATE-then-SELECT-same-column in same scope (per W3-RCA inherited issue F-05)

### §4.3 — API surface (Rust)

```rust
pub struct V2WalletBridge {
    inner: Arc<v2_db::wallets::WalletService>,
    idempotency_keys: Arc<RwLock<HashSet<String>>>,
}

impl V2WalletBridge {
    pub async fn hold(&self, customer_id: u64, amount_paise: i64, idem_key: String)
        -> Result<HoldId, WalletError>;
    pub async fn release(&self, hold_id: HoldId, idem_key: String)
        -> Result<(), WalletError>;
    pub async fn capture(&self, hold_id: HoldId, idem_key: String)
        -> Result<(), WalletError>;
}
```

### §4.4 — Idempotency contract (composes-with PACT-024 bono-LEAD)

- All three operations accept `idem_key: String` (UUID v4 or session-derived deterministic key)
- Duplicate operation with same `idem_key` → idempotent (no-op for terminal-state operations; cached-result for in-flight operations)
- Cross-pilot: `comms-link/shared/wallet-client.js` (bono-side) wraps these calls with Idempotency-Key HTTP header per PACT-024 §A

### §4.5 — F-05 anti-pattern regression test (Wave A.2.1 §6 + NEW-Q-1)

- `f05_anti_pattern_w3_capture_path` test — capture path MUST snapshot `wallet_redemptions.amount_paise` BEFORE UPDATE; SELECT after UPDATE in same scope = anti-pattern
- Code-comment at capture path UPDATE site: `// F-05 OK: explicit snapshot-read-before-write; anti-pattern guard via f05_anti_pattern_w3_capture_path test`

---

## §5 — V2 Audit-Log Doctrine compliance (NEW-Q-2)

### §5.1 — Wallet action types added to `audit_log.action_type` CHECK constraint

- `wallet_hrc_hold_created`
- `wallet_hrc_held_to_captured`
- `wallet_hrc_held_to_released`
- `wallet_hrc_held_to_auto_released`
- `wallet_hrc_idempotent_replay`
- `wallet_hrc_invariant_violation`

### §5.2 — Test (DB-level)

- `audit_log_action_type_check_constraint_includes_wallet_hrc_types`

---

## §6 — Test coverage matrix

### §6.1 — State machine tests

- `hrc_idle_to_held` (single hold, balance reserved correctly)
- `hrc_held_to_captured` (capture removes from balance)
- `hrc_held_to_released` (release restores to balance)
- `hrc_held_to_auto_released` (TTL expiry triggers release)
- `hrc_double_capture_idempotent`
- `hrc_double_release_idempotent`
- `hrc_capture_after_release_errors`
- `hrc_release_after_capture_errors`

### §6.2 — Idempotency tests

- `hrc_hold_with_same_idem_key_returns_same_hold_id`
- `hrc_capture_with_same_idem_key_idempotent`
- `hrc_release_with_same_idem_key_idempotent`

### §6.3 — F-05 anti-pattern regression test

- `f05_anti_pattern_w3_capture_path`

### §6.4 — V2 Audit-Log Doctrine compliance test

- `audit_log_action_type_check_constraint_includes_wallet_hrc_types`

### §6.5 — Cross-pilot wrapper integration (gates on bono-side ship)

- `e2e_hrc_via_wallet_client_js_wrapper` — calls into `comms-link/shared/wallet-client.js` (bono-LEAD; integration test gates on bono ship)

### §6.6 — Wallet Framing C compliance tests

- `hrc_hold_only_for_sim_or_ps5_session` (cafe orders MUST NOT trigger HRC; cafe-flow uses separate path)
- `hrc_18pct_gst_applied_at_topup_not_at_redemption` (Wallet Framing C: GST at top-up only)

### §6.7 — Integration tests

- `e2e_session_start_holds_release_on_cancel`
- `e2e_session_start_holds_capture_on_normal_end`
- `e2e_session_start_holds_auto_release_on_ttl_expiry`

---

## §7 — Risk + rollback

### §7.1 — Risks

| Risk | Class | Mitigation |
|------|-------|-----------|
| F-05 anti-pattern reintroduction in capture path | **P1 closed** | F-05 regression test per §4.5 |
| Double-capture / double-release race | **P0 closed** | Idempotency contract per §4.4 |
| TTL auto-release fires on a captured hold (race) | **P1 closed** | At-most-once invariant per §4.2; capture takes precedence over TTL via state-machine atomicity |
| F-CONS-5 wallet-cache HashMap unbounded growth | **P2 closed** | Wave A.2.1 §5 pruning pattern applied; idempotency-key cache TTL'd |
| Cafe redemption accidentally triggers HRC | P1 spec | Wallet Framing C compliance test per §6.6 |
| Cross-pilot wrapper not yet shipped (bono-LEAD) | P2 sequencing | V2 bridge gracefully degrades to direct WalletService calls if `shared/wallet-client.js` not yet present |
| 18% GST applied at wrong stage | P1 spec | Wallet Framing C compliance test per §6.6 |
| PACT-024 §A racecontrol path stale (Q-W3-RECONCILE-1) | P1 ack | §2 pre-merge check confirms canonical V2 path is `crates/v2-db/src/wallets.rs`; this PLAN updates path |

### §7.2 — Rollback path

1. Feature-flag: `wallet.use_v2_bridge` env-var (default `true` post-deploy; `false` reverts to V1 wallet.rs path)
2. V1 paths retained UNTOUCHED until V2 in production for >7 days with zero P0/P1 wallet incidents
3. Revert PR-D merge: clean revert; rollback migration from bono-side
4. Cross-PR: PR-D rollback does NOT affect PR-A (W1-S6) or PR-C (W1-S5)

### §7.3 — Cross-pilot rollback

- bono-side `shared/wallet-client.js` rollback would require coordination via comms-link relay
- PACT-024 amendment ratification path documented in PACT-024 §6 if needed

---

## §8 — Deploy section

```yaml
deploy:
  rust_binary: racecontrol (Server .23 + Bono VPS cloud)
  frontend_rebuild: none (PWA wallet UI deferred to V2.1+; this PR-D is backend HRC only)
  config_change: racecontrol.toml — add wallet.hrc_hold_ttl_secs, wallet.use_v2_bridge
  db_migration: bono-authored migrations for wallet_holds + audit_log.action_type extension (cross-pilot dependency)
  infrastructure: bono-side comms-link/shared/wallet-client.js wrapper (cross-pilot dependency)
  data_files: none
  bat_file: none
  cloud_parity: REQUIRED — racecontrol binary deploys to BOTH Server .23 + Bono VPS; v2-db migration runs on cloud as well
  targets: [Server .23, Bono VPS, Comms-link Bono] (Comms-link Bono only for shared/wallet-client.js)
  pre_deploy_smoke: bono-side migration smoke + js wrapper integration test + Idempotency-Key contract test
  post_deploy_verify:
    - racecontrol /api/v1/health build_id matches
    - existing V1 wallet refund/topup paths still functional (V1 retained, V2 added)
    - HRC hold + capture cycle on test customer succeeds
    - HRC TTL auto-release fires after 1h on test stale hold
    - F-05 regression test green in CI
    - audit_log rows for wallet HRC action types written
```

---

## §9 — Q-DECISION compliance map

| Ratification / substrate | This W3-PLAN.md status |
|--------------------------|------------------------|
| Q-W1-CROSS-1 | NOT APPLICABLE (auth boundary; W3 is wallet) |
| Q-W1-CROSS-2 (sequencing) | INDEPENDENT PR-D path; Q-W1-CROSS-2-a default-a does not constrain W3 |
| Q-W1-S5-NEW-1 | NOT APPLICABLE (auth) |
| Q-S5-NEW-2 | NOT APPLICABLE (auth) |
| Q-W1-S6-NEW-2 | NOT APPLICABLE (auth) |
| NEW-Q-1 | §4.5 + §6.3 — F-05 regression test for W3 capture path |
| NEW-Q-2 | §5 — V2 Audit-Log Doctrine `action_type` CHECK constraint extension for 6 wallet HRC action types |
| Wallet Framing C | §1 + §6.6 — sim/PS5 only redeemable; cafe separate; 18% GST at top-up |
| PACT-024 (bono-LEAD) | §3.2 + §4.4 + §7.3 — cross-pilot wrapper compatibility documented; AMPLIFIER vote on Q1-Q5 outstanding per V2-MASTER-STATE history |
| Q-W3-RECONCILE-1 | §2 + §7.1 — canonical V2 wallet path = `crates/v2-db/src/wallets.rs` |

---

## §10 — Cascade transition

| Cascade item | Pre-this-PLAN | Post-this-PLAN |
|--------------|---------------|----------------|
| #7 W3-PLAN.md authoring | gates on Captain G33 v6 Option C | **SHIPPED** (this turn) |
| #8 PR-D opens (INDEPENDENT) | gates on #7 + bono-LEAD coordination | **NOW UNBLOCKED — PR-D coordination next; per-PR Captain merge auth required at PR-open per G33 v5 #9** |
| PACT-024 AMPLIFIER vote | bono-LEAD pending; james AMPLIFIER on Q1-Q5 outstanding | unchanged (separate path) |

---

## §11 — NOT TESTED

- bono-side DB migration scope — gates on bono ship per PACT-024 §A
- bono-side `shared/wallet-client.js` wrapper — gates on bono ship per PACT-024 §A
- james AMPLIFIER vote on PACT-024 Q1-Q5 — outstanding (separate channel)
- Cafe wallet flow integration — explicitly out of scope (Wallet Framing C: cafe always separate)
- PS5 manual time-tracking integration — V2.1+ deferred
- F-05 lint feasibility study — see Wave A.2.1 §6
- Per-PR Captain merge auth — required at PR-open per G33 v5 #9
- bono AMPLIFIER absorption of this W3-PLAN — deferred to next bilateral cycle
- Customer-facing balance display UI — V2.1+ PWA scope
- Idempotency-Key cache TTL value — needs Captain Q-DECISION OR default 1h aligned with `hrc_hold_ttl_secs`

---

— james / 2026-05-09 ~20:36 IST · W3-PLAN.md SHIPPED · cascade #7 detail PLAN derived from W3-WALLET-HRC-RCA.md `78f82654` · INDEPENDENT PR-D path (orthogonal to PR-A/PR-C sequencing) · 2 Captain G33 v5 ratifications applied (NEW-Q-1 + NEW-Q-2) + Wallet Framing C compliance + PACT-024 cross-pilot composes-with · bono-LEAD coordination required for DB migration + JS wrapper · Captain Option C hybrid authorization · per-PR Captain merge auth required at PR-open per G33 v5 #9 · 0 G9 self-caught
