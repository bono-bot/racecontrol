# Feature Research

**Domain:** Event-driven mesh intelligence platform — sim racing venue (IoT fleet, distributed edge, AI agent coordination)
**Researched:** 2026-03-27
**Confidence:** HIGH (event bus, degraded mode, discovery) | MEDIUM (camera fusion, predictive layer, AI coordination)

---

## Context: What Already Exists

The following are BUILT and must NOT appear in the meshed intelligence roadmap as new features:

- Fleet health monitoring (polling, 30s interval)
- Pod rc-agent + rc-sentry lifecycle management
- Camera streaming via go2rtc (13 cameras)
- Relay exec + chain orchestration
- Feature flags + OTA pipeline
- Admin dashboard (fleet, billing, drivers, control room)
- Comms-link between James and Bono

The features below are exclusively what is NEW in v24.0 Meshed Intelligence.

---

## Feature Landscape

### Event Bus Features

#### Table Stakes

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| NATS server running on LAN | Any event mesh needs a broker; NATS is the decided backbone | LOW | Single binary, Windows-native, no JVM. Already decided in PROJECT.md. |
| JetStream streams per domain | Durable, replayable event history is required for reconnect sync and audit | MEDIUM | One stream per subject hierarchy: `venue.pods.*`, `venue.cameras.*`, `venue.agents.*` |
| Hierarchical subject taxonomy | Without well-defined subjects, consumers can't selectively subscribe | LOW | Design upfront; changing subjects later breaks all consumers |
| At-least-once delivery guarantee | Pod events must not be silently lost under LAN congestion or transient drops | LOW | JetStream default; configure ack policy per consumer |
| Rust `async-nats` client on pods | All 8 pods run Rust rc-agent; must publish events natively | MEDIUM | `async-nats` 0.x is the only mature Rust client; verify current API before coding |
| Node.js NATS client for web services | Admin dashboard and fusion service are Node.js; need same bus | LOW | `nats.js` 2.x — well maintained, same JetStream API surface |
| Event schema / envelope standard | All publishers must agree on envelope (type, source, timestamp, payload) | LOW | Define once in shared-types; violation = consumer breakage |
| Subject-based access control (auth) | Pods should not be able to publish to agent-reserved subjects | MEDIUM | NATS has native subject permissions; configure per-client credentials |

#### Differentiators

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| Event sourcing for pod lifecycle | Full audit trail of every state transition; enables replay-based root cause analysis | HIGH | Requires append-only stream + projection layer; high value for ops post-mortems |
| Republish fan-out for admin dashboard | Live dashboard gets events without polling; admin UI becomes truly real-time | MEDIUM | NATS JetStream `republish` config — push to WebSocket bridge subject |
| Wildcard consumer for James/Bono | AI agents subscribe to `venue.>` and build situational awareness from all events | MEDIUM | Single subscription for entire venue; agents must handle high-volume safely |

#### Anti-Features

| Feature | Why Requested | Why Problematic | Alternative |
|---------|---------------|-----------------|-------------|
| Per-pod NATS server (full mesh) | "True mesh — no central broker" | Adds peer replication complexity; NATS cluster is overkill for 8 pods on LAN; the value of NATS is the centralized broker with JetStream | Single NATS server on .23 with SQLite spool for offline; peer mesh is the overlay, not the broker |
| Kafka instead of NATS | "Kafka is proven at scale" | Kafka requires JVM, ZooKeeper/KRaft, 10x ops overhead; 8 pods does not need Kafka throughput | NATS JetStream — decided and validated by 3-model council |
| Real-time schema evolution / Avro | "Type-safe schemas" | Schema registry adds infrastructure; JSON envelope with versioned `type` field is sufficient for 8-pod venue | Version field in envelope + additive-only schema changes |

---

### Discovery Features

