---
phase: 367
slug: staff-tools
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-04-09
---

# Phase 367 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust: cargo test (racecontrol-crate, rc-agent-crate) / TypeScript: none (no Jest in admin portal) |
| **Config file** | `Cargo.toml` (workspace), `racingpoint-admin/package.json` |
| **Quick run command** | `cargo test -p racecontrol-crate --lib 2>&1 \| tail -3` |
| **Full suite command** | `cargo test -p racecontrol-crate && cargo test -p rc-agent-crate -p rc-common` |
| **Estimated runtime** | ~90 seconds (full suite) |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p racecontrol-crate --lib 2>&1 | tail -3`
- **After every plan wave:** Run full suite
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 90 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | Status |
|---------|------|------|-------------|-----------|-------------------|--------|
| 367-01-01 | 01 | 1 | GLD-G-01 | integration | `cargo test -p racecontrol-crate --lib test_suspect_sessions_route` | pending |
| 367-01-02 | 01 | 1 | GLD-G-01 | integration | `cargo test -p racecontrol-crate --lib test_telemetry_heatmap_route` | pending |
| 367-01-03 | 01 | 2 | GLD-G-01 | manual | Playwright: admin /sessions/suspect renders table with suspect badge | pending |
| 367-02-01 | 02 | 1 | GLD-G-02 | unit | `cargo test -p racecontrol-crate --lib test_pod_verify_handler` | pending |
| 367-02-02 | 02 | 2 | GLD-G-02 | manual | Admin /fleet/verify shows 8 pod buttons, Verify button returns pass/fail | pending |
| 367-03-01 | 03 | 1 | GLD-G-03 | integration | `cargo test -p racecontrol-crate --lib test_session_replay_route` | pending |
| 367-03-02 | 03 | 2 | GLD-G-03 | manual | Admin /sessions/[id]/replay: scrubber advances, speed selector works | pending |
| 367-04-01 | 04 | 1 | GLD-G-04 | integration | `cargo test -p racecontrol-crate --lib test_batch_export_csv` | pending |
| 367-04-02 | 04 | 1 | GLD-G-04 | integration | `cargo test -p racecontrol-crate --lib test_batch_export_30day_limit` | pending |
| 367-04-03 | 04 | 2 | GLD-G-04 | manual | Admin /sessions/export: estimate shows row count, Export downloads file | pending |
| 367-05-01 | 05 | 1 | GLD-G-05 | unit | `cargo test -p rc-agent-crate --lib test_ac_adapter_mismatch_detection` | pending |
| 367-05-02 | 05 | 1 | GLD-G-05 | unit | `cargo test -p rc-agent-crate --lib test_acevo_adapter_mismatch_detection` | pending |
| 367-05-03 | 05 | 1 | GLD-G-05 | unit | `cargo test -p rc-agent-crate --lib test_f125_adapter_mismatch_detection` | pending |
| 367-05-04 | 05 | 1 | GLD-G-05 | unit | `cargo test -p rc-agent-crate --lib test_iracing_adapter_mismatch_detection` | pending |
| 367-05-05 | 05 | 1 | GLD-G-05 | unit | `cargo test -p rc-agent-crate --lib test_lmu_adapter_mismatch_detection` | pending |
| 367-05-06 | 05 | 2 | GLD-G-05 | integration | `cargo test -p racecontrol-crate --lib test_8pod_concurrent_mismatch_load` | pending |
| 367-05-07 | 05 | 3 | GLD-G-05 | manual | Call POST /internal/test/config-mismatch, verify WhatsApp received on staff phone | pending |

*Status: pending -- all tasks not yet started*

---

## Wave 0 Requirements

- [x] `cargo test -p racecontrol-crate` — existing suite (891 tests), use as baseline
- [x] `cargo test -p rc-agent-crate` — existing suite (254 tests), use as baseline
- [ ] New test stubs needed in `crates/racecontrol/tests/integration.rs` for routes: suspect-sessions, telemetry-heatmap, pod-verify, session-replay, batch-export, concurrent-mismatch-load
- [ ] New test stubs needed in `crates/rc-agent/src/sims/` for each adapter's mismatch detection

*Note: No Next.js test framework in admin portal — frontend plans use manual/Playwright verification.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Heatmap colors correct (red/green/grey per coverage) | GLD-G-01 | No Jest in admin portal | Open /sessions/suspect, click suspect session, verify heatmap cell colors match coverage % |
| Replay scrubber advances in real time | GLD-G-03 | Browser interaction | Open /sessions/[id]/replay, click Play at 10x, verify currentIndex advances visually |
| CSV download contains correct columns | GLD-G-04 | File download | Open /sessions/export, export CSV, open in Excel, verify column headers match spec |
| WhatsApp E2E alert received on staff phone | GLD-G-05 | Requires live phone + WA | POST /internal/test/config-mismatch with superadmin JWT, verify WA received within 30s |

---

## Validation Sign-Off

- [ ] All tasks have automated verify or manual steps documented
- [ ] Wave 0 stubs exist before execution starts
- [ ] No watch-mode flags
- [ ] Feedback latency < 90s for automated
- [ ] `nyquist_compliant: true` set in frontmatter when all tests green

**Approval:** pending
