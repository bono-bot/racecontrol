---
phase: 414
slug: continuous-billing-session
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-04-18
updated: 2026-04-18 (revision iter 1 — B4 fix)
---

# Phase 414 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.
> Source: 414-RESEARCH.md ## Validation Architecture section.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework (Rust)** | `cargo test` per-crate. Workspace packages: `rc-common`, `racecontrol-crate`, `rc-agent-crate` |
| **Framework (TS)** | `vitest` (`packages/contract-tests/vitest.config.ts`). `jest` for kiosk component tests if added. |
| **Config file (Rust)** | Workspace `Cargo.toml` + per-crate `[[test]]` blocks + built-in `#[cfg(test)]` modules |
| **Config file (TS)** | `packages/contract-tests/vitest.config.ts` (existing) |
| **Quick run (FSM-only)** | `cargo test -p racecontrol-crate --lib billing_fsm` (~3s) |
| **Quick run (broader)** | `cargo test -p racecontrol-crate --lib billing` (~30s) |
| **Full Rust suite** | `cargo test -p rc-common && cargo test -p racecontrol-crate && cargo test -p rc-agent-crate` (~5min) |
| **TS contract tests** | `cd packages/contract-tests && npx vitest run` (~10s) |
| **Pre-commit gate** | `cargo test -p rc-common && cargo test -p racecontrol-crate --lib` (already enforced by hook) |

---

## Sampling Rate

- **After every task commit:** `cargo test -p rc-common && cargo test -p racecontrol-crate --lib billing` (~30s — FSM + timer + billing core)
- **After every plan wave merge:** Full Rust suite + TS contract tests (~5min)
- **Before `/gsd:verify-work`:** Full Rust suite + TS contract tests + **manual venue financial E2E** + **MMA audit** (per CLAUDE.md mandate for billing logic)
- **Max feedback latency:** 30 seconds per task commit

---

## Per-Task Verification Map

