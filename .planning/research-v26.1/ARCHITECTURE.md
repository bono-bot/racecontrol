# Architecture Research

**Domain:** Event-driven mesh for sim racing venue (IoT + edge agents + AI coordination)
**Researched:** 2026-03-27
**Confidence:** HIGH (NATS topology, Windows deployment, subject naming) / MEDIUM (fusion patterns, degraded mode specifics)

---

## Standard Architecture

### System Overview

```
+=====================================================================+
|                    CLOUD LAYER (Bono VPS)                          |
|  +---------------+  +----------------+  +--------------------+    |
|  | Next.js Admin | | Next.js Web PWA| |  Next.js Kiosk    |    |
|  |    :3201      | |    :3200       | |     :3300          |    |
|  +-------+-------+  +--------+-------+  +---------+----------+    |
|          |                   |                    |               |
|  +--------+-------------------+--------------------+----------+   |
|  |              Bono NATS Leaf Node (:4222 on VPS)            |   |
|  |        Bridges LAN-cluster → cloud via Tailscale           |   |
|  +-----------------------------+------------------------------+   |
+================================|====================================+
                                 | Tailscale (100.70.177.44)
+================================|====================================+
|                    LAN LAYER (192.168.31.x)                        |
|                                                                    |
|  +-------------------------------+-------------------------------+ |
|  |       CENTRAL SERVER (.23)                                    | |
|  |  +-------------------+  +----------------------------------+  | |
|  |  | racecontrol (Rust)|  | NATS Server + JetStream (:4222)  |  | |
|  |  | :8080 HTTP API    |  | (Windows Service, JetStream on)  |  | |
|  |  | PostgreSQL        |  | Streams: PODS, CAMERAS, POS,     |  | |
|  |  | WebSocket hub     |  |          VENUE, AI, AUDIT        |  | |
|  |  +-------------------+  +----------------------------------+  | |
|  |  +-------------------+  +----------------------------------+  | |
|  |  | Fusion Service    |  | Admin Dashboard (existing)       |  | |
|  |  | (Node.js/Python)  |  | :3201 local mirror               |  | |
|  |  | late-fusion joins |  +----------------------------------+  | |
|  |  +-------------------+                                        | |
|  +-------------------------------+-------------------------------+ |
|                                  |                                 |
|         +------------------------+-----------------------+         |
|         |     LAN NATS publish/subscribe mesh           |         |
|  +------+------+  +------+------+  +------+------+      |         |
|  |  POD 1 (.89)|  |  POD 2 (.33)|  |  POD 3 (.28)|  ... |         |
|  | rc-agent    |  | rc-agent    |  | rc-agent    |      |         |
|  | rc-sentry   |  | rc-sentry   |  | rc-sentry   |      |         |
|  | NATS client |  | NATS client |  | NATS client |      |         |
|  | SQLite spool|  | SQLite spool|  | SQLite spool|      |         |
|  | mdns-sd     |  | mdns-sd     |  | mdns-sd     |      |         |
|  +-------------+  +-------------+  +-------------+      |         |
|                                                          |         |
|  +-----------------+  +----------------------------+    |         |
|  |  POS PC (.20)   |  |  go2rtc Camera Bridge      |    |         |
|  |  rc-agent :8090 |  |  :8096 — 13 NVR cameras    |    |         |
|  |  NATS client    |  |  NATS client (camera events)|    |         |
|  +-----------------+  +----------------------------+    |         |
+=====================================================================+
```

### Component Responsibilities