#### Table Stakes

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| mDNS/DNS-SD pod self-registration | Pods must announce themselves without static config; IPs change after hardware swaps | MEDIUM | Windows supports mDNS natively via `dns-sd`; Rust crate `mdns-sd` 0.x available |
| Pod-to-pod heartbeat publication | Each pod publishes `venue.pods.{id}.heartbeat` on NATS; others can consume without asking the server | LOW | 5s interval is sufficient; 30s is polling, not mesh |
| Pod presence inventory | Each pod knows the full set of live pods from heartbeat subscriptions; no server query needed | MEDIUM | In-memory map in rc-agent; evict after 3 missed heartbeats (15s) |
| Heartbeat payload includes local state | Heartbeat carries: session_active, cpu, memory, disk, pod_id — enough for lateral awareness | LOW | Expand existing health struct; no new schema needed |

#### Differentiators

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| Leader election (mDNS-based) when server down | A pod takes on coordination role during server outage; venue does not freeze | HIGH | Bully algorithm or RAFT subset over mDNS; only meaningful if pods need to make collective decisions offline |
| Pod-announced capability flags | Pods publish what they support (AC version, GPU model, features enabled) as part of discovery | LOW | Enables intelligent session routing (e.g., steer VR session to pods with RTX) |

#### Anti-Features

| Feature | Why Requested | Why Problematic | Alternative |
|---------|---------------|-----------------|-------------|
| DNS-based static service registry | "Simpler than mDNS" | Requires central DNS server; defeats zero-config goal; fails if server is down | mDNS on LAN — self-healing, no single point of failure |
| Consul or etcd for service discovery | "Production-grade discovery" | Adds infrastructure; overkill for 8 fixed nodes on one LAN | mDNS + NATS presence subjects — sufficient for venue scale |

---

### Fusion Features

#### Table Stakes

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| Camera detection events on event bus | Camera AI detections (occupancy, person present, motion) must be first-class events | MEDIUM | go2rtc already streams; need detection layer publishing to `venue.cameras.{id}.detection` |
| Time-windowed join (camera + pod) | Correlate camera occupancy event with pod session event within a time window | HIGH | Most complex feature. 5-10s window. Node.js stream processor consuming both subjects. |
| Confidence score on fused events | Fused events must carry confidence so consumers can decide whether to act | MEDIUM | Simple weighted average of contributing sensor confidences; degrade gracefully if one source missing |
| Late fusion model | If camera data is missing, fused event still publishes — just with lower confidence | LOW | By design: fuse only what is available at window close; do not block on all sensors |

#### Differentiators

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| Occupancy vs session cross-check | Camera sees person at pod but no active session → alert for unauthorized use or unreported session | HIGH | Directly catches revenue leakage; requires pod session state in fusion join |
| Camera anomaly detection (idle during peak) | Camera fusion detects pods with no occupancy during peak hours → proactive utilization nudge | HIGH | Requires demand baseline; build after predictive layer is established |
| Ambient headcount aggregation | Count venue occupancy from multiple cameras → feed demand forecasting | MEDIUM | Privacy-safe: no identification, only count; useful for capacity planning |

#### Anti-Features

| Feature | Why Requested | Why Problematic | Alternative |
|---------|---------------|-----------------|-------------|
| Face recognition / identity at cameras | "Know exactly who is at which pod" | Privacy violation; legal exposure; outside scope of venue ops | Session-linked pod state is sufficient for ops purposes |
| Early fusion (merge at sensor level) | "More accurate fused model" | Tight coupling between camera and pod event schemas; one source failure halts all fusion | Late fusion — independent pipelines, merge at decision point |
| Real-time video stream processing | "Process every frame" | Compute cost is enormous; go2rtc is streaming-only, no frame pipeline | Detection events only (e.g., from go2rtc webhooks or periodic snapshot analysis) |

---

### AI Agent Coordination Features

#### Table Stakes

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| Event-driven agent trigger (not cron/watchdog) | James and Bono must react to events, not poll; this is the core shift from v23 | MEDIUM | Both subscribe to `venue.agents.inbox.{james|bono}` + wildcard alert subjects |
| Blackboard shared state on event bus | Agents post observations and conclusions to a shared subject; both can read and build on each other's work | MEDIUM | `venue.agents.blackboard` subject in JetStream; durable so both agents see history |
| Intent-based command proposals | Agent proposes action on `venue.agents.intent`; controller service validates and executes — agents never directly exec | HIGH | Prevents conflicts between James and Bono acting on same pod simultaneously |
| Command controller service | Arbitrates intents: deduplicates, checks preconditions, dispatches to relay exec | HIGH | New service (Node.js acceptable); the only entity with exec authority |
| Agent acknowledgment events | After action taken, agent publishes result to blackboard; other agent + dashboard can see outcome | LOW | Close the loop; enables audit and conflict detection |

