---
phase: 5
slug: kiosk-pin-launch
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-21
---

# Phase 5 — Validation Strategy

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust) + manual E2E (kiosk UI) |
| **Quick run command** | `cargo test -p racecontrol -- reservation` |
| **Full suite command** | `cargo test -p rc-common && cargo test -p racecontrol` |

## Sampling Rate

- **After every task commit:** `cargo test -p racecontrol -- reservation`
- **After every plan wave:** `cargo test -p rc-common && cargo test -p racecontrol`
- **Before `/gsd:verify-work`:** Full suite must be green

## Per-Task Verification Map

| Req ID | Behavior | Test Type | File Exists | Status |
|--------|----------|-----------|-------------|--------|
| KIOSK-01 | PIN entry screen renders | manual E2E | N/A | Manual |
| KIOSK-02 | PIN validated against synced reservations | unit | No | Wave 0 |
| KIOSK-03 | Valid PIN triggers pod assignment + game launch | unit | No | Wave 0 |
| KIOSK-04 | Rate limiting: 5/min, lockout after 10 | unit | No | Wave 0 |
| KIOSK-05 | PIN one-time use, marked redeemed | unit | No | Wave 0 |
| KIOSK-06 | Customer sees pod number + loading status | manual E2E | N/A | Manual |

---

*Phase: 05-kiosk-pin-launch*
*Validation strategy created: 2026-03-21*
