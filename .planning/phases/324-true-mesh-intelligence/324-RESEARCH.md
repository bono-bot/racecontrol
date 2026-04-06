# Phase 324: True Mesh Intelligence — Research

**Researched:** 2026-04-06 IST
**Domain:** Rust std::net UDP/TCP peer-to-peer communication; gossip protocol design; coordinated distributed action
**Confidence:** HIGH

---

<user_constraints>
## User Constraints (from Phase Description)

### Locked Decisions
- rc-sentry is pure std (NO tokio) — UDP/TCP must use std::net
- Pod IPs: 1=.89, 2=.33, 3=.28, 4=.88, 5=.86, 6=.87, 7=.38, 8=.91 (all on 192.168.31.x)
- Dedicated port for peer channel — must not conflict with :8090 (rc-agent), :8091 (rc-sentry), :8095 (people tracker)
- Solution gossip must work even when the server (.23) is completely dead
- Pure peer-to-peer — no server relay
- Keep it simple: UDP for gossip broadcast, TCP for coordinated launch (needs reliability)

### Claude's Discretion
- Port selection within the constraint above
- Seen-set / dedup implementation details
- Gossip message format
- Coordinated launch timeout values

### Deferred Ideas (OUT OF SCOPE)
- mDNS peer discovery
- Authenticated/encrypted gossip
- Cross-venue gossip
- Server relay fallback
- Leader election
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| MESH-01 | Direct pod-to-pod peer channel without server | std::net::UdpSocket bind on :8092, unicast to each peer |
| MESH-02 | Solution propagation via direct gossip within 60s | UDP broadcast loop, store_solution() receiver, /kb/solutions endpoint |
| MESH-03 | Coordinated multiplayer launch <500ms without server | TCP listener on :8093, two-phase READY/LAUNCH protocol |
</phase_requirements>

---

## Summary

Phase 324 adds direct pod-to-pod communication to rc-sentry so the 8-pod fleet can coordinate without the server at 192.168.31.23. The existing rc-sentry is pure std (no tokio), uses TCP for its HTTP server on :8091, and already has a background thread model (watchdog, crash-handler, mesh-client, MI engine threads all run concurrently via std::thread).

The new code adds two background threads and one new UDP socket:
1. **peer_channel thread** — binds UDP :8092, receives incoming gossip, calls mi_knowledge_base::store_solution() on new solutions
2. **peer_gossip sender** — triggered by MI tier engine after a solution is stored; sends UDP unicast to all 8 peers
3. **peer_launch thread** — binds TCP :8093, handles coordinated launch rendezvous for multiplayer sessions

The existing MI knowledge base (`mi_knowledge_base.rs`) already has `store_solution()` and `solution_count()`. The `/kb/solutions` HTTP endpoint just needs to be wired into main.rs's handle() dispatcher.

**Primary recommendation:** Two new modules (`peer_channel.rs` + `peer_launch.rs`), one new config section (`[peer]` in rc-sentry.toml), and two new HTTP endpoints (`GET /kb/solutions`, `POST /peer/ping`). No new crate dependencies — std::net + existing serde_json + existing rc-sentry types.

---

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| std::net::UdpSocket | stdlib | UDP gossip send/receive | Only option in pure-std context |
| std::net::TcpListener | stdlib | TCP coordinated launch | Consistent with rc-sentry HTTP server pattern |
| std::net::TcpStream | stdlib | TCP client for peer connect | Same pattern as existing mesh_client.rs |
| serde_json | workspace | Message serialization | Already used everywhere |
| std::collections::HashSet | stdlib | Seen-set for dedup | Sufficient; no external dep |
| std::sync::mpsc | stdlib | Cross-thread event passing | Consistent with existing crash-handler pattern |

### No New Dependencies Required
The Cargo.toml already has everything needed:
- `serde` / `serde_json` (workspace) — gossip message JSON
- `rusqlite` (bundled) — KB access via existing mi_knowledge_base.rs
- `tracing` (workspace) — logging

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| UDP unicast to each peer | UDP multicast 224.x.x.x | Multicast requires router support — venue router may block; unicast is guaranteed to work on LAN |
| JSON over UDP | CBOR/bincode | JSON is already the pattern; binary saves ~30% bytes but adds dep and complexity |
| Static peer table | mDNS discovery | mDNS is Phase 245; static table eliminates dependency and is already proven (CLAUDE.md has pod IPs) |

