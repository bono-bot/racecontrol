# Requirements: Racing Point eSports — Meshed Intelligence

**Defined:** 2026-03-27
**Core Value:** Every venue node publishes and subscribes to a shared event fabric, enabling lateral awareness, autonomous coordination, and resilient degraded-mode operation.

## v27.0 Requirements

Requirements for Meshed Intelligence milestone. Each maps to roadmap phases.

### Event Backbone

- [ ] **EBUS-01**: NATS 2.12 server running as Windows Service on .23 with JetStream enabled and 7 streams (PODS, CAMERAS, POS, AI, FUSION, VENUE, AUDIT)
- [ ] **EBUS-02**: Shared event envelope schema with `version: u32` field in shared-types crate, serde structs for all event types
- [ ] **EBUS-03**: rc-agent publishes health/telemetry/state events via async-nats 0.46 to `pod.<id>.*` subjects
- [ ] **EBUS-04**: `MESH_ENABLED` feature flag for parallel write (existing WS probes + new NATS publish), toggleable per pod
- [ ] **EBUS-05**: natscli installed on server for ops debugging, stream inspection, and consumer management

### Pod Mesh

- [ ] **MESH-01**: Pod heartbeat every 5s on `pod.<id>.heartbeat` replacing 30s WebSocket polling
- [ ] **MESH-02**: Digital twin KV bucket (`KV_PODS`) with TTL=120s and history=5, dashboards use KV Watch API instead of HTTP polling
- [ ] **MESH-03**: mDNS peer discovery via mdns-sd/swarm-discovery crate with static IP fallback table (192.168.31.x)
- [ ] **MESH-04**: Local leader election (lowest-IP pod becomes coordinator) when server is unreachable via mDNS membership

### Camera Fusion

- [ ] **FUSE-01**: NVR webhook bridge: camera motion detection events from NVR webhooks published to `camera.<id>.detection` on NATS
- [ ] **FUSE-02**: Fusion service with 5s sliding window joins of `pod.*` + `camera.*` events, producing correlated venue-level facts
- [ ] **FUSE-03**: Confidence scoring: missing camera input degrades confidence score, does not halt inference or produce false negatives
- [ ] **FUSE-04**: Fused output events published to `fusion.pod.<id>.occupied`, `fusion.safety.*`, `fusion.queue.*`

### AI Agent Mesh

- [ ] **AIAG-01**: Blackboard KV bucket (`KV_BLACKBOARD`) for James + Bono shared coordination state
- [ ] **AIAG-02**: Intent-based command system: agents emit `ai.<agent>.intent` events, intent controller approves/rejects before execution
- [ ] **AIAG-03**: Graph-based workflows: detect→verify→fix→escalate state machines for known failure patterns (pod crash, session hang, hardware fault)
- [ ] **AIAG-04**: Per-pod per-action cooldown (1 restart/pod/5min) + circuit breaker (3 actions/pod/60s → WhatsApp alert to Uday, halt automation)

### Predictive Operations

- [ ] **PRED-01**: Rule-based risk scoring using EWMA + threshold detectors for pod failure probability, published to `venue.prediction.risk.<id>`
- [ ] **PRED-02**: Demand forecasting from historical session/billing data to predict busy periods and trigger pod pre-warming
- [ ] **PRED-03**: Prediction events on `venue.prediction.*` consumed by admin dashboard and AI agents for proactive action

### Resilience

- [ ] **RESIL-01**: SQLite transactional outbox: write state change + pending event atomically in single ACID transaction, relay publishes to NATS
- [ ] **RESIL-02**: Tiered degradation: Tier 0 (racing) never gated on NATS ack, Tier 1 (billing) spools to SQLite, Tier 2 (analytics/AI) halts gracefully
- [ ] **RESIL-03**: Rate-limited event replay with idempotency keys on NATS reconnection, preventing duplicate event processing
- [ ] **RESIL-04**: Conflict resolution policy: server wins for configuration changes, pods win for billing/session events on reconnect after split-brain
- [ ] **RESIL-05**: Chaos testing scripts: simulate NATS down, pod failure, network partition for off-hours regression testing

### Capacity Intelligence

- [ ] **CAPI-01**: Live capacity dashboard: real-time CPU/RAM/GPU/disk/network metrics across all pods + server with configurable thresholds and alerts
- [ ] **CAPI-02**: Historical load analysis: identify peak load patterns, bottlenecks, and capacity headroom from mesh event history (7-30 day window)
- [ ] **CAPI-03**: Stress test framework: simulate peak load (all 8 pods + cameras + POS + AI agents active) to identify breaking points before they happen in production