| Req ID | Plan | Wave | Behavior | Test Type | Automated Command | File Exists | Status |
|--------|------|------|----------|-----------|-------------------|-------------|--------|
| 414-FSM-01 | 01 | 1 | `BillingEvent::GameStopped` enum variant exists | unit (compile) | `cargo build -p racecontrol-crate` | ✅ existing module | ⬜ pending |
| 414-FSM-02 | 01 | 1 | `Active + GameStopped → WaitingForGame` | unit | `cargo test -p racecontrol-crate --lib billing_fsm::tests::test_active_game_stopped_to_waiting` | ❌ W0 | ⬜ pending |
| 414-FSM-03 | 01 | 1 | `WaitingForGame + End → Completed` (auto-end path) | unit | `cargo test ... test_waiting_end_to_completed` | ❌ W0 | ⬜ pending |
| 414-FSM-04 | 01 | 1 | `WaitingForGame + EndEarly → EndedEarly` (staff-stop path) | unit | `cargo test ... test_waiting_end_early_to_ended_early` | ❌ W0 | ⬜ pending |
| 414-FSM-05 | 01 | 1 | `Completed + GameStopped` rejected | unit | `cargo test ... test_completed_game_stopped_rejected` | ❌ W0 | ⬜ pending |
| 414-TIMER-01 | 02 | 2 | Idle counter increments only when status==WaitingForGame | unit | `cargo test ... timer_idle_counter_advances_only_in_waiting` | ❌ W0 | ⬜ pending |
| 414-TIMER-02 | 02 | 2 | Idle counter resets to 0 on WaitingForGame→Active | unit | `cargo test ... timer_idle_counter_resets_on_resume` | ❌ W0 | ⬜ pending |
| 414-TIMER-03 | 02 | 2 | At 600s, idle_warning fires exactly once | unit | `cargo test ... idle_warning_fires_at_600s_once` | ❌ W0 | ⬜ pending |
| 414-TIMER-04 | 04 | 4 | At 900s, session auto-ends as Completed (via End event) | unit | `cargo test ... idle_auto_ends_at_900s_completed` | ❌ W0 | ⬜ pending |
| 414-PROTOCOL-01 | 03 | 3 | `DashboardEvent::IdleWarning` round-trips through serde | unit | `cargo test -p rc-common --lib test_idle_warning_serde_roundtrip` | ❌ W0 | ⬜ pending |
| 414-PROTOCOL-02 | 03 | 3 | `BillingSessionInfo.between_games_idle_seconds` round-trips Some+None | unit | `cargo test -p rc-common --lib test_billing_info_idle_seconds_roundtrip` | ❌ W0 | ⬜ pending |
| 414-INTEGRATION-01 | 04 | 4 | 25min Active → GameStopped → 7min wait → GameLive → 5min Active → cumulative cost == ₹700 (snap) | integration | `cargo test ... cumulative_snap_25_5_yields_pkg_30` | ❌ W0 | ⬜ pending |
| 414-INTEGRATION-02 | 04 | 4 | **(B4 update)** 16-min idle from mid-stream WaitingForGame → AUTO-END as `Completed` (via BillingEvent::End per CONTEXT.md D-IDLE-AUTOEND) with correct cumulative cost; assertion: `final_status == 'completed'` | integration | `cargo test ... idle_auto_end_completes_with_cumulative_cost` | ❌ W0 | ⬜ pending |
| 414-INTEGRATION-03 | 04 | 4 | Pod offline during WaitingForGame mid-stream + 16min → auto-end as Completed | integration | `cargo test ... pod_offline_in_waiting_auto_ends_completed` | ❌ W0 | ⬜ pending |
| 414-INTEGRATION-04 | 04 | 4 | `stop_billing` HTTP: elapsed_seconds==0 → CancelledNoPlayable+refund (existing); elapsed_seconds>0 → STAFF-TRIGGERED `EndedEarly`+bill cumulative (NOT Completed — auto-end is the Completed path) | e2e with DB | `cargo test --test billing_session_e2e stop_billing_branches_on_elapsed` | ❌ W0 | ⬜ pending |
| 414-CONTRACT-01 | 03 | 3 | TS `BillingSession.between_games_idle_seconds` matches Rust shape | unit (TS) | `cd packages/contract-tests && npx vitest run billing.contract.test.ts` | ✅ existing — append assertion | ⬜ pending |
| 414-CONTRACT-02 | 03 | 3 | `IdleWarning` event TS shape matches Rust shape | unit (TS) | `cd packages/contract-tests && npx vitest run ws-dashboard.contract.test.ts` | ✅ existing — add fixture+test | ⬜ pending |
| 414-FRONTEND-01 | 05 | 5 | Kiosk staff page renders Continue/End buttons when status=WaitingForGame AND elapsed_seconds>0 | component (manual + Playwright optional) | venue verification | ❌ W0 — Playwright spec optional | ⬜ pending |
| 414-FRONTEND-02 | 05 | 5 | IdleWarningModal renders + countdown decrements + Continue resets via game launch | component (manual) | venue verification | ❌ Manual-only | ⬜ pending |
| 414-FINANCIAL-E2E | 06 | 6 | At venue: customer plays AC 10min → swaps to F1 25 10min → ends — wallet debit matches snap math | manual at venue | n/a | **CLAUDE.md MANDATE** | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## B4 Decision Matrix (revision iter 1)

Per CONTEXT.md D-IDLE-AUTOEND lock + edge case 4, two distinct paths from mid-stream WaitingForGame use DIFFERENT FSM events:

| Path | Trigger | FSM Event | Final Status | Test |
|------|---------|-----------|--------------|------|
| Auto-end | tick_all_timers detects between_games_idle_seconds >= 900 | `BillingEvent::End` | `Completed` | INTEGRATION-02 (`final_status == 'completed'`) |
| Staff-triggered | stop_billing HTTP handler called by staff/customer with elapsed_seconds > 0 | `BillingEvent::EndEarly` | `EndedEarly` | INTEGRATION-04 (`final_status == 'ended_early'`) |
| Kiosk End Session button (Plan 05) | Routes through stop_billing handler | `BillingEvent::EndEarly` | `EndedEarly` | Plan 05 Task 3 venue verify |