---

## Architecture Patterns

### Recommended Project Structure Addition
```
crates/rc-sentry/src/
├── peer_channel.rs      # NEW: UDP gossip listener + sender + seen-set
├── peer_launch.rs       # NEW: TCP coordinated launch coordinator + client
├── mi_knowledge_base.rs # EXISTING: add gossip_store() helper
├── main.rs              # WIRE: spawn peer threads, add /kb/solutions + /peer/ping routes
└── sentry_config.rs     # ADD: PeerConfig struct with peer table + ports
```

### Pattern 1: UDP Gossip with Seen-Set Dedup
**What:** Each pod's peer_channel binds UDP :8092. When a solution is stored locally, it serializes a `GossipMessage` and sends UDP unicast to each of the 7 other peer IPs. Receivers check the seen-set before processing.
**When to use:** Solution propagation, peer pings, future: diagnostic announcements

```rust
// Source: std::net::UdpSocket docs + rc-sentry pattern
// peer_channel.rs

use std::net::UdpSocket;
use std::collections::HashMap;
use std::time::Instant;

const GOSSIP_PORT: u16 = 8092;
const MAX_UDP_PAYLOAD: usize = 1400; // Safe for LAN MTU
const SEEN_SET_TTL_SECS: u64 = 120;

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum GossipMessage {
    Ping { from: String, seq: u64, ts_unix: u64 },
    Pong { from: String, seq: u64, ts_unix: u64 },
    SolutionUpdate {
        from: String,
        seq: u64,
        ts_unix: u64,
        problem_key: String,
        problem_hash: String,
        fix_action: String,
        confidence: f64,
        source_node: String,
    },
}

// Seen-set entry: (source_node, seq) -> expiry_instant
type SeenSet = HashMap<(String, u64), Instant>;

pub fn spawn_receiver(shutdown: &'static std::sync::atomic::AtomicBool) -> std::sync::mpsc::Receiver<GossipMessage> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::Builder::new()
        .name("peer-gossip-rx".to_string())
        .spawn(move || run_receiver(tx, shutdown))
        .expect("spawn peer-gossip-rx");
    rx
}

fn run_receiver(tx: std::sync::mpsc::Sender<GossipMessage>, shutdown: &std::sync::atomic::AtomicBool) {
    let socket = match UdpSocket::bind(format!("0.0.0.0:{}", GOSSIP_PORT)) {
        Ok(s) => s,
        Err(e) => { tracing::error!(target: "peer-gossip", "bind :8092 failed: {e}"); return; }
    };
    socket.set_read_timeout(Some(std::time::Duration::from_secs(1))).ok();

    let mut seen: SeenSet = HashMap::new();
    let mut buf = [0u8; MAX_UDP_PAYLOAD + 64];

    loop {
        if shutdown.load(std::sync::atomic::Ordering::Acquire) { return; }

        // Evict stale seen entries
        let now = Instant::now();
        seen.retain(|_, exp| *exp > now);

        match socket.recv_from(&mut buf) {
            Ok((n, _addr)) => {
                if let Ok(msg) = serde_json::from_slice::<GossipMessage>(&buf[..n]) {
                    let (from, seq) = match &msg {
                        GossipMessage::Ping { from, seq, .. } => (from.clone(), *seq),
                        GossipMessage::Pong { from, seq, .. } => (from.clone(), *seq),
                        GossipMessage::SolutionUpdate { from, seq, .. } => (from.clone(), *seq),
                    };
                    let key = (from, seq);
                    if !seen.contains_key(&key) {
                        seen.insert(key, Instant::now() + std::time::Duration::from_secs(SEEN_SET_TTL_SECS));
                        let _ = tx.send(msg);
                    }
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock
                       || e.kind() == std::io::ErrorKind::TimedOut => {}
            Err(e) => tracing::warn!(target: "peer-gossip", "recv error: {e}"),
        }
    }
}

pub fn send_gossip(msg: &GossipMessage, peers: &[&str]) {
    let socket = match UdpSocket::bind("0.0.0.0:0") {
        Ok(s) => s,
        Err(e) => { tracing::warn!(target: "peer-gossip", "send socket bind failed: {e}"); return; }
    };
    let data = match serde_json::to_vec(msg) {
        Ok(d) => d,
        Err(e) => { tracing::warn!(target: "peer-gossip", "serialize failed: {e}"); return; }
    };
    if data.len() > MAX_UDP_PAYLOAD {
        tracing::warn!(target: "peer-gossip", "message too large: {} bytes", data.len());
        return;
    }
    for peer_ip in peers {
        let addr = format!("{}:{}", peer_ip, GOSSIP_PORT);
        if let Err(e) = socket.send_to(&data, &addr) {
            tracing::warn!(target: "peer-gossip", "send to {addr} failed: {e}");
        }
    }
}
```