### Unified Protocol

- [ ] **UPRO-01**: Cross-category health probe: single command verifies NATS → pod publishers → KV twin → fusion → AI → spool chain end-to-end
- [ ] **UPRO-02**: Integration test suite: after each phase ships, automated tests validate new category interoperates with all previously shipped categories
- [ ] **UPRO-03**: Mesh topology validator: verify all expected NATS subscriptions, KV buckets, streams, and leaf node connections are active and healthy
- [ ] **UPRO-04**: Event flow tracer: inject tagged test event at any entry point, trace its path through the entire mesh, report which services consumed it
- [ ] **UPRO-05**: Regression gate: phase cannot be marked shipped until UPRO integration tests pass against all previously shipped categories

## Future Requirements

Deferred to future release. Tracked but not in current roadmap.

### Authentication & Security

- **SEC-01**: NATS NKey authentication on venue LAN (prevents rogue event injection)
- **SEC-02**: Event signing for tamper detection on critical subjects (billing, audit)
- **SEC-03**: TLS encryption for NATS connections (currently LAN-trusted)

### Advanced Analytics

- **ANAL-01**: ML-based pod failure prediction using linfa (requires 60+ days of event history)
- **ANAL-02**: Customer behavior analytics from fused session + camera + POS events
- **ANAL-03**: Real-time anomaly detection using statistical models on event streams

### Multi-Venue

- **MULTI-01**: Cross-venue mesh via NATS super-cluster
- **MULTI-02**: Centralized analytics across multiple venue locations

## Out of Scope

| Feature | Reason |
|---------|--------|
| Eclipse Zenoh | Less operationally proven than NATS at venue scale; revisit in v25+ |
| Kafka / Flink | Overkill for 8-pod venue; NATS JetStream sufficient |
| Online ML training | Offline model training only in v27; no online learning |
| Mobile mesh client | Web-first; no mobile app mesh participation |
| Cross-venue mesh | Single venue only; multi-site deferred to v25+ |
| Full NATS clustering | Single server sufficient for venue scale; no quorum benefit at 8 pods |
| Real-time video analytics | NVR webhooks for detection events, not frame-by-frame processing |

## Traceability

Which phases cover which requirements. Updated during roadmap creation.

| Requirement | Phase | Status |
|-------------|-------|--------|
| EBUS-01 | Phase 241 | Pending |
| EBUS-02 | Phase 241 | Pending |
| EBUS-05 | Phase 241 | Pending |
| EBUS-03 | Phase 242 | Pending |
| EBUS-04 | Phase 242 | Pending |
| MESH-01 | Phase 242 | Pending |
| MESH-02 | Phase 243 | Pending |
| RESIL-01 | Phase 244 | Pending |
| RESIL-02 | Phase 244 | Pending |
| RESIL-03 | Phase 244 | Pending |
| RESIL-04 | Phase 244 | Pending |
| MESH-03 | Phase 245 | Pending |
| MESH-04 | Phase 245 | Pending |
| FUSE-01 | Phase 246 | Pending |
| FUSE-02 | Phase 246 | Pending |
| FUSE-03 | Phase 246 | Pending |
| FUSE-04 | Phase 246 | Pending |
| AIAG-01 | Phase 247 | Pending |
| AIAG-02 | Phase 247 | Pending |
| AIAG-03 | Phase 247 | Pending |
| AIAG-04 | Phase 247 | Pending |
| PRED-01 | Phase 248 | Pending |
| PRED-02 | Phase 248 | Pending |
| PRED-03 | Phase 248 | Pending |
| CAPI-01 | Phase 249 | Pending |
| CAPI-02 | Phase 249 | Pending |
| CAPI-03 | Phase 249 | Pending |
| UPRO-01 | Phase 250 | Pending |
| UPRO-02 | Phase 250 | Pending |
| UPRO-03 | Phase 250 | Pending |
| UPRO-04 | Phase 250 | Pending |
| UPRO-05 | Phase 250 | Pending |
| RESIL-05 | Phase 250 | Pending |

**Coverage:**
- v27.0 requirements: 33 total
- Mapped to phases: 33
- Unmapped: 0

---
*Requirements defined: 2026-03-27*
*Last updated: 2026-03-27 after roadmap creation — all 33 requirements mapped*
