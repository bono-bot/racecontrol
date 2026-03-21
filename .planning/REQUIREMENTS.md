# Requirements: Racing Point Operations — v15.0 AntiCheat Compatibility

**Defined:** 2026-03-21
**Core Value:** Customers never get banned because of RaceControl software running alongside their games

## v15.0 Requirements

Requirements for anti-cheat compatibility hardening. Each maps to roadmap phases.

### Audit

- [ ] **AUDIT-01**: Staff can view a risk inventory of every pod-side behavior classified by severity (CRITICAL/HIGH/MEDIUM/LOW) per anti-cheat system (EA Javelin, iRacing EOS, LMU EAC, Kunos, EA WRC)
- [ ] **AUDIT-02**: ConspitLink is audited via Sysinternals Process Monitor for kernel drivers, DLL injection, and process handle behaviors that could trigger anti-cheat
- [ ] **AUDIT-03**: All 8 pods have their Windows 11 edition verified and documented (affects Keyboard Filter vs GPO decision)
- [ ] **AUDIT-04**: Ops team has a per-game anti-cheat compatibility matrix documenting what is safe/unsafe while each game runs

### Safe Mode

- [ ] **SAFE-01**: rc-agent detects protected game launch within 1 second via WMI Win32_ProcessStartTrace event subscription (not polling)
- [ ] **SAFE-02**: rc-agent enters safe mode automatically when a protected game is detected, managed by a state machine in AppState (safe_mode.rs)
- [ ] **SAFE-03**: Safe mode remains active for 30 seconds after the protected game exits (EA Javelin post-game cooldown)
- [ ] **SAFE-04**: Process guard (allowlist enforcement + auto-kill) is suspended during safe mode
- [ ] **SAFE-05**: Ollama LLM queries are suppressed during safe mode (GPU/memory contention + anti-cheat suspicion)
- [ ] **SAFE-06**: Registry write operations are deferred until safe mode exits
- [ ] **SAFE-07**: Billing, lock screen, overlay, heartbeat, and WebSocket exec continue uninterrupted during safe mode

### Hardening

- [ ] **HARD-01**: SetWindowsHookEx keyboard hook (Phase 78) is fully removed and replaced with GPO registry keys (NoWinKeys, DisableTaskMgr)
- [ ] **HARD-02**: rc-agent.exe and rc-sentry.exe are code signed with an OV certificate via signtool in the deploy pipeline
- [ ] **HARD-03**: Shared memory telemetry readers defer MapViewOfFile connect until 5 seconds after game process is stable (anti-cheat init window)
- [ ] **HARD-04**: UDP telemetry sockets are created only when the corresponding game is active and destroyed on game exit
- [ ] **HARD-05**: AC EVO telemetry is feature-flagged off by default until anti-cheat status is confirmed at v1.0 release

### Validation

- [ ] **VALID-01**: Each protected game (F1 25, iRacing, LMU) completes a full staff test session on Pod 8 with safe mode active and no anti-cheat warnings
- [ ] **VALID-02**: Billing lifecycle (start, ticks, end) works correctly during safe mode with no gaps or incorrect amounts
- [ ] **VALID-03**: Kiosk lockdown remains effective (no Win key, no Alt+Tab, no Task Manager) after keyboard hook replacement

## Future Requirements

### v15.1 (deferred)

- **VALID-04**: EA WRC anti-cheat compatibility validated (pending v13.0 EA WRC telemetry adapter)
- **VALID-05**: AC EVO anti-cheat compatibility validated (pending Kunos v1.0 release)
- **SAFE-08**: Windows Keyboard Filter integration (if pods are upgraded to Enterprise/Education SKU)

## Out of Scope

| Feature | Reason |
|---------|--------|
| Anti-cheat SDK integration (EAC/Javelin whitelisting) | Requires publisher partnership — not available to venue operators |
| Kernel-level driver for rc-agent | Explicitly prohibited — user-mode only, forever |
| Modifying game files or game memory | Obvious ban trigger — never considered |
| Anti-cheat bypass or circumvention | Illegal and unethical — we make our software transparent, not invisible |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| AUDIT-01 | — | Pending |
| AUDIT-02 | — | Pending |
| AUDIT-03 | — | Pending |
| AUDIT-04 | — | Pending |
| SAFE-01 | — | Pending |
| SAFE-02 | — | Pending |
| SAFE-03 | — | Pending |
| SAFE-04 | — | Pending |
| SAFE-05 | — | Pending |
| SAFE-06 | — | Pending |
| SAFE-07 | — | Pending |
| HARD-01 | — | Pending |
| HARD-02 | — | Pending |
| HARD-03 | — | Pending |
| HARD-04 | — | Pending |
| HARD-05 | — | Pending |
| VALID-01 | — | Pending |
| VALID-02 | — | Pending |
| VALID-03 | — | Pending |

**Coverage:**
- v15.0 requirements: 19 total
- Mapped to phases: 0
- Unmapped: 19

---
*Requirements defined: 2026-03-21*
*Last updated: 2026-03-21 after initial definition*