| Component | Responsibility | Existing or New |
|-----------|----------------|-----------------|
| NATS Server (.23) | Event backbone, JetStream persistence, subject routing | NEW — Windows Service on central server |
| racecontrol (Rust) | Fleet ops, billing, WebSocket aggregation, NATS publisher | MODIFIED — add async-nats client |
| rc-agent (pods) | Pod health, game lifecycle, NATS heartbeat publisher | MODIFIED — add async-nats client |
| rc-sentry (pods) | Preflight checks, recovery, NATS alarm publisher | MODIFIED — add async-nats client |
| Fusion Service | Time-windowed join of pod + camera events, confidence scoring | NEW — Node.js on central server |
| Digital Twin KV | Per-pod state snapshot in NATS KV bucket | NEW — built on JetStream KV |
| NATS Leaf Node (Bono VPS) | Bridge LAN cluster to cloud over Tailscale | NEW — on Bono VPS |
| Blackboard KV | Shared AI agent coordination state | NEW — NATS KV bucket |
| mdns-sd (pods) | Zero-config peer discovery, leader election during server-down | NEW — Rust crate in rc-agent |
| SQLite spool (pods) | Offline event queue when NATS unreachable | NEW — append-only table in rc-agent |

---

## NATS Deployment Topology

### Placement Decision: Single Server, Not Cluster

**Recommendation:** Run one NATS server on the central server (.23) as a Windows service, with JetStream enabled and a leaf node on Bono VPS. Do not run a NATS cluster (3+ nodes) — the venue has one LAN server; a cluster requires 3+ hosts for quorum and is unnecessary at this scale.

**Rationale:**
- NATS server uses < 20 MB RAM — trivial overhead on existing server
- Leaf node on Bono VPS bridges cloud apps without opening inbound ports (leaf dials out)
- Pods are NATS clients, not cluster members — they connect to :4222 on .23
- Single node is sufficient; JetStream R=1 replication is fine for a venue with no standby server
- Leaf node provides cloud connectivity and can forward selected subjects via subject import/export

### Windows Service Installation

```
# On central server (.23), run once as Administrator:
sc.exe create nats-server binPath= "C:\nats\nats-server.exe -c C:\nats\nats.conf --log C:\nats\nats.log"
sc.exe start nats-server
sc.exe config nats-server start= auto
```

### NATS Server Configuration Skeleton

```
# C:\nats\nats.conf
port: 4222
http_port: 8222          # monitoring endpoint

jetstream {
  store_dir: "C:\\nats\\data"
  max_memory_store: 256MB
  max_file_store: 4GB
}

# Leaf node connection (outbound to Bono VPS)
leafnodes {
  remotes [
    {
      urls: ["nats://100.70.177.44:7422"]
      # subject import/export limits what crosses the boundary
    }
  ]
}
```

### Leaf Node on Bono VPS (outbound dial from .23)

The leaf node architecture means the central server dials OUT to Bono VPS — no inbound port required on Tailscale. Subjects cross the boundary selectively via NATS account import/export. Cloud Next.js apps and Bono AI agent subscribe via the leaf node connection transparently.

---

## Subject Taxonomy

### Design Principles (from NATS official docs)

- First token(s) establish namespace (domain)
- Encode physical entities, not technical implementation
- Use wildcards for consumer subscriptions, not per-subject subscribes
- Reserve `$` prefix for system use
- Convention: tokens in lowercase with hyphens or underscores, periods as separators

### Racing Point Subject Hierarchy

