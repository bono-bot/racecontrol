# Project Research Summary

**Project:** v24.0 Meshed Intelligence — Event-Driven Mesh Architecture
**Domain:** IoT fleet management + edge AI + distributed multi-agent coordination (sim racing venue)
**Researched:** 2026-03-27
**Confidence:** MEDIUM-HIGH

## Executive Summary

Racing Point eSports v24.0 replaces a hub-and-spoke polling architecture (30s WebSocket health probes) with an event-driven mesh. The closest industry analogues are industrial IoT fleets and smart building management systems — not consumer esports platforms — and experts in those domains converge on the same pattern: a single lightweight message broker (NATS) as event backbone, an SQLite outbox for degraded-mode durability on edge nodes, and a late-fusion approach for correlating heterogeneous sensors. NATS with JetStream is the right choice for this venue: single binary, Windows-native service, built-in persistence, and leaf-node support for cloud bridging via Tailscale. The 8-pod scale does not warrant Kafka, a NATS cluster, or any additional broker technology.

The recommended build order is strict: NATS server and subject taxonomy must exist before any pod client is written; the event envelope standard (including a `version` field from day one) must be committed to shared-types before any publisher is deployed; and the existing WebSocket health path must run in parallel with NATS throughout the migration to avoid breaking the admin dashboard and control room. The migration is additive by design — no big-bang cutover. The final architecture delivers four capabilities that the current system cannot provide: real-time lateral pod awareness, sensor fusion with camera detection, event-driven AI agent coordination with conflict prevention, and predictive risk scoring with degraded-mode resilience.

The two dominant risks are Windows-specific: (1) NATS JetStream silently falling back to in-memory mode if the store directory is not configured with correct permissions and forward-slash paths, and (2) mDNS multicast breaking on pods after any network interface state change including Tailscale reconnection. Both are well-documented and preventable with specific mitigation steps that must be treated as hard pre-requisites rather than post-hoc configuration. A third structural risk is the AI agent runaway loop — without the blackboard + intent controller pattern in place before any agent subscribes to the mesh, two agents can cascade conflicting actions across all 8 pods simultaneously.

## Key Findings

### Recommended Stack

The stack additions are targeted and minimal. NATS 2.12.x (single binary, Windows Service via `sc.exe`) provides the event backbone and JetStream persistence. `async-nats` 0.46 is the only Rust client to use — the synchronous `nats` crate is deprecated. On the ML side, `linfa` 0.8.1 provides classical ML (regression, classification, anomaly) with zero Python runtime dependency, suitable for rule-based risk scoring at v24.0 scale; `ort` 2.0-rc.12 with DirectML provides GPU-accelerated ONNX inference on the RTX 4070 for camera person-detection. The `mdns-sd` + `swarm-discovery` combination handles peer discovery at minimal dependency cost versus `libp2p`.

**Core technologies:**
- NATS Server 2.12.x: event backbone + JetStream persistence — single binary, Windows Service, no JVM, 3-model council consensus. Note: use single-node (not clustered); v2.12.6 has a regression in clustered JetStream consumer updates only.
- async-nats 0.46: Rust async NATS client for rc-agent/rc-sentry — official crate, full JetStream API, tokio-native. MSRV ~1.79; venue's 1.93.1 exceeds this.
- natscli: stream/consumer management, ops debugging — essential for deployment and ongoing operations.
- mdns-sd 0.13.11 + swarm-discovery 0.4.1: pod peer discovery — pure Rust, bandwidth-bounded, treats Windows mDNS as best-effort with static IP fallback.
- ort 2.0-rc.12 (DirectML): ONNX inference for camera AI — wraps ORT 1.24, GPU on RTX 4070, no CUDA runtime required.
- tract-onnx 0.22: CPU-only ONNX fallback for fusion confidence scoring — pure Rust, zero dynamic dependencies.
- linfa 0.8.1: classical ML for pod risk scoring and demand forecasting — Rust-native, no Python, inference-only at v24.0.
- dashmap 6.x: concurrent hashmap for AI agent blackboard state — needed for multi-handler writes from NATS subscribers.

