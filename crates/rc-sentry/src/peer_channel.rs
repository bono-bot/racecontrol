//! Peer Channel — direct pod-to-pod UDP gossip for Phase 324.
//!
//! - Binds UDP :8092 (configurable via PeerConfig.gossip_port)
//! - Receives GossipMessage datagrams from other pods
//! - Sends unicast to each known peer (no server relay)
//! - Seen-set with 120s TTL prevents gossip amplification
//!
//! Pure std::net — no tokio, no async.

use std::collections::HashMap;
use std::net::UdpSocket;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub const MAX_UDP_PAYLOAD: usize = 1400;
pub const SEEN_SET_TTL_SECS: u64 = 120;
const LOG_TARGET: &str = "peer-gossip";

static GOSSIP_SEQ: AtomicU64 = AtomicU64::new(0);
static GOSSIP_TX: OnceLock<std::sync::mpsc::SyncSender<GossipMessage>> = OnceLock::new();

pub type SeenSet = HashMap<(String, u64), Instant>;

fn unix_ts() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn next_seq() -> u64 {
    GOSSIP_SEQ.fetch_add(1, Ordering::Relaxed)
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
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

impl GossipMessage {
    fn from_seq(&self) -> (&str, u64) {
        match self {
            GossipMessage::Ping { from, seq, .. } => (from, *seq),
            GossipMessage::Pong { from, seq, .. } => (from, *seq),
            GossipMessage::SolutionUpdate { from, seq, .. } => (from, *seq),
        }
    }
}

/// Gossip event received from a peer. Sent from the receiver thread to the processor.
pub type GossipRx = std::sync::mpsc::Receiver<GossipMessage>;

/// Spawn the gossip receiver thread on the given port.
/// Returns a channel receiver for the main thread to process incoming messages.
pub fn spawn_receiver(
    port: u16,
    shutdown: &'static std::sync::atomic::AtomicBool,
) -> GossipRx {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::Builder::new()
        .name("peer-gossip-rx".to_string())
        .spawn(move || run_receiver(port, tx, shutdown))
        .expect("spawn peer-gossip-rx");
    rx
}

fn run_receiver(
    port: u16,
    tx: std::sync::mpsc::Sender<GossipMessage>,
    shutdown: &std::sync::atomic::AtomicBool,
) {
    let socket = match UdpSocket::bind(format!("0.0.0.0:{}", port)) {
        Ok(s) => s,
        Err(e) => {
            tracing::error!(target: LOG_TARGET, port = port, "bind UDP :{} failed: {e}", port);
            return;
        }
    };
    if let Err(e) = socket.set_read_timeout(Some(Duration::from_secs(1))) {
        tracing::warn!(target: LOG_TARGET, "set_read_timeout failed: {e}");
    }
    tracing::info!(target: LOG_TARGET, port = port, "UDP gossip receiver bound");

    let mut seen: SeenSet = HashMap::new();
    let mut buf = [0u8; MAX_UDP_PAYLOAD + 64];

    loop {
        if shutdown.load(Ordering::Acquire) {
            tracing::info!(target: LOG_TARGET, "gossip receiver shutdown");
            return;
        }

        // Evict expired seen entries (prevents unbounded growth)
        let now = Instant::now();
        seen.retain(|_, exp| *exp > now);

        match socket.recv_from(&mut buf) {
            Ok((n, addr)) => {
                match serde_json::from_slice::<GossipMessage>(&buf[..n]) {
                    Ok(msg) => {
                        let (from, seq) = msg.from_seq();
                        let key = (from.to_string(), seq);
                        if seen.contains_key(&key) {
                            tracing::debug!(
                                target: LOG_TARGET,
                                from = from, seq = seq,
                                "gossip dedup -- already seen"
                            );
                            continue;
                        }
                        seen.insert(
                            key,
                            Instant::now() + Duration::from_secs(SEEN_SET_TTL_SECS),
                        );
                        tracing::debug!(
                            target: LOG_TARGET,
                            from = from, seq = seq, peer = %addr,
                            "received gossip"
                        );
                        if tx.send(msg).is_err() {
                            // Processor channel closed -- time to exit
                            tracing::warn!(target: LOG_TARGET, "gossip channel closed -- receiver exiting");
                            return;
                        }
                    }
                    Err(e) => {
                        tracing::warn!(target: LOG_TARGET, peer = %addr, "invalid gossip JSON: {e}");
                    }
                }
            }
            Err(ref e)
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut => {}
            Err(e) => {
                tracing::warn!(target: LOG_TARGET, "recv error: {e}");
            }
        }
    }
}

/// Send a gossip message to a list of peer IP addresses.
/// Uses a transient ephemeral socket (bind to port 0) for sending -- avoids port conflicts.
pub fn send_gossip(msg: &GossipMessage, peer_ips: &[&str], gossip_port: u16) {
    let data = match serde_json::to_vec(msg) {
        Ok(d) => d,
        Err(e) => {
            tracing::warn!(target: LOG_TARGET, "gossip serialize failed: {e}");
            return;
        }
    };
    if data.len() > MAX_UDP_PAYLOAD {
        tracing::warn!(
            target: LOG_TARGET,
            "gossip message too large: {} bytes (max {})",
            data.len(), MAX_UDP_PAYLOAD
        );
        return;
    }
    let socket = match UdpSocket::bind("0.0.0.0:0") {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(target: LOG_TARGET, "send socket bind failed: {e}");
            return;
        }
    };
    for peer_ip in peer_ips {
        let addr = format!("{}:{}", peer_ip, gossip_port);
        match socket.send_to(&data, &addr) {
            Ok(_) => tracing::debug!(target: LOG_TARGET, peer = %addr, "gossip sent"),
            Err(e) => tracing::warn!(target: LOG_TARGET, peer = %addr, "gossip send failed: {e}"),
        }
    }
}