#### Differentiators

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| Graph-based agent workflows | Agents can model multi-step remediation as a directed graph; partial completion is resumable | HIGH | Significant uplift over current linear chain orchestration; enables branching remediation paths |
| Intent prioritization and veto | James (on-site, lower latency) can veto a Bono intent for on-site physical actions | MEDIUM | Prevents remote agent overriding on-site agent during physical intervention |
| Agent situational summary events | Every N minutes (or on demand), an agent publishes a venue state summary to the blackboard | LOW | Cheap to implement; makes dashboard "explain what happened" trivially possible |

#### Anti-Features

| Feature | Why Requested | Why Problematic | Alternative |
|---------|---------------|-----------------|-------------|
| Agents with direct pod exec (no controller) | "Faster response — remove indirection" | Two agents can race on same pod; no audit trail; v18.0 relay built for this reason | Intent → controller → relay exec pipeline; latency is <500ms, acceptable |
| LLM inference on-pod | "Edge AI on every pod" | RTX 4070 is gaming GPU, not inference server; thermal risk during racing; 8-pod inference fleet adds complexity | LLM inference on James (on-site server) or Bono (VPS); pods send events only |
| Autonomous agent self-update | "Agents update their own code via OTA" | Risk of update loop or bad update during live sessions; v22.0 OTA pipeline requires human approval gate | Keep OTA approval-gated; agents can propose OTA via intent, but human approves |

---

### Predictive Analytics Features

#### Table Stakes

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| Stream processor consuming mesh events | Predictions require a continuous view of events; a subscriber that maintains rolling state is required | MEDIUM | Node.js or Python stream processor subscribing to `venue.pods.*.health` and `venue.cameras.*.detection` |
| Pod failure risk scoring | From health event stream (cpu, memory, disk, session_active), derive a per-pod risk score | MEDIUM | Rule-based threshold scoring first; ML model upgrade in v25+ |
| Demand forecasting (hourly) | From historical session events + time of day, predict next-hour demand | MEDIUM | Rolling average + day-of-week seasonality; no deep ML required; valid for 8-pod scale |
| Risk events published to bus | Predictions must be first-class events: `venue.analytics.risk.{pod_id}`, `venue.analytics.demand` | LOW | Consumers (admin dashboard, agents) subscribe; no polling |

#### Differentiators

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| Maintenance window recommendation | From failure risk + demand forecast, suggest "Pod 3 safest to maintenance-lock at 14:00 Tuesday" | HIGH | Combines two predictive streams; high ops value; reduces downtime during peak |
| Anomaly detection on health baselines | Each pod has a learned baseline; deviation triggers risk event proactively (not threshold breach) | HIGH | Requires baseline accumulation phase; defer to v24.x after stream processor is stable |
| Churn/retention signal from session patterns | From session frequency per driver, predict churn risk and surface to admin dashboard | HIGH | Adjacent to existing HR psychology features (v14.0); powerful but out of core ops scope |

#### Anti-Features

| Feature | Why Requested | Why Problematic | Alternative |
|---------|---------------|-----------------|-------------|
| Online ML model training | "Self-improving predictions" | Requires GPU, training pipeline, model versioning; adds risk of drifting models in production | Offline model training (scheduled nightly batch); deploy via OTA pipeline |
| Real-time GPU telemetry prediction | "Predict GPU failure from sensor data" | GPU sensor access in Windows requires privileged NVML calls; adds Rust FFI complexity | CPU/memory/disk risk scoring is sufficient for v24; GPU can be added in v25 |
| Per-session outcome prediction | "Predict if this session will be good" | No validated outcome metric; prediction without ground truth is noise | Aggregate risk at pod level, not session level |

---

