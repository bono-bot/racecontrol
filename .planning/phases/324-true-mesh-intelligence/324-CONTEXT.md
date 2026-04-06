# Phase 324: True Mesh Intelligence — Context

**Phase:** 324
**Name:** True Mesh Intelligence
**Date:** 2026-04-06 IST
**Depends on:** Phase 323 (MI engine + cognitive gate fully in rc-sentry)

## Goal

Pods can coordinate directly with each other without routing through the server. Solutions discovered on one pod propagate to the fleet and multiplayer game sessions can self-coordinate.

## Success Criteria (LOCKED — from ROADMAP)

1. Pod 1 can send a message directly to Pod 2 via rc-sentry's peer channel (UDP or TCP on a dedicated port) without the server involved — verified by killing the server and watching a direct pod-to-pod ping succeed.
2. When Pods 3 and 5 are in the same F1 25 multiplayer session, rc-sentry on both pods can coordinate a synchronized launch — both pods receive LaunchGame within 500ms of each other without server orchestration.
3. When rc-sentry on Pod 4 records a solution to a known failure pattern (e.g. "MAINTENANCE_MODE clear + restart"), that solution propagates to all other pods via direct gossip within 60 seconds — verified by checking each pod's `:8091 /kb/solutions` count.

## Requirements Addressed

- **MESH-01 (Phase 324 variant):** Direct pod-to-pod peer channel, not through server
- **MESH-02:** Solution propagation via peer gossip
- **MESH-03:** Coordinated multiplayer launch without server orchestration

## Decisions (LOCKED)

### Transport
- **UDP port 8092** for gossip broadcast (solution propagation, peer pings)
- **TCP port 8093** for coordinated launch (needs reliability + <500ms timing guarantee)
- Ports 8092/8093 chosen to avoid: :8090 (rc-agent), :8091 (rc-sentry HTTP), :8095 (people tracker)

### Architecture
- **New module: `peer_channel.rs`** — UDP gossip listener + broadcaster in rc-sentry
- **New module: `peer_launch.rs`** — TCP coordinated launch coordinator
- **Static peer table** from sentry_config.toml — all 8 pod IPs hardcoded (192.168.31.x)
- No mDNS in this phase — static table is sufficient and eliminates dependency
- Pure std::net throughout — no tokio, consistent with rc-sentry constraint

### Peer Table (Static)
```
pod_1 = "192.168.31.89"
pod_2 = "192.168.31.33"
pod_3 = "192.168.31.28"
pod_4 = "192.168.31.88"
pod_5 = "192.168.31.86"
pod_6 = "192.168.31.87"
pod_7 = "192.168.31.38"
pod_8 = "192.168.31.91"
```

### Gossip Protocol
- UDP broadcast to all known peers (not true UDP broadcast — unicast to each peer's :8092)
- Message format: JSON over UDP (fits in single MTU for solution records <1400 bytes)
- Anti-replay: sequence number + 60s TTL window to deduplicate re-received gossip
- Seen-set: `HashSet<(source_node, seq)>` to drop already-processed messages
- Max UDP payload: 1400 bytes (safe for LAN MTU 1500 - headers)

### Coordinated Launch Protocol
- TCP connect to all session peers on :8093
- Two-phase: READY broadcast → collect ACKs → LAUNCH at agreed timestamp
- Timeout: 200ms for READY ACKs, then fire unilaterally (degrade gracefully)
- Session peers identified by: `peer_session_id` field set when F1 25 multiplayer is detected

### Solution Propagation
- rc-sentry's MI engine stores solutions in `mi_knowledge_base.rs` (existing KB)
- After `store_solution()` succeeds, gossip thread broadcasts `GossipSolutionUpdate` via UDP
- Receiving pod calls `kb.store_solution()` with `promotion_status = "fleet_gossip"`
- `/kb/solutions` HTTP endpoint added to rc-sentry to expose solution count (success criteria 3)

## Claude's Discretion

- **Seen-set eviction policy:** Use time-based eviction (drop entries older than 120s) rather than bounded size, to handle burst gossip correctly
- **UDP send errors:** Log warn and continue — LAN packet loss is non-fatal for gossip
- **Coordinated launch fallback:** If TCP peer connect fails within 200ms, launch independently with log warning

## Deferred (OUT OF SCOPE for Phase 324)

- mDNS peer discovery (Phase 245 in v27.0 roadmap)
- Authenticated gossip (SEC-01 in future requirements)
- Cross-venue gossip (multi-venue is Phase 303+)
- Server relay fallback for gossip (Phase 324 is server-down operation)
- Peer leader election (Phase 245)
- Gossip fan-out limiting / partial mesh (Phase 324 uses full broadcast to all 8 peers)