Plan 01 added BOTH FSM transitions: `(WaitingForGame, End, Completed)` AND `(WaitingForGame, EndEarly, EndedEarly)`.

---

## Wave 0 Requirements

- [ ] `crates/racecontrol/src/billing_fsm.rs::tests` — append 5 unit tests (FSM-02..05). All marked `#[ignore = "Wave 1 fills TRANSITION_TABLE..."]` so pre-commit gate passes (B1 fix). Plan 01 removes the `#[ignore]` attributes.
- [ ] `crates/racecontrol/src/billing_tests.rs` — append 6 tests (TIMER-01..04, INTEGRATION-01..03). Follow `snap_debit_normal_at_15` pattern for cumulative snap test. All marked `#[ignore = "Wave N implements"]`.
- [ ] `crates/racecontrol/tests/billing_session_e2e.rs` — NEW file for INTEGRATION-04. Mirror Phase 314 `billing_atomicity` integration patterns. Marked `#[ignore = "Plan 04 implements"]`.
- [ ] `crates/rc-common/src/protocol.rs::tests` — append 2 round-trip tests (PROTOCOL-01, PROTOCOL-02). Mirror `test_dashboard_event_session_paused`. Marked `#[ignore = "Plan 03 implements"]`.
- [ ] `packages/contract-tests/src/fixtures/ws-dashboard.json` — add `idle_warning` and `billing_tick_between_games` fixtures.
- [ ] `packages/contract-tests/src/ws-dashboard.contract.test.ts` — add 2 tests for new fixtures.
- [ ] `packages/contract-tests/src/billing.contract.test.ts` — append `between_games_idle_seconds` field assertion (marked `describe.skip`; Plan 03 removes).

*No new framework install needed — cargo test and vitest already configured.*

**B1 (revision iter 1): Pre-commit gate compatibility — every Wave 0 stub test MUST be `#[ignore]`'d so `cargo test --lib` passes between Wave 0 commit and Wave 1 commit. Without this, the pre-commit hook would block ALL commits in the repo.**

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Kiosk Continue/End buttons render correctly in WaitingForGame state | 414-FRONTEND-01 | Visual layout, touch targets, brand colour usage | At venue: book session → start game → end game cleanly via menu → observe staff kiosk for buttons + paused-meter UI; verify no layout shift |
| IdleWarning modal countdown + balance display + branched CTAs | 414-FRONTEND-02 | Live countdown timing, modal focus, ESC behaviour | At venue: book session → start AC → end via menu → wait 10 min → modal appears with countdown + balance; tap Continue → game-select opens; alternate: drain wallet to <₹25 → verify "Insufficient balance" branch |
| Real wallet debit matches snap math across game swap | 414-FINANCIAL-E2E | "Financial flow E2E" mandated by CLAUDE.md before any billing/wallet deploy | At venue: top up customer to known balance → start AC → drive 10 min → close AC cleanly → wait 2 min → start F1 25 → drive 10 min → end session → verify wallet debit matches expected snap-cost (20min × ₹25 = ₹500, or ₹700 if snap to 30-min tier triggers) |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies (assigned to plans by gsd-planner)
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references (5 Rust test files + 3 TS test/fixture files)
- [ ] Wave 0 stubs all `#[ignore]`'d so pre-commit gate passes (B1 — revision iter 1)
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s per task commit
- [ ] `nyquist_compliant: true` set in frontmatter (after gsd-planner assigns Plan/Wave to each REQ)
- [ ] **Pre-deploy:** Full Rust suite + TS contracts + venue financial E2E + MMA audit ALL green

**Approval:** pending (awaiting gsd-planner Plan/Wave assignment + gsd-plan-checker verify pass)
</content>
</invoke>