### Resilience Features

#### Table Stakes

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| SQLite event spool on each pod | When NATS is unreachable, events are written to local SQLite outbox with monotonic sequence | MEDIUM | Outbox pattern: table `(id, subject, payload, published_at, sent_at)`; `sent_at` is null until confirmed |
| Event replay on reconnect | On NATS reconnect, pod drains outbox in sequence; deduplication via idempotency key | MEDIUM | Idempotency key = `{pod_id}:{sequence_id}`; JetStream deduplication window handles duplicates |
| Tiered degradation (Tier 0 / 1 / 2) | Core racing must never stop; management and analytics are acceptable casualties | MEDIUM | Tier 0: racing (always runs), Tier 1: billing sync (degrades gracefully), Tier 2: analytics/predictions (drops) |
| Pod local session state machine | Each pod maintains its own session state independently of server; can start/stop sessions without connectivity | HIGH | Already partially implemented via rc-agent; needs formalization as explicit state machine with event publication |
| NATS reconnect with backoff | Pods must not flood NATS on reconnect; exponential backoff prevents thundering herd | LOW | `async-nats` reconnect options; configure max_reconnects and reconnect_delay |

#### Differentiators

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| Graceful partial mesh (server down, pods still coordinate) | Pods continue lateral heartbeats via mDNS even when NATS is unreachable | MEDIUM | Heartbeats switch from NATS to mDNS-only when broker unreachable; pods stay aware of each other |
| Degradation event published on recovery | On reconnect, pod publishes `venue.pods.{id}.recovery` with spool size and replay duration | LOW | Helps agents understand what happened during the gap; free to implement alongside replay |
| Cloud-side event mirror (Bono VPS) | JetStream stream mirrored to Bono VPS via Tailscale NATS leaf node | HIGH | Enables cloud-side analytics even during LAN partition; adds operational complexity |

#### Anti-Features

| Feature | Why Requested | Why Problematic | Alternative |
|---------|---------------|-----------------|-------------|
| Redis for event spool | "Redis is faster than SQLite" | Redis is another process on every pod; SQLite is already present; adding Redis for a write-ahead log on Windows is unnecessary complexity | SQLite outbox — zero new dependencies, append-only writes are fast enough |
| Full RAFT cluster across pods | "True distributed consensus" | 8 Windows gaming pods running RAFT adds network chattiness, split-brain risk, and Rust complexity; venue scale does not require this | NATS JetStream on server + mDNS fallback for discovery; RAFT is for the broker cluster, not the pods |
| Active-active NATS cluster | "High availability broker" | Two NATS servers on the same 8-pod LAN adds ops complexity without meaningful HA gain | Single NATS on .23 + pod SQLite spool; the spool is the HA mechanism |

---

## Feature Dependencies

```
[NATS Server + JetStream]
    └──required-by──> [Event Schema / Envelope Standard]
    └──required-by──> [Rust async-nats clients on pods]
                          └──required-by──> [Pod heartbeat publication]
                          └──required-by──> [SQLite event spool + replay]
    └──required-by──> [Node.js NATS client for web services]
                          └──required-by──> [Stream processor / predictive layer]
                          └──required-by──> [AI agent event subscriptions]
                          └──required-by──> [Command controller service]

[mDNS pod self-registration]
    └──required-by──> [Pod presence inventory]
    └──enhances──>    [Graceful partial mesh when NATS down]

[Camera detection events on bus]
    └──required-by──> [Time-windowed fusion join]
                          └──required-by──> [Confidence scoring]
                          └──required-by──> [Occupancy vs session cross-check]

[Pod heartbeat publication]
    └──enhances──>    [Time-windowed fusion join]   (pod state in join)
    └──enhances──>    [Stream processor / risk scoring]

[Blackboard shared state]
    └──required-by──> [Intent-based command proposals]
                          └──required-by──> [Command controller service]
                              └──required-by──> [Agent acknowledgment events]

[Stream processor / risk scoring]
    └──required-by──> [Demand forecasting]
    └──required-by──> [Maintenance window recommendation]

[SQLite event spool]
    └──required-by──> [Event replay on reconnect]
    └──required-by──> [Tiered degradation]

[Pod local session state machine]
    └──required-by──> [Tier 0: racing always runs]
    └──enhances──>    [Event replay on reconnect]
```