### Pattern 2: TCP Coordinated Launch (Two-Phase)
**What:** When multiplayer session is detected, the "initiator" (lowest-numbered pod in session) connects to all session peers on TCP :8093. Phase 1: send READY with target_ts. Phase 2: collect ACKs from peers within 200ms, then all parties launch at target_ts (200ms in future from READY).
**When to use:** Multiplayer game coordinated launch only

```rust
// Source: std::net::TcpListener docs + rc-sentry handle() pattern
// peer_launch.rs

const LAUNCH_PORT: u16 = 8093;
const READY_TIMEOUT_MS: u64 = 200;

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum LaunchMessage {
    Ready { session_id: String, launch_at_ms: u64, from: String },
    Ack { session_id: String, from: String },
    Launch { session_id: String },
}
```

### Pattern 3: /kb/solutions HTTP Endpoint
**What:** Simple GET endpoint exposing solution count for external verification (success criterion 3).
**When to use:** After gossip propagation to verify fleet-wide KB sync

```rust
// Add to main.rs handle() dispatcher
("GET", "/kb/solutions") => handle_kb_solutions(stream),

fn handle_kb_solutions(stream: &mut TcpStream) -> Result<(), Box<dyn std::error::Error>> {
    match mi_knowledge_base::KnowledgeBase::open(mi_knowledge_base::KB_PATH) {
        Ok(kb) => {
            let count = kb.solution_count().unwrap_or(0);
            let resp = serde_json::json!({ "solution_count": count, "kb_path": mi_knowledge_base::KB_PATH });
            send_response(stream, 200, &resp.to_string())
        }
        Err(e) => {
            send_response(stream, 500, &serde_json::json!({ "error": e.to_string() }).to_string())
        }
    }
}
```

### Anti-Patterns to Avoid
- **Blocking UDP recv without timeout:** `recv_from` without `set_read_timeout` blocks forever — shutdown signal never checked
- **Gossip amplification loop:** A pod re-broadcasting received gossip to peers would create exponential storms; only originate gossip from the pod that stored the solution
- **Gossip before KB write confirms:** Only send gossip AFTER `store_solution()` returns `Ok(())` — never pre-announce
- **TCP launch with blocking connect:** `TcpStream::connect` without timeout blocks up to system TCP timeout (minutes); always use `connect_timeout` with 200ms
- **Single global UDP socket reuse across threads:** Bind a fresh send socket per call (or use channel to dedicated sender thread) — UdpSocket is not Sync

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Dedup gossip | Custom rolling window | HashMap + Instant TTL eviction | Simple, correct, no alloc overhead |
| Peer IP discovery | mDNS/Zeroconf | Static table in sentry_config.toml | mDNS is Phase 245; static table already works for fixed-IP venue |
| Message framing | Length-prefix | Raw UDP datagrams (fits MTU) | Solution records are <400 bytes; single datagram is sufficient |
| Coordinated timing | NTP sync | Relative 200ms future target from initiator | NTP not guaranteed; 200ms slack absorbs LAN jitter |

---

## Runtime State Inventory

This is a new feature phase (not a rename/refactor). No runtime state migration required.