```
venue.*                         — top-level venue-wide broadcasts
venue.status                    — overall venue health summary
venue.alert.<severity>          — venue-wide alerts (critical/warn/info)

pod.<id>.*                      — all events for a specific pod (id = 1-8)
pod.<id>.health                 — periodic health snapshot (30s)
pod.<id>.heartbeat              — liveness tick (5s)
pod.<id>.session.start          — billing session opened
pod.<id>.session.end            — billing session closed
pod.<id>.game.launch            — AC/game launched
pod.<id>.game.crash             — game crashed
pod.<id>.alarm.<type>           — sentry alarm (preflight/thermal/process)
pod.<id>.peer.discovered        — mDNS peer found (pod-to-pod)
pod.<id>.leader.elected         — local leader elected during server-down
pod.<id>.spool.flush            — queued events replaying after reconnect

camera.<id>.*                   — all events for a specific camera (id = 1-13)
camera.<id>.detection           — AI detection event (person/motion/anomaly)
camera.<id>.alert               — camera-specific alert

pos.*                           — POS terminal events
pos.session.start               — customer session opened at POS
pos.payment.completed           — payment event
pos.alert                       — POS health alert

ai.*                            — AI agent events
ai.james.intent                 — James proposes an action (intent-based command)
ai.bono.intent                  — Bono proposes an action
ai.blackboard.update            — either agent writes to shared blackboard
ai.command.approved             — controller approves and executes intent
ai.command.rejected             — intent rejected with reason

fusion.*                        — fusion service output
fusion.pod-camera.<pod-id>      — fused pod+camera context for a pod zone
fusion.venue.occupancy          — occupancy estimate from camera fusion
fusion.risk.<pod-id>            — predictive risk score for a pod

audit.*                         — audit system events
audit.phase.start               — audit phase beginning
audit.phase.result              — audit phase outcome
audit.fleet.complete            — full audit cycle done
```

### JetStream Stream Bindings

| Stream Name | Subjects | Purpose | Retention |
|-------------|----------|---------|-----------|
| PODS | `pod.>` | All pod events | 7 days, limits-based |
| CAMERAS | `camera.>` | Camera detections | 24 hours (high-volume) |
| POS | `pos.>` | POS events | 30 days (billing relevance) |
| AI | `ai.>` | Agent coordination | 7 days |
| FUSION | `fusion.>` | Fused outputs | 24 hours |
| VENUE | `venue.>` | Venue-wide events | 30 days |
| AUDIT | `audit.>` | Audit trail | 90 days |

**Stream naming:** uppercase to visually distinguish from subjects (NATS convention).

---

## Architectural Patterns

### Pattern 1: Event Sourcing via JetStream Streams

**What:** Every state change is published as an immutable event to a JetStream stream. Current state is derived by replaying events, not by querying a mutable record.

**When to use:** Pod health state, session lifecycle, alarm history — anything where audit trail and replay-on-reconnect matter.

**Trade-offs:** Simpler recovery (replay from last offset); slightly more complex query path (need read model / digital twin).

**Example (Rust rc-agent publishing):**
```rust
use async_nats::jetstream;

let client = async_nats::connect("nats://192.168.31.23:4222").await?;
let js = jetstream::new(client);

let payload = serde_json::to_vec(&health_snapshot)?;
js.publish(format!("pod.{}.health", pod_id), payload.into()).await?;
```

### Pattern 2: Digital Twin via NATS KV Bucket

**What:** Each pod has a key in a NATS KV bucket (`KV_PODS`). Key = `pod.<id>`, value = latest serialized state snapshot (JSON). Updated on every health event. Consumers read current state without replaying full stream.

**When to use:** Dashboards that need "current state now" (not history), fusion service reading pod context, AI agents reading fleet state.

**Trade-offs:** Eventually consistent (last-write-wins). Optimistic concurrency via revision check prevents write conflicts.

**KV bucket config:**
```
Bucket: KV_PODS
History: 5             -- keep last 5 revisions per key
TTL: 120s              -- entry expires if pod stops publishing
Storage: File          -- persisted across NATS restart
```

### Pattern 3: Late Fusion with Time-Windowed Joins

**What:** The Fusion Service subscribes to both `pod.<id>.health` and `camera.<id>.detection` streams independently. It maintains a 5-second sliding window per zone. When both a camera detection and a pod health event fall within the same window, it publishes a fused event to `fusion.pod-camera.<id>`.

**When to use:** Correlating camera occupancy with pod session state. Detecting unoccupied pods with active sessions (billing anomaly). Validating pod-zone occupancy.