### Dependency Notes

- **NATS Server must be Phase 1:** Every other feature is downstream; nothing else can be built without the broker running and subject taxonomy defined.
- **Event schema / envelope standard must be designed before any publisher is coded:** Changing the envelope format after 8 Rust clients are deployed requires a coordinated multi-pod redeploy.
- **Camera detection events require go2rtc integration work:** go2rtc streams video but does not emit detection events; a detection shim (snapshot + vision model, or motion webhook from NVR) must be built before fusion is possible.
- **Command controller conflicts with direct relay exec:** The existing `relay/exec/run` endpoint must be funneled through the controller, not replaced — agents must be updated to emit intents rather than exec calls.
- **Predictive layer requires event history:** Risk scoring degrades to rules-only without 7+ days of health event history in JetStream; launch with rule-based scoring, promote to model-based later.
- **SQLite spool does not conflict with existing billing sync:** Billing sync uses a separate SQLite table; the event outbox is a new table in the same file — no schema collision.

---

## MVP Definition

### Launch With (v24.0 core — phases 1-3)

These form the irreducible spine. Without them, "meshed intelligence" is just a label on the existing system.

- [ ] NATS + JetStream running on server (.23), subject taxonomy defined — without the broker, nothing else works
- [ ] Event schema envelope standard committed to shared-types — prevents rework of all 8 pod clients
- [ ] Rust `async-nats` client integrated in rc-agent — pods must be event publishers before any other feature can consume their data
- [ ] Pod heartbeat publication (`venue.pods.{id}.heartbeat` every 5s) — the minimum lateral awareness primitive
- [ ] SQLite event spool + replay on reconnect — degraded mode is table stakes for a venue that cannot afford downtime
- [ ] Tiered degradation (Tier 0 protected) — racing never stops; this is the non-negotiable contract

### Add After Validation (v24.x — phases 4-5)

Add once core pub/sub is proven stable and all 8 pods are publishing events.

- [ ] mDNS pod self-registration + presence inventory — adds zero-config discovery; add when IP churn is observed in practice
- [ ] AI agent event-driven subscription (James + Bono subscribe to mesh) — upgrade from watchdog-triggered to event-triggered; add once event volume is understood
- [ ] Blackboard shared state + intent-based commands + command controller — agent coordination; add once both agents are consuming events reliably
- [ ] Stream processor for pod risk scoring (rule-based) — adds predictive value; add once 7 days of health event history exists

### Future Consideration (v25+)

Defer these until v24.0 core is stable and validated.

- [ ] Camera detection events + time-windowed fusion — requires detection shim work; high value but high complexity; validate event backbone first
- [ ] Graph-based agent workflows — significant uplift over chain orchestration; defer until agent mesh coordination is stable
- [ ] ML-based anomaly detection / demand forecasting — needs baseline data; build after rule-based scoring runs for 30+ days
- [ ] Cloud-side event mirror to Bono VPS (NATS leaf node) — adds ops complexity; only needed if Bono-side analytics are required in real-time
- [ ] Leader election for server-down coordination — only needed if pods need collective decision-making; validate whether use case actually occurs

---

## Feature Prioritization Matrix

| Feature | User Value | Implementation Cost | Priority |
|---------|------------|---------------------|----------|
| NATS + JetStream backbone | HIGH | LOW | P1 |
| Event schema / envelope standard | HIGH | LOW | P1 |
| Rust async-nats on pods | HIGH | MEDIUM | P1 |
| Pod heartbeat publication | HIGH | LOW | P1 |
| SQLite spool + replay | HIGH | MEDIUM | P1 |
| Tiered degradation (Tier 0) | HIGH | MEDIUM | P1 |
| mDNS pod discovery | MEDIUM | MEDIUM | P2 |
| AI agent event-driven trigger | HIGH | MEDIUM | P2 |
| Blackboard + intent commands | HIGH | HIGH | P2 |
| Command controller service | HIGH | HIGH | P2 |
| Pod risk scoring (rule-based) | MEDIUM | MEDIUM | P2 |
| Camera detection events | HIGH | HIGH | P2 |
| Time-windowed fusion join | HIGH | HIGH | P2 |
| Demand forecasting | MEDIUM | MEDIUM | P3 |
| Graph-based agent workflows | MEDIUM | HIGH | P3 |
| Cloud event mirror (leaf node) | LOW | HIGH | P3 |
| Leader election on server-down | LOW | HIGH | P3 |
| ML-based anomaly detection | MEDIUM | HIGH | P3 |

