# Requirements: v18.2 Debugging & Quality Gates

**Defined:** 2026-03-22
**Core Value:** Bugs must be caught by automated tests BEFORE deploy, not discovered manually after. Standing rules must be organized so they're actually followed, not skimmed.

## v18.2 Requirements

### Rules Hygiene

- [x] **RULES-01**: CLAUDE.md standing rules reorganized into named categories (Deploy, Comms, Code Quality, Process, Debugging)
- [x] **RULES-02**: Obsolete/duplicate rules pruned (rules superseded by v18.0, duplicate across sections)
- [x] **RULES-03**: Each rule has a one-line justification comment so future sessions can evaluate relevance
- [x] **RULES-04**: standing-rules.md memory file matches CLAUDE.md (no drift)

### Integration Tests

- [ ] **INTEG-01**: Integration test starts comms-link daemon, sends exec_request over WS, verifies exec_result with correct fields
- [x] **INTEG-02**: Integration test sends chain_request with 2+ steps, verifies chain_result with matching chainId and all step outputs
- [ ] **INTEG-03**: Integration test sends message with from:james, verifies relay and persistence
- [x] **INTEG-04**: Cross-platform syntax check runs node --check on all source files on both James and Bono
- [x] **INTEG-05**: Contract tests verify chainId passthrough, from field preservation, MessageType routing

### GSD Gate

- [ ] **GATE-01**: Integration test invocable with single command (node test/integration.js or bash test/e2e.sh)
- [ ] **GATE-02**: GSD execute-phase verifier runs integration test as part of phase verification
- [ ] **GATE-03**: Integration test failures block phase from being marked complete

## Out of Scope

| Feature | Reason |
|---------|--------|
| Full CI/CD pipeline | v17.0 Phase 127 handles CI/CD — this milestone is about the local test gate |
| Rewriting GSD executor | The executor is fine — just needs the integration test wired as a post-step |
| Mock-based integration tests | Defeats the purpose — tests must run against real daemons |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| RULES-01 | Phase 142 | Complete (2026-03-22) |
| RULES-02 | Phase 142 | Complete (2026-03-22) |
| RULES-03 | Phase 142 | Complete (2026-03-22) |
| RULES-04 | Phase 142 | Complete (2026-03-22) |
| INTEG-01 | Phase 143 | Pending |
| INTEG-02 | Phase 143 | Complete |
| INTEG-03 | Phase 143 | Pending |
| INTEG-04 | Phase 143 | Complete |
| INTEG-05 | Phase 143 | Complete |
| GATE-01 | Phase 144 | Pending |
| GATE-02 | Phase 144 | Pending |
| GATE-03 | Phase 144 | Pending |

**Coverage:**
- v18.2 requirements: 12 total
- Mapped to phases: 12
- Unmapped: 0

---
*Requirements defined: 2026-03-22*
*Last updated: 2026-03-22 after roadmap creation (phases 142-144 assigned)*