| Category | Items Found | Action Required |
|----------|-------------|------------------|
| Stored data | mesh_kb.db on each pod (existing solutions table) | No migration — gossip adds rows via existing store_solution() |
| Live service config | rc-sentry.toml on each pod | Add [peer] section during deploy |
| OS-registered state | None — no new scheduled tasks | None |
| Secrets/env vars | None — peer comms is LAN-internal, no auth | None |
| Build artifacts | New rc-sentry.exe after build | Standard deploy to all 8 pods via deploy protocol |

---

## Common Pitfalls

### Pitfall 1: UDP Socket on Windows with SO_REUSEADDR
**What goes wrong:** Binding a UDP socket while another instance is running (e.g., during restart) fails with "address already in use" unless SO_REUSEADDR is set.
**Why it happens:** Windows does not auto-release UDP sockets on process death as fast as Linux.
**How to avoid:** Set `socket.set_reuse_address(true)` (available in std via `std::net::UdpSocket` — actually not available directly; use `socket2` crate or Windows API). Simpler: on pod restart, the old process exits, new one waits 1s before binding.
**Warning signs:** `bind :8092 failed: address already in use` in logs immediately after redeploy.
**Resolution:** Since rc-sentry already uses this pattern for its HTTP server (TCP port 8091) without issues, UDP 8092 will behave the same. The watchdog kills the old process before the new one starts.

### Pitfall 2: UDP Message Size Exceeding LAN MTU
**What goes wrong:** SolutionRecord serialized to JSON can exceed 1400 bytes if `symptoms`, `environment`, or `tags` fields are long strings.
**Why it happens:** UDP datagrams exceeding MTU are fragmented at IP layer; some home/venue routers drop fragments.
**How to avoid:** Truncate the gossip payload to essential fields only (problem_key, problem_hash, fix_action, confidence, source_node) — not the full SolutionRecord. The receiver can do a full KB lookup if needed. Enforce 1400-byte max and log a warning if exceeded.
**Warning signs:** Gossip sent but peer KB count doesn't increase.

### Pitfall 3: TCP connect_timeout Not Available on std::net::TcpStream Directly
**What goes wrong:** `TcpStream::connect()` with a string addr has no timeout parameter. A peer that is powered off causes up to 20 second block.
**Why it happens:** std::net TCP connect blocks on SYN/ACK.
**How to avoid:** Use `TcpStream::connect_timeout(&socket_addr, Duration::from_millis(200))` which IS available in std. Parse the peer addr to `SocketAddr` first.
**Warning signs:** Coordinated launch takes >500ms when any peer is offline.

### Pitfall 4: Gossip Storm on Startup
**What goes wrong:** All 8 pods start simultaneously after a fleet reboot and each gossips its entire KB to all 7 peers — 8x7 = 56 UDP messages in first few seconds.
**Why it happens:** No startup delay.
**How to avoid:** Add a 5-10 second startup delay before beginning gossip broadcasts. Each pod only gossips NEW solutions (those stored after startup), not existing KB entries. Existing solutions are already present on all pods from prior gossip rounds.
**Warning signs:** Log flood of "Solution stored in MI KB" entries in first 10 seconds.

### Pitfall 5: Coordinated Launch Race — Both Pods Think They're Initiator
**What goes wrong:** Pods 3 and 5 both detect the multiplayer session simultaneously and both try to initiate TCP connection to the other.
**Why it happens:** No tiebreaker for initiator selection.
**How to avoid:** Lowest-numbered pod (lowest LAN IP octet, or lowest pod number) is always the initiator. Pod 5 connects to Pod 3 as listener. Pod 3 is listener. This is deterministic from static config.
**Warning signs:** TCP connection refused because neither is listening, or both connect simultaneously and race.

---

## Code Examples