**Priority key:**
- P1: Must have for v24.0 launch
- P2: Should have; add in v24.x after core validated
- P3: Future; v25+

---

## Comparable Systems Analysis

The closest analogues to Racing Point meshed intelligence are industrial IoT fleets and smart building management systems, not consumer esports platforms (which are primarily matchmaking/streaming, not physical hardware operations).

| Feature | Industrial IoT (Azure IoT Edge pattern) | Smart Building (BACnet/Haystack) | Racing Point Approach |
|---------|----------------------------------------|----------------------------------|-----------------------|
| Event bus | MQTT/AMQP to IoT Hub | BACnet broadcasts on LAN | NATS JetStream — better replay + Rust support |
| Device discovery | Static registry + DPS provisioning | Static IP per controller | mDNS — zero-config for Windows pods |
| Degraded mode | EdgeHub local spool + replay | Manual fallback procedures | SQLite outbox — already present, no new deps |
| AI/automation | Azure functions on cloud | Rule-based BAS controllers | Local agents (James) + cloud (Bono) — lower latency for on-site ops |
| Sensor fusion | Azure Stream Analytics | BAS point grouping | Custom Node.js fusion service — avoids cloud cost for LAN data |
| Predictive analytics | Azure ML + digital twin | Reactive maintenance only | Rule-based first, ML deferred — right-sized for 8 pods |

---

## Sources

- [NATS JetStream documentation](https://docs.nats.io/nats-concepts/jetstream) — event streaming, persistence, republish patterns (HIGH confidence)
- [NATS at the Edge — InfoQ](https://www.infoq.com/presentations/nats/) — fleet management with NATS at low-powered devices (MEDIUM confidence)
- [Four Patterns for Event-Driven Multi-Agent Systems — Confluent](https://www.confluent.io/blog/event-driven-multi-agent-systems/) — blackboard, orchestrator-worker, hierarchical, market-based (HIGH confidence)
- [Intent Streams — Solace](https://solace.com/blog/unlocking-agentic-ai-evolving-intent-streams/) — intent-based agent coordination on event mesh (MEDIUM confidence)
- [Azure IoT Edge Offline Capabilities — Microsoft Learn](https://learn.microsoft.com/en-us/azure/iot-edge/offline-capabilities) — offline spool and replay reference pattern (HIGH confidence)
- [Offline-First SQLite Sync Queues](https://www.sqliteforum.com/p/building-offline-first-applications-4f4) — outbox pattern with idempotency keys (MEDIUM confidence)
- [mDNS for IoT Device Discovery — Engineering IoT](https://medium.com/engineering-iot/understanding-mdns-on-esp32-local-network-device-discovery-made-easy-9aab590f0eea) — mDNS zero-config discovery pattern (HIGH confidence)
- [Predictive Maintenance for Fleet — Volpis](https://volpis.com/blog/comprehensive-guide-to-predictive-fleet-maintenance/) — rule-based → ML progression, risk scoring features (MEDIUM confidence)
- [AI Agent Service Mesh Guide — Fast.io](https://fast.io/resources/ai-agent-service-mesh/) — agent mesh patterns, capability routing (MEDIUM confidence)
- [Distributed Data Mesh for Smart Communities — ScienceDirect](https://www.sciencedirect.com/science/article/pii/S1877050923006099) — event mesh for venue-like sensor networks (MEDIUM confidence, academic)

---

*Feature research for: Meshed Intelligence — event-driven mesh architecture for sim racing venue*
*Researched: 2026-03-27*
