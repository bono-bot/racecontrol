---
phase: 112
slug: rtsp-infrastructure-camera-pipeline
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-21
---

# Phase 112 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust) + manual RTSP stream verification |
| **Config file** | Cargo.toml workspace |
| **Quick run command** | `cargo test -p rc-sentry-ai` |
| **Full suite command** | `cargo test -p rc-sentry-ai && cargo test -p rc-common` |
| **Estimated runtime** | ~15 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p rc-sentry-ai`
- **After every plan wave:** Run `cargo test -p rc-sentry-ai && cargo test -p rc-common`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 15 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 112-01-01 | 01 | 1 | CAM-01 | integration | `curl http://localhost:1984/api/streams` | ❌ W0 | ⬜ pending |
| 112-02-01 | 02 | 1 | CAM-02 | unit | `cargo test -p rc-sentry-ai` | ❌ W0 | ⬜ pending |
| 112-03-01 | 03 | 2 | CAM-03 | integration | `curl http://localhost:8096/health` | ❌ W0 | ⬜ pending |
| 112-04-01 | 04 | 2 | CAM-04 | integration | manual people tracker check | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `crates/rc-sentry-ai/` — new crate scaffold with Cargo.toml
- [ ] `crates/rc-sentry-ai/src/lib.rs` — module structure
- [ ] go2rtc binary downloaded and config created

*If none: "Existing infrastructure covers all phase requirements."*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| RTSP relay proxies 3 cameras for 24+ hours | CAM-01 | Long-duration stability test | Start go2rtc, connect 3 cameras, monitor for 24h |
| People tracker works via relay | CAM-04 | Requires running people tracker service | Start people tracker with relay URL, verify counts |
| NVR recording unaffected | CAM-01 | Physical NVR verification | Check NVR UI shows all cameras recording |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 15s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