**Trade-offs:** Each sensor domain remains independent — missing camera data degrades confidence score, does not halt pod operations. Confidence is emitted with each fused event (0.0-1.0), downstream consumers decide threshold.

**Confidence scoring:**
```
confidence = 1.0
if camera_event_age > 3s:  confidence -= 0.3
if pod_health_age > 30s:   confidence -= 0.4
if camera_detection_type != "person": confidence -= 0.2
emit fusion.pod-camera.<id> with { ...data, confidence }
```

### Pattern 4: Blackboard Coordination for AI Agents

**What:** James and Bono share a NATS KV bucket (`KV_BLACKBOARD`) as a coordination surface. Each agent writes intents (`ai.james.intent`, `ai.bono.intent`) as events. A lightweight controller service (part of racecontrol or standalone) reads intents, checks for conflicts, approves or rejects, and publishes `ai.command.approved` which triggers execution.

**When to use:** Any AI-initiated action (pod restart, maintenance lock, deploy, audit trigger). Prevents two agents issuing conflicting commands simultaneously.

**Trade-offs:** Adds one hop (intent → approve → execute) versus direct RPC. The benefit: full audit trail of every AI action, conflict detection, and human override capability.

### Pattern 5: SQLite Spool + Event Replay (Degraded Mode)

**What:** When a pod cannot reach NATS (:4222 on .23), rc-agent writes events to a local SQLite table (`event_spool`). On reconnect, it replays events in order using `js.publish_with_headers()` including an `X-Spool-Timestamp` header so downstream consumers can reconstruct true event order.

**When to use:** Server maintenance, network partition, NATS restart.

**Trade-offs:** Events arrive out of real-time order during replay. Consumers must tolerate late-arriving events. For billing-critical events (session start/end), replay ensures no data loss.

```sql
CREATE TABLE event_spool (
  id       INTEGER PRIMARY KEY AUTOINCREMENT,
  subject  TEXT NOT NULL,
  payload  BLOB NOT NULL,
  ts       INTEGER NOT NULL,  -- unix ms
  replayed INTEGER DEFAULT 0
);
```

---

## Data Flow

### Normal Operations: Pod Health Event

```
rc-agent (pod N)
    |
    | async-nats publish("pod.N.health", snapshot)
    v
NATS Server (.23) — persists to PODS stream
    |
    +---> racecontrol subscriber — updates PostgreSQL pod table
    |
    +---> Fusion Service subscriber — updates pod window
    |
    +---> NATS KV put("KV_PODS", "pod.N", snapshot) — digital twin updated
    |
    +---> Leaf Node → Bono VPS — cloud dashboard receives real-time update
    |
    +---> Admin dashboard SWR refetch (existing WebSocket/HTTP polling)
```

### Camera + Pod Fusion Flow

```
go2rtc camera bridge
    |
    | NATS publish("camera.N.detection", detection_event)
    v
NATS Server (.23) — persists to CAMERAS stream
    |
    v
Fusion Service (subscribes camera.> and pod.>)
    |
    | 5-second sliding window join per zone
    v
NATS publish("fusion.pod-camera.N", fused_context + confidence)
    |
    +---> racecontrol (billing anomaly check)
    +---> AI agent subscriptions (James/Bono)
    +---> fusion.venue.occupancy aggregate
```

### AI Agent Intent Flow (Blackboard Pattern)

```
James AI Agent
    |
    | NATS publish("ai.james.intent", { action: "restart_pod", pod_id: 3, reason: "..." })
    v
NATS Server (.23)
    |
    v
Intent Controller (racecontrol or standalone)
    |
    | checks KV_BLACKBOARD for conflicting intents
    | checks operational constraints (session active? venue open?)
    v
NATS publish("ai.command.approved", { intent_id, action, executor: "james" })
    |
    v
James executes via existing relay exec endpoint
    |
    v
NATS publish("ai.james.intent.result", { intent_id, outcome })
```

### Degraded Mode: Server/NATS Unreachable