**Do not use:** synchronous `nats` crate (deprecated), `onnxruntime-ng` (unmaintained), Redis for event spool (adds a service to 8 Windows pods), Python inference on pods (ops nightmare), Docker on pods (VT-x issues documented in project history).

Full version matrix and Cargo.toml additions: `.planning/research/STACK.md`

### Expected Features

v24.0 introduces net-new capabilities only. All existing features (fleet health monitoring, billing, camera streaming, relay exec, feature flags, OTA, admin dashboard, comms-link) are pre-built and out of scope.

**Must have (table stakes — v24.0 core):**
- NATS server + JetStream running on .23, subject taxonomy defined — prerequisite for everything; nothing else is buildable without this.
- Event schema / envelope standard with `version: u32` in shared-types — must be locked before any publisher is deployed; changing it post-deployment requires coordinated 8-pod redeploy.
- Rust `async-nats` client integrated in rc-agent + rc-sentry — pods must be event publishers before any consumer can deliver value.
- Pod heartbeat publication (`pod.<id>.heartbeat` every 5s) — minimum lateral awareness primitive replacing the 30s polling model.
- SQLite event spool (`event_spool` table) + replay on reconnect with rate limiting — degraded-mode durability is non-negotiable for a venue that cannot afford data loss.
- Tiered degradation (Tier 0 racing never depends on NATS) — racing core must never block on network infrastructure; publish-and-forget with local spool fallback.
- Feature flag `MESH_ENABLED` for parallel-write transition — dual-write must be in place before any polling path is retired.

**Should have (add after core is validated — v24.x):**
- mDNS pod self-registration + presence inventory — zero-config discovery; add when IP churn is observed in practice.
- AI agent event-driven subscriptions (James + Bono subscribe to mesh) — upgrades from watchdog-triggered to event-triggered; add once event volume is understood.
- Blackboard shared state + intent-based commands + intent controller service — prevents two agents issuing conflicting commands; add before any agent exec authority is granted.
- Camera NATS bridge (go2rtc sidecar publishing `camera.<id>.detection`) — thin Node.js bridge; prerequisite for fusion.
- Fusion service with time-windowed join and confidence scoring — camera + pod late fusion; requires camera bridge and 5s join window.
- Stream processor for pod risk scoring (rule-based thresholds first) — add once 7 days of health event history exists.

**Defer to v25+:**
- Graph-based agent workflows (branching, resumable remediation paths).
- ML-based anomaly detection and demand forecasting (needs 60+ days of event history baseline).
- Cloud-side event mirror to Bono VPS via NATS leaf node (high ops complexity; add only if real-time cloud analytics are required).
- Leader election for server-down pod coordination (only if pods need collective decision-making; validate use case first).
- Online ML model training and per-session outcome prediction.

Full prioritization matrix and dependency graph: `.planning/research/FEATURES.md`

### Architecture Approach

The architecture is a single NATS server on the central server (.23) as a Windows service, with all 8 pods and the POS PC connecting as NATS clients (TCP :4222). A JetStream leaf node on Bono VPS bridges the LAN cluster to cloud over Tailscale without requiring inbound ports. Two new NATS KV buckets serve as the digital twin (`KV_PODS` — per-pod state snapshot, 120s TTL) and the AI blackboard (`KV_BLACKBOARD` — agent coordination surface). A new Fusion Service (Node.js on .23) performs 5-second sliding window joins of pod health and camera detection events, publishing fused context to `fusion.pod-camera.<id>`. All pod racing continues without NATS dependency (Tier 0); billing events spool to SQLite and replay on reconnect (Tier 1); analytics and AI actions halt gracefully (Tier 2).

