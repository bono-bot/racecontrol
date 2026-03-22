# Requirements: v18.1 Seamless Execution Hardening

**Defined:** 2026-03-22
**Core Value:** The v18.0 execution relay must survive crashes, reboots, and network drops — and tell you when it can't.

## v18.1 Requirements

### Daemon Recovery

- [x] **RECOV-01**: James comms-link daemon auto-restarts within 30s after a crash
- [x] **RECOV-02**: James comms-link daemon starts automatically on Windows boot
- [x] **RECOV-03**: Watchdog detects comms-link down and restarts it (james_watchdog.ps1)
- [x] **RECOV-04**: Bono notified via WhatsApp/email when James daemon crashes and recovers

### Chain Endpoint Fix

- [ ] **CHAIN-10**: /relay/chain/run returns chain_result synchronously (not 504 timeout)
- [ ] **CHAIN-11**: chain_result WS messages route through ExecResultBroker.handleResult()

### Degradation Visibility

- [ ] **VIS-01**: /relay/health returns connection mode and last heartbeat timestamp
- [ ] **VIS-02**: /relay/exec/run returns 503 with descriptive error when WS disconnected
- [ ] **VIS-03**: Exec skills check relay health before sending and report status

## Out of Scope

| Feature | Reason |
|---------|--------|
| PM2 on James (Windows) | Windows doesn't support PM2 well — Task Scheduler + watchdog is the established pattern |
| node-windows service wrapper | Adds npm dependency + complexity — watchdog + Run key achieves the same with existing infra |
| Chain retry on relay failure | Caller (Claude) can retry — relay should fail fast and report, not silently retry |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| RECOV-01 | Phase 135 | Complete |
| RECOV-02 | Phase 135 | Complete |
| RECOV-03 | Phase 135 | Complete |
| RECOV-04 | Phase 135 | Complete |
| CHAIN-10 | Phase 136 | Pending |
| CHAIN-11 | Phase 136 | Pending |
| VIS-01 | Phase 136 | Pending |
| VIS-02 | Phase 136 | Pending |
| VIS-03 | Phase 136 | Pending |

**Coverage:**
- v18.1 requirements: 9 total
- Mapped to phases: 9
- Unmapped: 0

---
*Requirements defined: 2026-03-22*
*Last updated: 2026-03-22 after roadmap creation (v18.1)*