/// Initialize the gossip send channel. Called once from main.rs after peer config is loaded.
/// Returns a Receiver that the gossip sender thread will drain.
pub fn init_gossip_sender() -> std::sync::mpsc::Receiver<GossipMessage> {
    let (tx, rx) = std::sync::mpsc::sync_channel(64);
    let _ = GOSSIP_TX.set(tx);
    rx
}

/// Queue a gossip message for sending. Safe to call from any thread.
/// No-op if peer gossip was not initialized (peer.enabled=false).
pub fn queue_gossip(msg: GossipMessage) {
    if let Some(tx) = GOSSIP_TX.get() {
        if let Err(e) = tx.try_send(msg) {
            tracing::warn!(target: LOG_TARGET, "gossip queue full or closed: {e}");
        }
    }
}

/// Build a SolutionUpdate gossip message from stored solution fields.
pub fn build_solution_update(
    from: &str,
    problem_key: &str,
    problem_hash: &str,
    fix_action: &str,
    confidence: f64,
    source_node: &str,
) -> GossipMessage {
    GossipMessage::SolutionUpdate {
        from: from.to_string(),
        seq: next_seq(),
        ts_unix: unix_ts(),
        problem_key: problem_key.to_string(),
        problem_hash: problem_hash.to_string(),
        fix_action: fix_action.to_string(),
        confidence,
        source_node: source_node.to_string(),
    }
}

/// Build a Ping gossip message.
pub fn build_ping(from: &str) -> GossipMessage {
    GossipMessage::Ping {
        from: from.to_string(),
        seq: next_seq(),
        ts_unix: unix_ts(),
    }
}

/// Build a Pong in response to a Ping.
pub fn build_pong(from: &str, ping_seq: u64) -> GossipMessage {
    GossipMessage::Pong {
        from: from.to_string(),
        seq: ping_seq, // Echo the seq so sender can match response
        ts_unix: unix_ts(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gossip_message_ping_roundtrip() {
        let msg = GossipMessage::Ping {
            from: "pod_1".to_string(),
            seq: 42,
            ts_unix: 1712345678,
        };
        let bytes = serde_json::to_vec(&msg).expect("serialize ping");
        assert!(bytes.len() < MAX_UDP_PAYLOAD, "Ping must fit in one UDP datagram");
        let decoded: GossipMessage = serde_json::from_slice(&bytes).expect("deserialize ping");
        match decoded {
            GossipMessage::Ping { from, seq, ts_unix } => {
                assert_eq!(from, "pod_1");
                assert_eq!(seq, 42);
                assert_eq!(ts_unix, 1712345678);
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn gossip_message_solution_update_roundtrip() {
        let msg = GossipMessage::SolutionUpdate {
            from: "pod_3".to_string(),
            seq: 100,
            ts_unix: 1712345678,
            problem_key: "maintenance_mode_clear".to_string(),
            problem_hash: "abc123def456abcd".to_string(),
            fix_action: "del MAINTENANCE_MODE && schtasks /Run /TN StartRCAgent".to_string(),
            confidence: 0.95,
            source_node: "pod_4".to_string(),
        };
        let bytes = serde_json::to_vec(&msg).expect("serialize solution update");
        assert!(bytes.len() < MAX_UDP_PAYLOAD, "SolutionUpdate must fit in one UDP datagram: {} bytes", bytes.len());
        let decoded: GossipMessage = serde_json::from_slice(&bytes).expect("deserialize solution update");
        match decoded {
            GossipMessage::SolutionUpdate { from, problem_key, confidence, .. } => {
                assert_eq!(from, "pod_3");
                assert_eq!(problem_key, "maintenance_mode_clear");
                assert!((confidence - 0.95).abs() < f64::EPSILON);
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn seen_set_evicts_old_entries() {
        let mut seen = SeenSet::new();
        let key = ("pod_1".to_string(), 1u64);
        // Insert with expiry in the past
        seen.insert(key.clone(), Instant::now().checked_sub(Duration::from_secs(1)).unwrap_or(Instant::now()));
        // Evict
        let now = Instant::now();
        seen.retain(|_, exp| *exp > now);
        assert!(seen.is_empty(), "Expired entry should be evicted");
    }

    #[test]
    fn seen_set_dedup_blocks_duplicate() {
        let mut seen = SeenSet::new();
        let key = ("pod_2".to_string(), 99u64);
        seen.insert(key.clone(), Instant::now() + Duration::from_secs(SEEN_SET_TTL_SECS));
        assert!(seen.contains_key(&key), "Seen entry should block duplicate");
    }

    #[test]
    fn solution_update_with_long_fix_action_fits_mtu() {
        // Worst-case: long fix_action string
        let long_fix = "del C:\\RacingPoint\\MAINTENANCE_MODE && del C:\\RacingPoint\\GRACEFUL_RELAUNCH && del C:\\RacingPoint\\rcagent-restart-sentinel.txt && schtasks /Run /TN StartRCAgent".to_string();
        let msg = GossipMessage::SolutionUpdate {
            from: "pod_8".to_string(),
            seq: 9999,
            ts_unix: 9999999999,
            problem_key: "maintenance_mode_clear_restart".to_string(),
            problem_hash: "ffffffffffffffff".to_string(),
            fix_action: long_fix,
            confidence: 0.90,
            source_node: "pod_8".to_string(),
        };
        let bytes = serde_json::to_vec(&msg).expect("serialize long solution update");
        assert!(bytes.len() < MAX_UDP_PAYLOAD,
            "SolutionUpdate with realistic fix_action must fit MTU: {} bytes", bytes.len());
    }
}