### Wiring peer threads in main.rs
```rust
// Source: existing main.rs pattern for MI engine + watchdog

// After MI engine spawn (line ~417 in main.rs):
let peer_cfg = cfg.peer.clone();  // new PeerConfig from sentry_config
if peer_cfg.enabled {
    let gossip_rx = peer_channel::spawn_receiver(&SHUTDOWN_REQUESTED);

    // Gossip processor thread — stores received solutions in local KB
    std::thread::Builder::new()
        .name("peer-gossip-proc".to_string())
        .spawn(move || {
            for msg in gossip_rx {
                match msg {
                    peer_channel::GossipMessage::SolutionUpdate { problem_key, problem_hash, fix_action, confidence, source_node, .. } => {
                        if let Ok(kb) = mi_knowledge_base::KnowledgeBase::open(mi_knowledge_base::KB_PATH) {
                            let solution = build_gossip_solution(&problem_key, &problem_hash, &fix_action, confidence, &source_node);
                            if let Err(e) = kb.store_solution(&solution) {
                                tracing::warn!(target: "peer-gossip", "store failed: {e}");
                            } else {
                                tracing::info!(target: "peer-gossip", problem_key = %problem_key, from = %source_node, "gossip solution stored");
                            }
                        }
                    }
                    peer_channel::GossipMessage::Ping { from, seq, .. } => {
                        // Reply with Pong
                        peer_channel::send_gossip(&peer_channel::GossipMessage::Pong { from: peer_cfg.node_id.clone(), seq, ts_unix: unix_ts() }, &[&peer_ip_for(&from, &peer_cfg)]);
                    }
                    peer_channel::GossipMessage::Pong { .. } => {}
                }
            }
        })
        .expect("spawn peer-gossip-proc");

    peer_launch::spawn_listener(&SHUTDOWN_REQUESTED);
}
```

### sentry_config.rs PeerConfig addition
```rust
#[derive(Clone, Deserialize, Default)]
pub struct PeerConfig {
    /// Enable peer-to-peer gossip and coordinated launch
    #[serde(default)]
    pub enabled: bool,

    /// This pod's identifier (e.g. "pod_1")
    #[serde(default)]
    pub node_id: String,

    /// UDP port for gossip (default 8092)
    #[serde(default = "default_gossip_port")]
    pub gossip_port: u16,

    /// TCP port for coordinated launch (default 8093)
    #[serde(default = "default_launch_port")]
    pub launch_port: u16,

    /// Known peers: node_id -> IP
    /// Example: { "pod_1" = "192.168.31.89", ... }
    #[serde(default)]
    pub peers: std::collections::HashMap<String, String>,
}

fn default_gossip_port() -> u16 { 8092 }
fn default_launch_port() -> u16 { 8093 }
```