**Major components:**
1. NATS Server (.23) — event backbone, JetStream streams (PODS/CAMERAS/POS/AI/FUSION/VENUE/AUDIT), Windows Service
2. rc-agent modifications (pods) — add `async-nats` publish, SQLite event spool, mdns-sd peer discovery
3. rc-sentry modifications (pods) — add `async-nats` alarm publisher
4. racecontrol modifications (.23) — add NATS subscriber, update PostgreSQL from events, expose intent approval endpoint
5. Fusion Service (Node.js, .23) — time-windowed join of pod + camera events, confidence scoring, publish to `fusion.*`
6. Intent Controller (Node.js, .23 or standalone) — arbiter for AI agent intents, deduplication, conflict detection, blackboard management
7. NATS Leaf Node (Bono VPS) — bridges LAN NATS to cloud over Tailscale; cloud dashboards and Bono agent subscribe transparently
8. Camera NATS bridge (Node.js sidecar, .23) — reads go2rtc HTTP event stream, publishes `camera.<id>.detection`

**Subject taxonomy root:** `pod.<id>.*`, `camera.<id>.*`, `pos.*`, `ai.*`, `fusion.*`, `venue.*`, `audit.*`

Full topology diagram, subject hierarchy, JetStream stream bindings, and build order: `.planning/research/ARCHITECTURE.md`

### Critical Pitfalls

The pitfall research identified 13 documented pitfalls. The five that will kill a phase if missed:

1. **NATS JetStream silently falls back to in-memory mode** — If `store_dir` uses backslashes or the service account lacks write permission, JetStream disables silently with no client error. Use forward-slash paths (`C:/nats/jetstream`), set explicit NTFS ACL for the service account, and verify with `nats server info` showing `jetstream: true` before writing any application code. Also add Windows Defender exclusion on the JetStream store directory — AV real-time scanning causes severe write amplification.

2. **mDNS multicast breaks after Tailscale reconnection** — Tailscale tunnel reconnections silently invalidate Windows multicast group membership on the LAN interface. mDNS service continues running but peer list drops to 0 with no error. Mitigation: treat mDNS as best-effort; maintain a static IP fallback table (192.168.31.x); implement a health watchdog that detects 0 peers for 60s and triggers interface rebind (not full rc-agent restart).

3. **AI agent runaway loop via cascading reactive triggers** — An agent observes a pod health event, triggers a remediation, the remediation generates a new health event, the agent triggers again. With 8 pods publishing at 5s intervals, a misconfigured handler can cascade across the fleet. Prevention is architectural: all agent actions must go through the intent controller (never direct exec); add per-pod per-action cooldown (max 1 restart per pod per 5 minutes); circuit breaker after 3 actions in 60s escalates to Uday via WhatsApp.

4. **Event schema with no version field breaks replay** — Adding a `version` field after events are in production requires migrating every event in the JetStream stream. There is no clean path. Every event struct must have `version: u32 = 1` from the very first commit, with `#[serde(default)]` on new fields for backward compatibility. This is not negotiable.

5. **Dual write non-atomicity / outbox pattern required** — SQLite write + NATS publish are not atomic. If NATS is unreachable, the database updates but no event is emitted; downstream consumers never learn of the change. Use the transactional outbox: write state change AND pending event to SQLite in a single ACID transaction; a relay goroutine publishes and marks as sent only after NATS confirms. This is the same `event_spool` table already in scope — the outbox and the spool are the same mechanism.

Additional pitfalls to address per phase: polling consumer breakage during migration (strangler-fig dual-write), JetStream infinite redelivery (set `MaxDeliver: 3`, ack on receipt not completion), sensor fusion clock skew (NTP sync prerequisite, 5s conservative join window), split-brain on reconnection (document conflict resolution policy before coding), replay storm (rate-limit spool replay at 10 events/s per pod). Full details and verification checklists: `.planning/research/PITFALLS.md`