```
rc-agent (pod N) — NATS connect fails
    |
    | write events to SQLite event_spool
    |
    | mdns-sd discovers peer pods on 192.168.31.x
    |
    | pod-to-pod heartbeat via UDP multicast (224.0.0.251)
    |
    | if no existing leader: mDNS leader election
    |   pod with lowest IP wins initial election
    |   leader maintains local fleet health summary
    |
    | Tier-0 racing continues uninterrupted (no NATS dependency)
    | Tier-1 billing sessions spool locally, sync on reconnect
    | Tier-2 analytics/AI halted until reconnect
    |
    | on NATS reconnect:
    |   replay event_spool in order with X-Spool-Timestamp headers
    |   update KV_PODS digital twin
    |   resume normal publish cadence
```

---

## Degraded Mode Topology

```
+================================================================+
|  DEGRADED MODE: Central Server (.23) or NATS unreachable       |
+================================================================+

  Pod 1 (.89) <---mDNS---> Pod 2 (.33) <---mDNS---> Pod 3 (.28)
      |                        |                        |
      | UDP multicast heartbeat (224.0.0.251:5353)      |
      +--------------------+---+------------------------+
                           |
                    [Leader: lowest IP = Pod 1]
                    Maintains local fleet summary
                    No external dependencies

  Each pod independently:
  - Continues Tier-0 racing (AC, pedals, display) — NO dependency on server
  - Spools billing events to SQLite event_spool
  - Maintains local session timer (in-memory)
  - Accepts POS payment via local fallback (static rate table)

  POS PC (.20):
  - Falls back to local rate table for billing
  - Queues payments in local SQLite
  - No customer-visible disruption for active sessions

  go2rtc cameras:
  - Continue recording to NVR independently
  - Camera events not published (no NATS) — queued if camera client has spool

  Bono VPS:
  - Leaf node disconnects when .23 unreachable
  - Cloud dashboards show "server offline" status via health timeout
  - No cloud action possible until reconnect

  On server recovery:
  1. NATS server restarts (Windows service auto-restart)
  2. Pods reconnect and flush event_spool (ordered replay)
  3. racecontrol reconciles PostgreSQL from replayed events
  4. Digital twin (KV_PODS) rebuilt from replayed health snapshots
  5. Leaf node reconnects, cloud resumes
  6. AI agents resume subscriptions

+================================================================+
|  TIERED DEGRADATION LEVELS                                     |
+================================================================+
  Tier 0 — Racing Core (NEVER degrades)
    - Pod game session (AC, ffb, display)
    - Local billing timer
    - Pod health watchdog (rc-sentry)

  Tier 1 — Venue Management (degrades to local-only)
    - Billing sync (spool → replay)
    - POS payments (local fallback)
    - Staff notifications (queue → send on reconnect)

  Tier 2 — Analytics + AI (halts gracefully)
    - Fusion service outputs
    - Predictive scoring
    - AI agent proactive actions
    - Cloud dashboard real-time updates
```

---

## New vs Modified Components

### New Components

| Component | Location | Tech | Purpose |
|-----------|----------|------|---------|
| NATS Server | Central server (.23) | NATS 2.10+ binary, Windows Service | Event backbone |
| NATS Leaf Node | Bono VPS | NATS 2.10+ | Cloud bridge |
| Fusion Service | Central server (.23) | Node.js 20 | Camera+pod late fusion |
| Intent Controller | Central server (.23) | Added to racecontrol or standalone Node.js | AI command gate |
| mdns-sd integration | Each pod rc-agent | Rust crate `mdns-sd` 0.11+ | Peer discovery |
| SQLite event spool | Each pod rc-agent | Existing SQLite | Offline event queue |
| Digital Twin KV | NATS KV bucket | JetStream KV | Per-pod state mirror |
| Blackboard KV | NATS KV bucket | JetStream KV | AI agent shared state |
| Camera NATS publisher | go2rtc bridge or sidecar | Node.js or Python | Publish camera events |

