---
phase: 349
slug: db-sync-google-drive
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-04-11
---

# Phase 349 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (racecontrol-crate) |
| **Config file** | crates/racecontrol/Cargo.toml |
| **Quick run command** | `cargo test -p racecontrol-crate -- venue_authority` |
| **Full suite command** | `cargo test -p racecontrol-crate` |
| **Estimated runtime** | ~45 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p racecontrol-crate -- venue_authority`
- **After every plan wave:** Run `cargo test -p racecontrol-crate`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 60 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 349-03-01 | 03 | 1 | SYNC-05 | unit | `cargo test -- venue_authority_guard` | ❌ W0 | ⬜ pending |
| 349-03-02 | 03 | 1 | SYNC-06 | unit | `cargo test -- probe_db_sync` | ❌ W0 | ⬜ pending |
| 349-03-03 | 03 | 1 | SYNC-08 | integration | manual check (sentinel file) | N/A | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] Unit test for `venue_authority_guard()` — returns 409 on cloud for non-authoritative tables, returns None on venue
- [ ] Unit test for `probe_db_sync_lag()` — returns ok when file fresh, degraded when stale, skip on venue

*Existing cargo test infrastructure covers all phase requirements.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Cloud returns 409 on venue-authoritative write | SYNC-05 | Requires deployed cloud binary | curl POST to cloud endpoint, verify 409 + hint |
| Sync lag probe shows real data | SYNC-06 | Requires live DB download from Drive | Check /api/health on cloud, verify db_sync probe |
| Pause replication sentinel | SYNC-08 | Requires VPS access | touch /tmp/DB_SYNC_PAUSED, verify download-db.sh skips |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 60s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