## Implications for Roadmap

Research establishes a hard dependency chain: NATS server before any client, event schema before any publisher, parallel write before any polling retirement, camera bridge before fusion, 7 days of event history before rule-based risk scoring. These constraints produce 6 natural phases with clear entry gates.

### Phase 1: Event Backbone
**Rationale:** NATS server + subject taxonomy + event envelope standard are the foundational prerequisite for every other feature. Nothing else can be built without the broker running and the schema locked.
**Delivers:** NATS 2.12.x as Windows Service on .23 with JetStream enabled; 7 JetStream streams defined (PODS/CAMERAS/POS/AI/FUSION/VENUE/AUDIT); event envelope standard with `version: u32` committed to shared-types; natscli operational; startup health check verifying `jetstream: true` and store directory writable.
**Features addressed:** NATS + JetStream backbone, event schema / envelope standard, subject-based access control, JetStream stream retention limits.
**Pitfalls to prevent:** JetStream store_dir permissions and path format (must verify before proceeding), outbox pattern design locked before any publisher is written, NATS Windows service registration conflict (`NATS_DOCKERIZED` or standalone service), MaxDeliver and AckWait consumer standards established.
**Research flag:** Standard — NATS official docs are high-quality; Windows service installation is well-documented. Verify JetStream persistence by restarting the server and confirming stream data survives.

### Phase 2: Pod Event Publishers (Parallel Write)
**Rationale:** Once the broker exists, pods must become event publishers. The dual-write / parallel approach keeps existing WebSocket health probes running. No existing consumer is broken. This phase validates that all 8 pods can reach NATS and that the event volume is as expected before any consumer is built.
**Delivers:** `async-nats` client in rc-agent (heartbeat, health, session start/end, game launch/crash); `async-nats` client in rc-sentry (preflight alarm, recovery events); `MESH_ENABLED` feature flag controls NATS publish path; outbox spool table (`event_spool`) created in pod SQLite DB; existing WebSocket health probe continues unchanged.
**Features addressed:** Rust async-nats on pods, pod heartbeat publication (5s), SQLite event spool (write path only — replay deferred to Phase 4), Tiered degradation (Tier 0 protection by design: no session gate on NATS ack).
**Pitfalls to prevent:** Dual write atomicity — outbox pattern must be implemented here, not as a retrofit; `async-nats` tokio integration (never use synchronous `nats` crate); Tier 0 never blocks on NATS.
**Research flag:** Standard — async-nats API is well-documented with JetStream examples. Integration test: kill NATS mid-session, verify racing continues and outbox accumulates.

### Phase 3: Server-Side NATS Subscriber + Digital Twin
**Rationale:** Once pods publish, racecontrol must consume. This phase closes the loop between mesh events and the existing PostgreSQL fleet state, and creates the NATS KV digital twin that the fusion service and dashboard will read. The leaf node to Bono VPS is low-effort config added here.
**Delivers:** racecontrol NATS subscriber updating PostgreSQL pod table from `pod.>.health` events; NATS KV bucket `KV_PODS` updated on each health event (TTL 120s, 5-revision history); NATS leaf node configured on Bono VPS (dials out to .23 via Tailscale); Admin dashboard KV Watch replacing 30s polling for fleet grid (after 1 week of stable operation).
**Features addressed:** Server-side event aggregation, digital twin pattern, cloud dashboard real-time updates, existing WS hub runs parallel until KV Watch is validated.
**Pitfalls to prevent:** Polling consumer migration (dual-write must be running for 1 week before polling path is retired; admin dashboard must be explicitly tested); PostgreSQL write batching at 500ms intervals (not every NATS event).
**Research flag:** Standard — NATS KV Watch API is well-documented. The racecontrol code change is medium-effort but within existing Rust patterns.