### Modified Components

| Component | Change Required | Effort |
|-----------|----------------|--------|
| rc-agent (Rust) | Add `async-nats` client, publish health/session/alarm events, SQLite spool, mdns-sd | Medium |
| rc-sentry (Rust) | Add `async-nats` client, publish preflight/alarm events | Low |
| racecontrol (Rust) | Add `async-nats` client, subscribe to mesh events, update PostgreSQL, expose intent approval | Medium |
| James AI agent | Subscribe to ai/fusion/pod subjects, publish intents, read blackboard | Medium |
| Bono AI agent | Same as James, via leaf node | Medium |
| Admin dashboard | Subscribe to NATS KV watch for real-time twin updates (replaces polling) | Low |

### Backward Compatibility

All existing interfaces are preserved during rollout:
- Existing WebSocket hub (pods → .23) continues running during Phase 1-2
- Existing HTTP API (:8080) unchanged
- Existing health probes and billing sync unchanged
- NATS added as parallel channel; existing channels deprecated only after mesh is validated
- Feature flag `MESH_ENABLED` controls whether pod publishes to NATS or WebSocket only

---

## Recommended Project Structure

```
racecontrol/
├── crates/
│   ├── rc-agent/
│   │   ├── src/
│   │   │   ├── nats/              # NEW — NATS client + subject publishers
│   │   │   │   ├── client.rs      # connection, reconnect, spool flush
│   │   │   │   ├── publishers.rs  # health, session, alarm publishers
│   │   │   │   └── spool.rs       # SQLite spool read/write/replay
│   │   │   └── mdns/              # NEW — peer discovery
│   │   │       ├── discovery.rs   # mdns-sd integration
│   │   │       └── leader.rs      # leader election logic
│   │   └── migrations/
│   │       └── add_event_spool.sql
│   └── rc-sentry/
│       └── src/
│           └── nats/              # NEW — alarm publisher
│               └── publisher.rs
├── services/
│   ├── fusion/                    # NEW — Node.js fusion service
│   │   ├── src/
│   │   │   ├── windows.js         # sliding window join
│   │   │   ├── confidence.js      # confidence scoring
│   │   │   └── index.js           # NATS subscriber + publisher
│   │   └── package.json
│   └── intent-controller/         # NEW — AI command gate
│       ├── src/
│       │   ├── blackboard.js      # KV read/write
│       │   ├── constraints.js     # conflict + operational checks
│       │   └── index.js
│       └── package.json
├── nats/
│   ├── nats.conf                  # server config
│   ├── streams.json               # stream definitions (nats CLI import)
│   └── install-windows-service.bat
└── agents/
    ├── james/
    │   └── mesh/                  # NEW — mesh subscriptions + intent publisher
    └── bono/
        └── mesh/                  # NEW — same, via leaf node
```

---

## Integration Points

### Existing System → NATS

| Existing Component | Integration Method | Notes |
|---------------------|-------------------|-------|
| rc-agent health loop | Add `js.publish("pod.N.health", ...)` alongside existing WS push | Parallel during transition |
| rc-sentry alarms | Add `js.publish("pod.N.alarm.preflight", ...)` | Low-effort add |
| racecontrol WS hub | Subscribe `pod.>` from NATS, merge with WS data | Gradual migration |
| go2rtc (:8096) | Add Node.js sidecar that reads go2rtc HTTP event stream and publishes to `camera.N.detection` | New thin bridge |
| comms-link (James↔Bono) | Augment: publish AI intents to NATS `ai.>` in addition to existing WS relay | Additive |

### NATS → Existing System

