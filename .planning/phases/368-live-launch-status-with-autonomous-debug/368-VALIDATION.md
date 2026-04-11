---
phase: 368
slug: live-launch-status-with-autonomous-debug
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-04-11
---

# Phase 368 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.
> **Source:** Derived from 368-RESEARCH.md §Validation Architecture.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework (Rust)** | `cargo test` (workspace: racecontrol-crate + rc-agent-crate + rc-common) |
| **Framework (TS kiosk)** | `jest` (kiosk app unit tests) |
| **Framework (Playwright)** | `@playwright/test` (tests/e2e + tests/page-crawler) |
| **Config file (Rust)** | `Cargo.toml` workspace root |
| **Config file (Playwright)** | `tests/e2e/smart-pipes/playwright.config.ts` + `tests/page-crawler/playwright.config.ts` |
| **Quick run command** | `cargo test -p rc-common launch_status_serde && cargo test -p racecontrol-crate launch_state_machine` |
| **Full suite command (backend)** | `cargo test -p rc-common && cargo test -p racecontrol-crate && cargo test -p rc-agent-crate` |
| **Full suite command (frontend)** | `cd kiosk && npm test` |
| **Playwright probe** | `PROBE_SPEC=probe-debug-launches.spec.ts npx playwright test --config tests/page-crawler/playwright.config.ts --project=kiosk` |
| **Contract test (cross-boundary)** | `cargo test -p rc-common launch_status_value_contract` (Phase 62 enum-value contract pattern) |
| **Estimated runtime (quick)** | ~15 seconds |
| **Estimated runtime (full)** | ~180 seconds |

---

## Sampling Rate

- **After every task commit:** Run the quick-run command (`cargo test -p rc-common launch_status_serde && cargo test -p racecontrol-crate launch_state_machine`) — ~15s feedback
- **After every plan wave:** Run the full backend suite AND the Playwright probe — ~180s feedback
- **Before `/gsd:verify-work`:** Full backend suite + full Playwright probe + MMA audit (mandatory for cross-system bridge)
- **Max feedback latency:** 15 seconds per-task, 180 seconds per-wave

---

## Per-Task Verification Map

> Populated by the planner. Every task MUST appear in this table with an automated verification command OR a Wave 0 stub dependency, OR an explicit "manual-only" justification in the section below.

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 368-01-01 | 01 | 1 | LLS-01 | unit | `cargo test -p rc-common dashboard_event_launch_status_serde` | ❌ W0 | ⬜ pending |
| 368-01-02 | 01 | 1 | LLS-01, LLS-02 | unit | `cargo test -p rc-common launch_status_value_contract` | ❌ W0 | ⬜ pending |
| 368-01-03 | 01 | 1 | LLS-03, LLS-10 | unit | `cargo test -p racecontrol-crate launch_state_machine::transition` | ❌ W0 | ⬜ pending |
| 368-01-04 | 01 | 1 | LLS-03 | integration | `cargo test -p racecontrol-crate launch_id_threaded_through_agent` | ❌ W0 | ⬜ pending |
| 368-02-01 | 02 | 2 | LLS-04 | unit | `cargo test -p rc-agent-crate game_doctor_emits_analysis_event` | ❌ W0 | ⬜ pending |
| 368-02-02 | 02 | 2 | LLS-04 | unit | `cargo test -p rc-agent-crate tier_engine_emits_fix_boundaries` | ❌ W0 | ⬜ pending |
| 368-02-03 | 02 | 2 | LLS-04 | integration | `cargo test -p racecontrol-crate ws_launch_status_relay` | ❌ W0 | ⬜ pending |
| 368-03-01 | 03 | 2 | LLS-05 | unit | `cargo test -p racecontrol-crate launch_notes_migration_idempotent` | ❌ W0 | ⬜ pending |
| 368-03-02 | 03 | 2 | LLS-05 | unit | `cargo test -p racecontrol-crate launch_notes_append_only` | ❌ W0 | ⬜ pending |
| 368-03-03 | 03 | 2 | LLS-06 | integration | `cargo test -p racecontrol-crate debug_launches_endpoints_staff_gated` | ❌ W0 | ⬜ pending |
| 368-03-04 | 03 | 2 | LLS-06 | integration | `cargo test -p racecontrol-crate approve_fix_tier_gate` | ❌ W0 | ⬜ pending |
| 368-03-05 | 03 | 2 | LLS-07 | unit | `cargo test -p racecontrol-crate feature_flag_kiosk_launch_cards` | ❌ W0 | ⬜ pending |
| 368-03-06 | 03 | 2 | LLS-05 | unit | `cargo test -p racecontrol-crate cloud_sync_includes_launch_notes` | ❌ W0 | ⬜ pending |
| 368-04-01 | 04 | 3 | LLS-08 | unit | `cd kiosk && npm test -- LaunchCard.test.tsx` | ❌ W0 | ⬜ pending |
| 368-04-02 | 04 | 3 | LLS-08 | unit | `cd kiosk && npm test -- useKioskSocket.launch.test.ts` | ❌ W0 | ⬜ pending |
| 368-04-03 | 04 | 3 | LLS-09 | e2e | `PROBE_SPEC=probe-debug-launches.spec.ts npx playwright test --config tests/page-crawler/playwright.config.ts --project=kiosk` | ❌ W0 | ⬜ pending |
| 368-04-04 | 04 | 3 | LLS-11 | contract | `cargo test -p rc-common launch_status_value_contract && cd kiosk && npm test -- launch-status-types.test.ts` | ❌ W0 | ⬜ pending |
| 368-04-05 | 04 | 4 | LLS-12 | mma | `node scripts/multi-model-audit.js --phase 368 --mode bridge` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