### Phase 4: Degraded Mode (SQLite Spool + mDNS Discovery)
**Rationale:** With basic mesh operational, degrade gracefully must be proven before any subsequent phase increases system complexity. This phase adds replay on reconnect and mDNS-based peer discovery as fallback when NATS is unreachable.
**Delivers:** SQLite event spool replay on NATS reconnect with rate limiting (10 events/s per pod); replay prioritizes Tier 0 (billing) before Tier 1 (management); mDNS pod self-registration and presence inventory in rc-agent using `swarm-discovery`; static IP fallback table for mDNS failure; degraded mode banner in control room UI; pod publishes `pod.<id>.spool.flush` event on reconnect.
**Features addressed:** Event replay on reconnect, mDNS pod self-registration, pod presence inventory, graceful partial mesh (heartbeats via mDNS when NATS down), degraded mode visibility for staff.
**Pitfalls to prevent:** mDNS blocked by Windows Firewall (audit script extension to verify UDP 5353 inbound rule on all pods, network profile set to Private); mDNS multicast instability after Tailscale reconnection (health watchdog, static IP fallback); replay storm (rate limiting is mandatory — chaos test with 500+ events spooled); split-brain conflict resolution policy must be documented before coding.
**Research flag:** Needs care — mDNS on Windows is medium-confidence territory with documented intermittent issues. Run chaos tests (hard-kill server, Tailscale reconnect on pods) before declaring phase done.

### Phase 5: Camera Fusion + Predictive Risk Scoring
**Rationale:** Once the event backbone is stable and 7+ days of pod health history exists in JetStream, the higher-value analytical features become buildable. Camera fusion requires a detection bridge (go2rtc does not emit detection events natively). Risk scoring starts rule-based and promotes to ML only after 60 days of data.
**Delivers:** Camera NATS bridge (thin Node.js sidecar reading go2rtc HTTP event stream, publishing `camera.<id>.detection`); Fusion Service (Node.js, 5s sliding window join, confidence scoring, publishes `fusion.pod-camera.<id>` and `fusion.venue.occupancy`); NTP sync verification across all pods and cameras as prerequisite gate; pod risk scoring (rule-based thresholds: CPU >90%, memory >85%, disk <10% free = risk HIGH); risk events published to `fusion.risk.<pod-id>`; predictive alert displayed in control room with top 2 contributing factors.
**Features addressed:** Camera detection events, time-windowed fusion join, confidence scoring on fused events, occupancy vs session cross-check (billing anomaly detection), pod failure risk scoring, demand forecasting (rolling average + day-of-week, no ML at v24.0).
**Pitfalls to prevent:** Sensor fusion clock skew (NTP prerequisite gate — all sources within 500ms before fusion is enabled); fusion join window conservative (5s minimum, not 500ms); late-arriving camera events handled via sorted merge queue; predictive cold start (rule-based only, no ML, output "INSUFFICIENT DATA" for first 30 days).
**Research flag:** Needs deeper research during planning — time-windowed join implementation patterns, go2rtc HTTP event stream API, camera detection shim approach (snapshot vs NVR webhook). Camera detection bridge design is the biggest unknown.

### Phase 6: AI Agent Mesh Coordination
**Rationale:** Agent coordination is the highest-risk phase architecturally. The blackboard + intent controller must be in place before James or Bono are given any NATS-driven exec authority. The runaway loop prevention is the primary safety mechanism.
**Delivers:** James and Bono subscribe to `venue.>` on mesh (read-only first); NATS KV `KV_BLACKBOARD` shared state; intent-based command publishing (`ai.james.intent`, `ai.bono.intent`); Intent Controller service (deduplication, conflict detection, operational constraint checks, per-pod per-action cooldown); `ai.command.approved` / `ai.command.rejected` events; agent acknowledgment events on action completion; circuit breaker (3 actions/pod/60s → WhatsApp alert to Uday); James can veto Bono intents for on-site physical actions.
**Features addressed:** Event-driven agent trigger, blackboard shared state, intent-based command proposals, command controller service, agent acknowledgment, intent veto for on-site priority.
**Pitfalls to prevent:** AI agent runaway loop (blackboard + cooldown + circuit breaker required before agents have exec authority); dual agent conflict (intent controller is the single exec gate; agents never publish directly to action subjects); idempotency keys on all agent-dispatched actions; `MaxDeliver: 3` on all AI event consumers.
**Research flag:** Standard pattern (blackboard + orchestrator-worker is well-documented in multi-agent literature) but implementation complexity is high. Must include end-to-end runaway loop test before shipping.