| NATS Subject | Consumed By | Action |
|-------------|------------|--------|
| `pod.>.health` | racecontrol | Update PostgreSQL pod health table |
| `fusion.pod-camera.*` | racecontrol | Billing anomaly check |
| `ai.command.approved` | rc-agent relay | Execute command |
| `pod.>.spool.flush` | racecontrol | Reconcile any gap in PostgreSQL |
| `venue.alert.critical` | WhatsApp notifier | Alert Uday via Evolution API |

### Internal Boundaries

| Boundary | Communication | Notes |
|----------|--------------|-------|
| Pod rc-agent ↔ NATS server | TCP :4222, auto-reconnect | Pod dials out, not inbound |
| Fusion service ↔ NATS server | TCP :4222, push consumers | Durable consumers for both PODS and CAMERAS streams |
| Bono VPS ↔ NATS server | Leaf node via Tailscale (100.70.177.44:7422) | Leaf dials out from .23 |
| Intent controller ↔ NATS KV | JetStream KV operations | Optimistic concurrency for blackboard writes |
| Digital twin KV ↔ dashboards | NATS KV Watch API | Push updates without polling |

---

## Anti-Patterns

### Anti-Pattern 1: One Stream Per Pod

**What people do:** Create 8 streams — `PODS_1`, `PODS_2`, ... `PODS_8`.
**Why it's wrong:** NATS performs best with fewer streams with subject wildcards. Per-pod streams multiply consumer management overhead and prevent cross-pod wildcard queries.
**Do this instead:** One `PODS` stream binding `pod.>`, wildcard consumers for cross-pod queries, subject filter `pod.N.>` for per-pod consumers.

### Anti-Pattern 2: Polling Consumer Info for Pending Checks

**What people do:** Call `consumer.info()` in a loop to check if new messages arrived.
**Why it's wrong:** Consumer info is expensive at scale; NATS docs flag this as a top anti-pattern that causes server instability above ~100k consumers.
**Do this instead:** Use push consumers (NATS delivers to subscriber) or extract pending count from the last-fetched message metadata.

### Anti-Pattern 3: Making Pod Racing Depend on NATS

**What people do:** Gate game session start on a NATS publish acknowledgment.
**Why it's wrong:** Racing core (Tier 0) must never block on external infrastructure. Network partition = racing stops.
**Do this instead:** Publish-and-forget with local spool fallback. Session starts locally immediately; event reaches NATS when possible.

### Anti-Pattern 4: Direct AI-to-Pod RPC Without Intent Gate

**What people do:** AI agent calls rc-agent relay directly to restart a pod.
**Why it's wrong:** Two AI agents (James + Bono) can issue conflicting commands simultaneously. No audit trail.
**Do this instead:** All AI actions go through `ai.<agent>.intent` → intent controller → `ai.command.approved`. Intent controller detects conflicts and serializes execution.

### Anti-Pattern 5: Blocking Rust Async Code with sync NATS calls

**What people do:** Use the synchronous `nats` crate in an async Tokio context.
**Why it's wrong:** Blocks the Tokio thread pool, causes latency spikes under load.
**Do this instead:** Use `async-nats` (the official async Rust client) exclusively. The synchronous `nats` crate is legacy.

---

## Scaling Considerations

| Scale | Architecture Notes |
|-------|-------------------|
| Current (8 pods, 13 cameras) | Single NATS server, R=1 streams, one fusion service. All fits on existing server. |
| 2x venue (16 pods) | Same topology, no changes needed. NATS handles thousands of publishers. |
| Multi-venue | Promote to NATS cluster (3 nodes), add leaf node per venue. Subjects get venue prefix: `venue.rp-main.pod.>` |
| High camera event rate | Reduce CAMERAS stream retention to 1 hour, add subject filter to drop low-confidence detections before stream write |

**First bottleneck:** PostgreSQL write throughput if racecontrol writes every NATS event to DB. Mitigation: batch writes at 500ms intervals, write only delta events.

**Second bottleneck:** Fusion service sliding windows if camera detection rate spikes. Mitigation: drop camera events below confidence threshold 0.3 before fusion join.

