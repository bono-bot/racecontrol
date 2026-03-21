---
phase: 4
slug: remote-booking-pin-generation
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-21
---

# Phase 4 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust) + manual E2E (no Jest for PWA) |
| **Config file** | Cargo.toml workspace test config |
| **Quick run command** | `cargo test -p racecontrol -- reservation` |
| **Full suite command** | `cargo test -p rc-common && cargo test -p racecontrol` |
| **Estimated runtime** | ~30 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p racecontrol -- reservation`
- **After every plan wave:** Run `cargo test -p rc-common && cargo test -p racecontrol`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Req ID | Behavior | Test Type | Automated Command | File Exists | Status |
|--------|----------|-----------|-------------------|-------------|--------|
| BOOK-01 | Customer can book experience from PWA | manual E2E | N/A (UI flow) | N/A | Manual |
| BOOK-02 | Pod-agnostic reservation created | unit | `cargo test -p racecontrol -- reservation::test_create` | No | Wave 0 |
| BOOK-03 | 6-char PIN generated, displayed | unit | `cargo test -p racecontrol -- pin::test_generate` | No | Wave 0 |
| BOOK-04 | PIN delivered via WhatsApp | manual | N/A (requires Evolution API) | N/A | Manual |
| BOOK-05 | View/cancel/modify reservation | unit + manual | `cargo test -p racecontrol -- reservation::test_cancel` | No | Wave 0 |
| BOOK-06 | Reservations expire after TTL | unit | `cargo test -p racecontrol -- reservation::test_expiry` | No | Wave 0 |
| BOOK-07 | Expired reservations auto-refund | unit | `cargo test -p racecontrol -- reservation::test_expiry_refund` | No | Wave 0 |
| API-04 | Reservation CRUD endpoints | integration | `cargo test -p racecontrol -- reservation::test_api` | No | Wave 0 |

---

## Wave 0 Gaps

- [ ] `crates/racecontrol/src/reservation.rs` — new module with unit tests for PIN generation, reservation CRUD, expiry logic
- [ ] No PWA test infrastructure exists (no jest.config, no test files) — PWA testing is manual E2E only

---

## Coverage Targets

| Requirement | Tests Required | Tests Exist | Gap |
|-------------|---------------|-------------|-----|
| BOOK-01 | 0 (manual) | 0 | 0 |
| BOOK-02 | 1 | 0 | 1 |
| BOOK-03 | 1 | 0 | 1 |
| BOOK-04 | 0 (manual) | 0 | 0 |
| BOOK-05 | 1 | 0 | 1 |
| BOOK-06 | 1 | 0 | 1 |
| BOOK-07 | 1 | 0 | 1 |
| API-04 | 1 | 0 | 1 |
| **Total** | **6** | **0** | **6** |

---

*Phase: 04-remote-booking-pin-generation*
*Validation strategy created: 2026-03-21*