### Phase Ordering Rationale

- Phases 1-2 are strictly sequential: broker before client, schema before publisher.
- Phase 3 depends on Phase 2 (needs pod events to consume) but leaf node config is low-risk and can be done anytime after Phase 1.
- Phase 4 (degraded mode) is deliberately placed before camera fusion: a system that loses events silently should not be extended with new features.
- Phase 5 (fusion + analytics) requires both Phase 3 (KV twin for pod context) and Phase 2 (7 days of event history) before meaningful fusion or risk scoring is possible.
- Phase 6 (AI coordination) is last because it requires the full mesh to be stable — agents consuming from an unstable event stream will produce unreliable reasoning.
- The parallel-write (existing WS + NATS) pattern runs from Phase 2 through at least the end of Phase 3, decommissioned only after 1 week of stable dual operation.

### Research Flags

Phases needing deeper research during planning:
- **Phase 4 (Degraded Mode):** mDNS on Windows 11 is medium-confidence; intermittent multicast issues are documented but root cause is inconsistent. Plan for static IP fallback as default path, mDNS as enhancement.
- **Phase 5 (Camera Fusion):** go2rtc HTTP event stream API and the detection shim approach (snapshot-based vs NVR webhook) need concrete research before implementation plans are written. Camera AI model format (ONNX vs custom) and inference latency budget also need validation.

