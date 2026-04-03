# Requirements: v42.0 Meshed Intelligence Migration

**Defined:** 2026-04-03
**Core Value:** Move MI tier engine from rc-agent to rc-sentry — eliminate the blind spot where rc-agent death kills the entire self-healing system.

## v42.0 Requirements

Requirements for this milestone. Each maps to roadmap phases.

### MI Core Migration (MIG)

- [ ] **MIG-01**: Tier engine (5-tier decision tree) runs inside rc-sentry, diagnosing rc-agent health from outside
- [ ] **MIG-02**: Diagnostic engine (anomaly detection, trigger classification) runs in rc-sentry with full event channel
- [ ] **MIG-03**: Knowledge base (SQLite solution DB, pattern matching, KB lifecycle) runs in rc-sentry
- [ ] **MIG-04**: MMA engine (OpenRouter multi-model audit) + budget tracker runs in rc-sentry
- [ ] **MIG-05**: rc-agent retains thin MI proxy that forwards telemetry to rc-sentry (backward compatible during migration)
- [ ] **MIG-06**: Cognitive gate + diagnosis planner moved to rc-sentry for structured fix planning

### External Monitoring (MON)

- [ ] **MON-01**: rc-sentry monitors rc-agent via process inspection (tasklist) + health endpoint polling, independent of rc-agent API
- [ ] **MON-02**: Server pod_healer falls back to rc-sentry :8091 when rc-agent :8090 is unreachable
- [ ] **MON-03**: Crash loop breaker detects 3+ restarts in 10min, applies exponential backoff, clears stale sentinels automatically
- [ ] **MON-04**: COMMS_PSK deployed to all 8 pods + POS, watchdog sends WhatsApp/Bono alerts when rc-agent dies
- [ ] **MON-05**: rc-sentry captures and analyzes pod screenshots for visual verification of blanking/kiosk state

### True Mesh Intelligence (MESH)

- [ ] **MESH-01**: Pod-to-pod direct communication channel (not just via server) for low-latency coordination
- [ ] **MESH-02**: Multiplayer game state sync — pods hosting the same F1 25 or AC session can coordinate launch/stop
- [ ] **MESH-03**: Fleet-wide solution gossip propagates through mesh (pod discovers fix → direct broadcast to peers)

## Future Requirements

Deferred to future milestones. Tracked but not in current roadmap.

### Autonomous Actions (v32.0 scope)

- **AUTO-01**: Autonomous game launch fix + cascade (diagnose → fix → retry → KB harden → gossip)
- **AUTO-02**: Predictive alert → action pipeline (connect predictive_maintenance to tier engine)
- **AUTO-03**: Experience scoring integration (auto-flag/remove low-scoring pods)
- **AUTO-04**: Revenue protection triggers (game running without billing, session ended but game active)

## Out of Scope

| Feature | Reason |
|---------|--------|
| Full v32.0 autonomous actions | Requires MI migration first — this milestone provides the foundation |
| Multi-venue cloud KB sync | Deferred — single venue focus for now |
| rc-sentry GUI operations | rc-sentry runs in Session 0 (services) — GUI stays in rc-agent |
| Replacing rc-agent entirely | rc-agent still needed for game launch, lock screen, Edge control |
| tokio runtime in rc-sentry | Keep std threads for now — evaluate async migration separately |

## Traceability

Which phases cover which requirements. Updated during roadmap creation.

| Requirement | Phase | Status |
|-------------|-------|--------|
| MIG-01 | Phase 322 | Pending |
| MIG-02 | Phase 322 | Pending |
| MIG-03 | Phase 322 | Pending |
| MIG-04 | Phase 323 | Pending |
| MIG-05 | Phase 322 | Pending |
| MIG-06 | Phase 323 | Pending |
| MON-01 | Phase 321 | Pending |
| MON-02 | Phase 321 | Pending |
| MON-03 | Phase 321 | Pending |
| MON-04 | Phase 321 | Pending |
| MON-05 | Phase 321 | Pending |
| MESH-01 | Phase 324 | Pending |
| MESH-02 | Phase 324 | Pending |
| MESH-03 | Phase 324 | Pending |

**Coverage:**
- v42.0 requirements: 14 total
- Mapped to phases: 14
- Unmapped: 0 (100% coverage)

---
*Requirements defined: 2026-04-03*
*Last updated: 2026-04-03 — traceability filled after roadmap creation*