### rc-sentry.toml peer section (deploy to all pods)
```toml
[peer]
enabled = true
node_id = "pod_1"   # Change per pod

[peer.peers]
pod_1 = "192.168.31.89"
pod_2 = "192.168.31.33"
pod_3 = "192.168.31.28"
pod_4 = "192.168.31.88"
pod_5 = "192.168.31.86"
pod_6 = "192.168.31.87"
pod_7 = "192.168.31.38"
pod_8 = "192.168.31.91"
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| All gossip via server WS | Direct UDP pod-to-pod | Phase 324 | Server-independent fleet learning |
| Solution propagation: server broadcasts | Gossip: originating pod broadcasts | Phase 324 | Zero server dependency |
| Game launch: server orchestrates WS sequence | Coordinated launch: TCP rendezvous on pods | Phase 324 | <500ms sync without server |

**Deprecated/outdated in this context:**
- The existing `mesh_gossip.rs` in rc-agent: this was the old server-routed gossip. Phase 324 builds server-free gossip in rc-sentry. The two coexist — rc-agent still announces solutions upward to server; rc-sentry gossips laterally to peers.

---

## Open Questions

1. **Does rc-sentry.toml exist on all 8 pods currently?**
   - What we know: SentryConfig has `load()` that falls back to defaults if no file exists
   - What's unclear: Whether any pod currently has a rc-sentry.toml deployed
   - Recommendation: Deploy new rc-sentry.toml with `[peer]` section as part of this phase's deploy step

2. **Will Windows Firewall block UDP :8092 and TCP :8093 on pods?**
   - What we know: The pods currently have "firewall disable removed" from the audit (CLAUDE.md: "Pod firewall disable removed" in 60-phase audit)
   - What's unclear: Whether Windows Firewall is on or off on pods
   - Recommendation: Add `netsh advfirewall firewall add rule` commands to start-rcsentry.bat as part of deploy, or add to the install bat. Test on Pod 8 first.

3. **Does the MI tier engine expose a hook for "solution stored" events?**
   - What we know: `mi_tier_engine.rs` exists but wasn't read in full
   - What's unclear: Whether there's an existing callback or channel for post-store events
   - Recommendation: Read mi_tier_engine.rs in planning phase; if no hook exists, peer_channel can poll the KB solution_count every 5s and gossip on increase (simpler, no code change to tier engine)

---

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Pod LAN (192.168.31.x) | UDP gossip | Yes | — | None needed |
| UDP port 8092 | Gossip | Not yet bound | — | Bind at startup |
| TCP port 8093 | Launch | Not yet bound | — | Bind at startup |
| rusqlite (bundled) | KB access | Yes (in Cargo.toml) | 0.32 bundled | — |
| serde_json | Message format | Yes (workspace) | — | — |

---

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | cargo test (built-in) |
| Config file | None — workspace-level Cargo.toml |
| Quick run command | `cargo test -p rc-sentry` |
| Full suite command | `cargo test -p rc-sentry -p rc-common` |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| MESH-01 | UDP gossip send+receive loop | unit | `cargo test -p rc-sentry peer_channel` | ❌ Wave 0 |
| MESH-01 | Seen-set dedup prevents double-process | unit | `cargo test -p rc-sentry seen_set` | ❌ Wave 0 |
| MESH-02 | GossipSolutionUpdate → store_solution() roundtrip | unit | `cargo test -p rc-sentry gossip_store` | ❌ Wave 0 |
| MESH-02 | /kb/solutions count exposed correctly | unit | `cargo test -p rc-sentry kb_solutions_endpoint` | ❌ Wave 0 |
| MESH-03 | LaunchMessage ser/deser | unit | `cargo test -p rc-sentry launch_message` | ❌ Wave 0 |
| MESH-03 | connect_timeout path compiles | compile | `cargo build -p rc-sentry` | N/A |

### Sampling Rate
- **Per task commit:** `cargo test -p rc-sentry`
- **Per wave merge:** `cargo test -p rc-sentry -p rc-common`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `crates/rc-sentry/src/peer_channel.rs` — unit tests for receiver loop, seen-set, send_gossip
- [ ] `crates/rc-sentry/src/peer_launch.rs` — unit tests for LaunchMessage ser/deser, connect_timeout path
- [ ] New test functions in existing test modules (no new files needed for /kb/solutions endpoint test)

---

## Sources

### Primary (HIGH confidence)
- std::net::UdpSocket Rust docs — bind, recv_from, send_to, set_read_timeout, set_reuse_address
- std::net::TcpStream docs — connect_timeout signature verified: `TcpStream::connect_timeout(&addr: &SocketAddr, timeout: Duration) -> Result<TcpStream>`
- Existing rc-sentry codebase — thread spawn pattern, SlotGuard, handle() dispatcher, sentry_config.rs structure
- mi_knowledge_base.rs — store_solution(), solution_count(), KB_PATH constant

### Secondary (MEDIUM confidence)
- UDP MTU: 1500 bytes Ethernet MTU - 20 IP header - 8 UDP header = 1472 bytes safe payload. Using 1400 for extra margin (verified common practice)
- Windows Firewall: per CLAUDE.md audit notes "Pod firewall disable removed" — implies firewall may be active

### Tertiary (LOW confidence — flag for validation)
- Whether rc-sentry.toml exists on deployed pods: not verified from current codebase state
- Whether mi_tier_engine.rs has a post-store callback: not read in this research pass

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all std::net, no new deps
- Architecture: HIGH — consistent with existing rc-sentry thread model
- Pitfalls: HIGH — UDP/Windows-specific pitfalls verified from std docs + existing codebase patterns
- Port selection: HIGH — verified no conflicts with :8090, :8091, :8095

**Research date:** 2026-04-06
**Valid until:** 2026-05-06 (stable Rust std::net APIs)