---

## Build Order Rationale

The build order is driven by three dependency constraints:

1. NATS server must exist before any client can publish
2. Digital twin (KV) is read by fusion service — fusion requires KV to exist
3. Degraded mode (SQLite spool + mDNS) is standalone — can be built in parallel with mesh

**Suggested Phase Sequence:**

| Phase | Component | Depends On | Risk |
|-------|-----------|-----------|------|
| 1 | NATS server + streams + Windows service + nats CLI | Nothing | Low — single binary |
| 2 | rc-agent + rc-sentry async-nats publish (pod.N.health, pod.N.alarm.*) | Phase 1 | Low — additive, existing WS unchanged |
| 3 | racecontrol NATS subscriber + PostgreSQL update + Digital twin KV | Phase 2 | Medium — modifying core service |
| 4 | Leaf node on Bono VPS + cloud dashboard KV watch | Phase 3 | Low — config, not code |
| 5 | Camera NATS bridge (go2rtc sidecar) | Phase 1 | Low — new thin service |
| 6 | Fusion service (time-windowed join, confidence scoring) | Phase 3, 5 | Medium — new service, timing-sensitive |
| 7 | SQLite event spool + replay in rc-agent | Phase 2 | Medium — offline edge case testing complex |
| 8 | mDNS peer discovery + leader election in rc-agent | Phase 7 | Medium — Windows multicast networking |
| 9 | AI agent mesh subscriptions + intent publishing | Phase 3, 4 | Low — additive to existing agents |
| 10 | Intent controller + blackboard KV | Phase 9 | Medium — conflict detection logic |
| 11 | Predictive layer (stream processors, risk scoring) | Phase 6, 9 | High — requires training data from Phase 2-6 |

**Backward compatibility gate:** After Phase 2, run both NATS publish and existing WebSocket push in parallel. Decommission WebSocket hub only after Phase 3 racecontrol subscriber is stable for 1 week.

---

## Sources

- [NATS JetStream Documentation](https://docs.nats.io/nats-concepts/jetstream) — official, HIGH confidence
- [NATS Adaptive Edge Deployment](https://docs.nats.io/nats-concepts/service_infrastructure/adaptive_edge_deployment) — official, HIGH confidence
- [NATS Windows Service](https://docs.nats.io/running-a-nats-service/introduction/windows_srv) — official, HIGH confidence
- [NATS Subject-Based Messaging](https://docs.nats.io/nats-concepts/subjects) — official, HIGH confidence
- [NATS KV Store](https://docs.nats.io/nats-concepts/jetstream/key-value-store) — official, HIGH confidence
- [JetStream Anti-Patterns — Synadia](https://www.synadia.com/blog/jetstream-design-patterns-for-scale) — official vendor blog, HIGH confidence
- [async-nats Rust crate](https://docs.rs/async-nats/latest/async_nats/) — official, HIGH confidence
- [mdns-sd Rust crate](https://github.com/keepsimple1/mdns-sd) — HIGH confidence (actively maintained, Windows supported)
- [MachineMetrics NATS at Edge — Synadia](https://www.synadia.com/customer-stories/machinemetrics) — industrial IoT precedent, MEDIUM confidence
- [Four Design Patterns for Event-Driven Multi-Agent Systems — Confluent](https://www.confluent.io/blog/event-driven-multi-agent-systems/) — MEDIUM confidence
- [CQRS and Event Sourcing for IoT — SenseTecnic](http://sensetecnic.com/cqrs-and-event-sourcing-for-the-iot/) — MEDIUM confidence
- [Blackboard Pattern for Multi-Agent Systems — Medium](https://lijojose.medium.com/the-blackboard-pattern-when-agents-think-better-together-bbe6e73934ea) — MEDIUM confidence

---

*Architecture research for: Racing Point eSports — Meshed Intelligence (v24.0)*
*Researched: 2026-03-27*