Phases with standard patterns (skip research-phase):
- **Phase 1 (Event Backbone):** NATS JetStream is extensively documented with official patterns, Windows Service installation is confirmed, no novel territory.
- **Phase 2 (Pod Publishers):** async-nats + tokio integration follows established patterns; SQLite outbox is well-documented.
- **Phase 3 (Server Subscriber + Digital Twin):** NATS KV Watch API is official and documented; racecontrol modification follows existing Rust patterns.
- **Phase 6 (AI Coordination):** Blackboard + orchestrator-worker pattern is well-documented in multi-agent literature; risk is implementation discipline, not pattern novelty.

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | MEDIUM-HIGH | NATS 2.12.x, async-nats 0.46, linfa 0.8.1 verified on crates.io with recent release dates. ort 2.0-rc.12 is RC status — production-used but carries minor API-change risk before stable release. swarm-discovery 0.4.1 is low-traffic crate by credible author (Roland Kuhn / Akka). |
| Features | HIGH | Event bus, degraded mode, and discovery features are based on official NATS docs and established IoT patterns with high source confidence. Camera fusion and predictive layer are MEDIUM — analogues exist but venue-specific implementation details need validation. |
| Architecture | HIGH | Single-server NATS with leaf node is the official recommended topology for edge deployments; subject taxonomy follows official NATS naming conventions; JetStream stream bindings follow anti-pattern guidance from Synadia. Digital twin KV pattern is official. |
| Pitfalls | HIGH | Critical pitfalls verified against official NATS docs, GitHub issues (NATS server #1113, JetStream redelivery #4627), Jepsen analysis, and project-specific history (rc-sentry restart bug, pod healer flicker, SSH config corruption — same failure classes). |

**Overall confidence:** MEDIUM-HIGH

### Gaps to Address

- **go2rtc detection event API:** go2rtc streams video via RTSP/WebRTC but does not natively emit detection events. The camera bridge in Phase 5 must either poll snapshots and run inference, or leverage NVR motion webhook capabilities. The exact integration path needs validation against the specific NVR hardware in use at the venue. Recommend scoping this explicitly in Phase 5 planning.
- **Windows Defender exclusion verification:** JetStream store directory must be excluded from real-time AV scanning. This is a manual configuration step on the server that must be part of the Phase 1 deploy checklist — it cannot be automated via code.
- **NTP enforcement on cameras and pods:** The NVR and 13 cameras may have free-running clocks. Before Phase 5, a concrete NTP sync verification step across all 14 devices (8 pods + 1 POS + 1 server + NVR + cameras) must be added to the fleet audit script (v23.0 audit framework).
- **swarm-discovery 0.4.1 Windows validation:** The crate tests against Avahi/macOS/iOS, not Windows explicitly. Confidence is MEDIUM. A local integration test (2 pods on the same subnet) should be the first deliverable of Phase 4 before any further mDNS work proceeds.
- **NATS v2.12.6 regression scope:** The documented regression in clustered JetStream consumer updates does not affect single-node deployments (which is the plan). This gap is already mitigated by the single-node decision. Verify by checking NATS release notes before upgrading to any patch version beyond 2.12.6.

## Sources

### Primary (HIGH confidence)
- https://docs.nats.io/nats-concepts/jetstream — JetStream official documentation
- https://docs.nats.io/running-a-nats-service/introduction/windows_srv — NATS Windows Service install
- https://docs.nats.io/nats-concepts/subjects — Subject naming conventions
- https://docs.nats.io/nats-concepts/jetstream/key-value-store — KV Store API
- https://docs.rs/async-nats/0.46.0/async_nats/ — async-nats Rust API
- https://www.synadia.com/blog/jetstream-design-patterns-for-scale — JetStream anti-patterns (Synadia / NATS vendor)
- https://docs.nats.io/nats-concepts/service_infrastructure/adaptive_edge_deployment — Leaf node topology
- https://crates.io/crates/async-nats — v0.46.0 confirmed
- https://crates.io/crates/linfa — v0.8.1 confirmed
- https://docs.aws.amazon.com/prescriptive-guidance/latest/cloud-design-patterns/transactional-outbox.html — Outbox pattern
- https://docs.aws.amazon.com/prescriptive-guidance/latest/cloud-design-patterns/strangler-fig.html — Strangler-fig migration pattern
- https://jepsen.io/analyses/nats-2.12.1 — Jepsen analysis (data loss risk under corruption)

### Secondary (MEDIUM confidence)
- https://github.com/nats-io/nats-server/issues/1113 — NATS Windows service registration conflict
- https://github.com/nats-io/nats-server/issues/4627 — JetStream infinite redelivery
- https://crates.io/crates/swarm-discovery — v0.4.1 (by Roland Kuhn, Akka author)
- https://crates.io/crates/mdns-sd — v0.13.11, Windows supported
- https://github.com/pykeio/ort — ort 2.0-rc.12 DirectML backend confirmed
- https://www.confluent.io/blog/event-driven-multi-agent-systems/ — Blackboard, orchestrator-worker agent patterns
- https://www.confluent.io/blog/dual-write-problem/ — Dual write atomicity
- https://learn.microsoft.com/en-us/azure/iot-edge/offline-capabilities — IoT Edge offline spool reference
- Project-specific: `project_rcsentry_restart_bug.md`, `project_pod_healer_flicker.md`, `feedback_ssh_config_corruption.md` — same failure class as NATS Windows service and mDNS pitfalls

### Tertiary (LOW confidence / needs validation)
- go2rtc HTTP event stream API — not directly verified; needs validation against venue NVR hardware in Phase 5 planning
- swarm-discovery Windows 11 behavior — crate tests on macOS/Linux; Windows LAN behavior inferred from mdns-sd base library

---
*Research completed: 2026-03-27*
*Ready for roadmap: yes*