> **Legend — Wave 0:** Tests marked `❌ W0` do not exist yet and MUST be created in the Wave 0 task for the corresponding plan. No execution task may claim "done" until its matching verification file exists.

---

## Wave 0 Requirements

All verification files must exist before any execution task runs. Wave 0 creates the test stubs:

### Rust test stubs

- [ ] `crates/rc-common/tests/launch_status_serde.rs` — stubs for LLS-01 (DashboardEvent serde roundtrip for LaunchStatusChanged + LaunchNoteAdded variants)
- [ ] `crates/rc-common/tests/launch_status_value_contract.rs` — Phase 62 enum-value contract: asserts exact string values for LaunchState enum discriminants match what kiosk/src/lib/types.ts expects
- [ ] `crates/racecontrol/tests/launch_state_machine.rs` — LaunchStateMachine transition unit tests + invariants (one launch_id per state, monotonic transitions, terminal states)
- [ ] `crates/racecontrol/tests/launch_id_threading.rs` — integration: server mints launch_id, rc-agent receives it via CoreToAgentMessage::LaunchGame.launch_id (backward-compat via #[serde(default)])
- [ ] `crates/rc-agent/tests/game_doctor_emits_analysis.rs` — game_doctor::analyze_game_failure() writes to ws_msg_tx channel
- [ ] `crates/rc-agent/tests/tier_engine_emits_fix.rs` — tier_engine at DiagnosticTrigger::GameLaunchFail branch emits issue_being_fixed + issue_fixed + needs_manual_intervention
- [ ] `crates/racecontrol/tests/ws_launch_status_relay.rs` — server receives LaunchStatusUpdate AgentMessage, broadcasts LaunchStatusChanged DashboardEvent to kiosk WS subscribers
- [ ] `crates/racecontrol/tests/launch_notes.rs` — idempotent CREATE TABLE, append-only constraint, staff_id + staff_name persistence, idx_launch_notes_launch_id exists
- [ ] `crates/racecontrol/tests/debug_launches_routes.rs` — 5 new /api/v1/debug/launches/* endpoints staff-JWT-gated (401 without, 200 with)
- [ ] `crates/racecontrol/tests/approve_fix_tier_gate.rs` — POST /debug/launches/{id}/approve-fix rejects Tier 1 auto-apply (already applied) and accepts Tier 2+ with staff token
- [ ] `crates/racecontrol/tests/feature_flag_launch_cards.rs` — kiosk_launch_cards_enabled toggle: OFF → poll fallback ACTIVE; ON → WS push path ACTIVE
- [ ] `crates/racecontrol/tests/cloud_sync_launch_notes.rs` — SYNC_TABLES constant includes "launch_notes"; sync replicates new row to cloud replica

### Kiosk test stubs

- [ ] `kiosk/src/components/__tests__/LaunchCard.test.tsx` — state timeline renders 4 dots with active state highlighted; note composer posts to /api/v1/debug/launches/{id}/notes with Bearer token; dismiss button shown when state is `issue_fixed` or `needs_manual_intervention`; empty state renders when no launches
- [ ] `kiosk/src/hooks/__tests__/useKioskSocket.launch.test.ts` — new launch_status_changed + launch_note_added WS event handlers update launches Map correctly; existing game_state_changed handler untouched
- [ ] `kiosk/src/lib/__tests__/launch-status-types.test.ts` — TypeScript literal union for LaunchState exactly matches the 5 string values from Rust contract

### Playwright test stub

- [ ] `tests/page-crawler/probe-debug-launches.spec.ts` — load /kiosk/debug with staff auth injected (sessionStorage pattern from probe-debug.spec.ts), feature flag ON, mock WS LaunchStatusChanged event sequence: `launch_started` → `ai_analysis_requested` → `issue_being_fixed` → `issue_fixed`, assert each state renders in DOM, assert card auto-dismisses 5min after `issue_fixed` (use test-clock fast-forward), assert note POST succeeds, screenshot the final state

*If none: N/A — all Wave 0 stubs required.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Visual card design matches kiosk brand (rp-card, rp-border, rp-black tailwind variables) | LLS-08 | Visual judgment — no automated diff baseline yet for the new component | Staff opens `http://192.168.31.23:3300/kiosk/debug` after feature flag enabled; compares to existing debug page visual language |
| Real game launch end-to-end with live pod | LLS-09, LLS-12 | Requires physical pod + real sim game to reproduce the full diagnose-and-fix path; cannot be fully simulated | On a pod with a known flaky launch trigger (e.g., stale acServer), call POST /games/launch from kiosk, observe the debug page card progress through all 4 states, confirm Phase 275 retry applies Tier 1 fix automatically, confirm card auto-dismisses 5min after success |
| MMA audit sign-off | LLS-12 | Multi-model audit produces consensus findings that require human review to triage | Run `node scripts/multi-model-audit.js --phase 368 --mode bridge`; triage findings; address all P1s before claiming phase complete; document outcome in VERIFICATION.md |
| Deploy parity verification | LLS-12 | Venue + cloud deploy must be checked from two separate environments | After venue deploy: probe `/kiosk/debug` from venue LAN; after cloud deploy: probe from external network pointing at `https://racingpoint.cloud/kiosk/debug`; confirm both show the same build_id + flag state |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 15 seconds (quick) / 180 seconds (full)
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
