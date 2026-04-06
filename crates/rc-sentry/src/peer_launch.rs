//! Peer Launch — TCP coordinated launch for multiplayer sessions (Phase 324).
//!
//! Binds TCP :8093 as listener. When a multiplayer session is detected,
//! the lowest-numbered pod becomes initiator and connects to all session peers.
//!
//! Two-phase protocol:
//!   Initiator: connect all -> send READY {launch_at_ms} -> collect ACKs (200ms timeout) -> send LAUNCH
//!   Listener: receive READY -> send ACK -> receive LAUNCH -> execute local launch
//!
//! Pure std::net -- no tokio, no async.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub const LAUNCH_PORT: u16 = 8093;
pub const READY_TIMEOUT_MS: u64 = 200;
const LOG_TARGET: &str = "peer-launch";
const MAX_MSG_SIZE: usize = 1024;

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
#[serde(tag = "type")]
pub enum LaunchMessage {
    Ready {
        session_id: String,
        launch_at_ms: u64,
        from: String,
    },
    Ack {
        session_id: String,
        from: String,
    },
    Launch {
        session_id: String,
    },
}

/// Determine if this node is the initiator (lowest pod number in session).
pub fn is_initiator(my_node_id: &str, session_peers: &[String]) -> bool {
    let my_num = pod_number(my_node_id);
    session_peers.iter().all(|p| pod_number(p) >= my_num)
}

fn pod_number(node_id: &str) -> u32 {
    // Extract trailing number from "pod_N" or "pod-N"
    node_id
        .chars()
        .rev()
        .take_while(|c| c.is_ascii_digit())
        .collect::<String>()
        .chars()
        .rev()
        .collect::<String>()
        .parse()
        .unwrap_or(u32::MAX)
}

fn unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Spawn the TCP launch listener on the given port.
/// Returns a receiver for incoming launch signals (session_id strings).
/// The caller must handle launch signals by triggering the local game launch.
pub fn spawn_listener(
    port: u16,
    my_node_id: String,
    shutdown: &'static std::sync::atomic::AtomicBool,
) -> mpsc::Receiver<String> {
    let (tx, rx) = mpsc::channel();
    std::thread::Builder::new()
        .name("peer-launch-listener".to_string())
        .spawn(move || run_listener(port, my_node_id, tx, shutdown))
        .expect("spawn peer-launch-listener");
    rx
}

fn run_listener(
    port: u16,
    my_node_id: String,
    tx: mpsc::Sender<String>,
    shutdown: &std::sync::atomic::AtomicBool,
) {
    let listener = match TcpListener::bind(format!("0.0.0.0:{}", port)) {
        Ok(l) => l,
        Err(e) => {
            tracing::error!(target: LOG_TARGET, port, "TCP launch listener bind failed: {e}");
            return;
        }
    };
    if let Err(e) = listener.set_nonblocking(true) {
        tracing::warn!(target: LOG_TARGET, "set_nonblocking failed: {e}");
    }
    tracing::info!(target: LOG_TARGET, port, "TCP launch listener bound");

    loop {
        if shutdown.load(Ordering::Acquire) {
            tracing::info!(target: LOG_TARGET, "launch listener shutdown");
            return;
        }
        match listener.accept() {
            Ok((stream, addr)) => {
                tracing::debug!(target: LOG_TARGET, peer = %addr, "launch TCP connection");
                if let Err(e) = stream.set_nonblocking(false) {
                    tracing::warn!(target: LOG_TARGET, "set_nonblocking(false) failed: {e}");
                }
                let _ = stream.set_read_timeout(Some(Duration::from_millis(READY_TIMEOUT_MS + 100)));

                let tx2 = tx.clone();
                let node_id = my_node_id.clone();
                std::thread::Builder::new()
                    .name("peer-launch-handler".to_string())
                    .spawn(move || handle_incoming(stream, addr, node_id, tx2))
                    .ok();
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(e) => tracing::warn!(target: LOG_TARGET, "accept error: {e}"),
        }
    }
}

fn handle_incoming(
    mut stream: TcpStream,
    addr: SocketAddr,
    my_node_id: String,
    tx: mpsc::Sender<String>,
) {
    let mut buf = vec![0u8; MAX_MSG_SIZE];
    let n = match stream.read(&mut buf) {
        Ok(n) => n,
        Err(e) => {
            tracing::warn!(target: LOG_TARGET, peer = %addr, "read error: {e}");
            return;
        }
    };

    let msg: LaunchMessage = match serde_json::from_slice(&buf[..n]) {
        Ok(m) => m,
        Err(e) => {
            tracing::warn!(target: LOG_TARGET, peer = %addr, "invalid launch JSON: {e}");
            return;
        }
    };

    match msg {
        LaunchMessage::Ready {
            ref session_id,
            launch_at_ms,
            ref from,
        } => {
            tracing::info!(
                target: LOG_TARGET,
                session_id = %session_id,
                launch_at_ms,
                initiator = %from,
                "received READY -- sending ACK"
            );
            let ack = LaunchMessage::Ack {
                session_id: session_id.clone(),
                from: my_node_id,
            };
            if let Ok(ack_bytes) = serde_json::to_vec(&ack) {
                let _ = stream.write_all(&ack_bytes);
            }

            // Wait for LAUNCH signal on same connection
            let mut launch_buf = vec![0u8; MAX_MSG_SIZE];
            let _ = stream.set_read_timeout(Some(Duration::from_millis(500)));
            match stream.read(&mut launch_buf) {
                Ok(n) if n > 0 => {
                    if let Ok(LaunchMessage::Launch { session_id: sid }) =
                        serde_json::from_slice(&launch_buf[..n])
                    {
                        tracing::info!(
                            target: LOG_TARGET,
                            session_id = %sid,
                            "received LAUNCH -- signaling local launch"
                        );
                        let _ = tx.send(sid);
                    }
                }
                _ => {
                    // Timeout or error -- still launch (degrade gracefully)
                    tracing::warn!(
                        target: LOG_TARGET,
                        session_id = %session_id,
                        "LAUNCH signal timeout -- launching anyway"
                    );
                    let _ = tx.send(session_id.clone());
                }
            }
        }
        LaunchMessage::Launch { ref session_id } => {
            // Direct LAUNCH (no READY phase) -- unusual but handle gracefully
            tracing::info!(target: LOG_TARGET, session_id = %session_id, "received direct LAUNCH");
            let _ = tx.send(session_id.clone());
        }
        LaunchMessage::Ack { .. } => {
            tracing::debug!(target: LOG_TARGET, "received unexpected ACK on listener side");
        }
    }
}

/// Initiate coordinated launch as the coordinator pod.
/// Connects to all session peer IPs on LAUNCH_PORT, sends READY, collects ACKs,
/// sends LAUNCH, then returns (caller executes local launch).
///
/// Returns Ok(()) if all peers ACK'd or timeout was reached gracefully.
/// Returns Err if unable to connect to any peer (all unresponsive).
pub fn initiate_launch(
    session_id: &str,
    my_node_id: &str,
    peer_ips: &[&str],
    launch_port: u16,
) -> Result<(), String> {
    let launch_at_ms = unix_ms() + READY_TIMEOUT_MS + 50; // 50ms padding after ACK window
    let mut connected: Vec<TcpStream> = Vec::new();

    // Phase 1: Connect and send READY to all peers
    for peer_ip in peer_ips {
        let addr = format!("{}:{}", peer_ip, launch_port);
        let sock_addr: SocketAddr = match addr.parse() {
            Ok(a) => a,
            Err(e) => {
                tracing::warn!(target: LOG_TARGET, peer = %addr, "invalid peer addr: {e}");
                continue;
            }
        };
        match TcpStream::connect_timeout(&sock_addr, Duration::from_millis(READY_TIMEOUT_MS)) {
            Ok(mut stream) => {
                let _ = stream.set_write_timeout(Some(Duration::from_millis(100)));
                let _ = stream.set_read_timeout(Some(Duration::from_millis(READY_TIMEOUT_MS)));
                let ready = LaunchMessage::Ready {
                    session_id: session_id.to_string(),
                    launch_at_ms,
                    from: my_node_id.to_string(),
                };
                match serde_json::to_vec(&ready) {
                    Ok(bytes) => {
                        if stream.write_all(&bytes).is_ok() {
                            connected.push(stream);
                            tracing::debug!(target: LOG_TARGET, peer = %addr, "READY sent");
                        } else {
                            tracing::warn!(target: LOG_TARGET, peer = %addr, "READY write failed");
                        }
                    }
                    Err(e) => {
                        tracing::warn!(target: LOG_TARGET, "READY serialize failed: {e}");
                    }
                }
            }
            Err(e) => {
                tracing::warn!(target: LOG_TARGET, peer = %addr, "connect failed: {e}");
            }
        }
    }

    if connected.is_empty() && !peer_ips.is_empty() {
        return Err(format!("No peers reachable for session {}", session_id));
    }

    // Phase 2: Collect ACKs from connected peers (best effort within timeout)
    let ack_deadline = std::time::Instant::now() + Duration::from_millis(READY_TIMEOUT_MS);
    let mut acked = 0usize;
    for stream in &mut connected {
        let remaining = ack_deadline.saturating_duration_since(std::time::Instant::now());
        if remaining.is_zero() {
            break;
        }
        let _ = stream.set_read_timeout(Some(remaining.max(Duration::from_millis(10))));
        let mut buf = vec![0u8; MAX_MSG_SIZE];
        match stream.read(&mut buf) {
            Ok(n) if n > 0 => {
                if let Ok(LaunchMessage::Ack {
                    session_id: sid,
                    from,
                }) = serde_json::from_slice(&buf[..n])
                {
                    tracing::info!(target: LOG_TARGET, from = %from, session_id = %sid, "received ACK");
                    acked += 1;
                }
            }
            _ => tracing::warn!(target: LOG_TARGET, "ACK timeout or error from peer"),
        }
    }

    tracing::info!(
        target: LOG_TARGET,
        session_id,
        acked,
        total = connected.len(),
        "ACK collection complete -- sending LAUNCH"
    );

    // Phase 3: Send LAUNCH to all connected peers
    let launch_msg = LaunchMessage::Launch {
        session_id: session_id.to_string(),
    };
    if let Ok(launch_bytes) = serde_json::to_vec(&launch_msg) {
        for stream in &mut connected {
            let _ = stream.write_all(&launch_bytes);
        }
    }

    tracing::info!(
        target: LOG_TARGET,
        session_id,
        "LAUNCH sent to all peers -- coordinated launch complete"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn launch_message_ready_roundtrip() {
        let msg = LaunchMessage::Ready {
            session_id: "f125-session-abc123".to_string(),
            launch_at_ms: 1712345678900,
            from: "pod_3".to_string(),
        };
        let json = serde_json::to_string(&msg).expect("serialize Ready");
        let decoded: LaunchMessage = serde_json::from_str(&json).expect("deserialize Ready");
        match decoded {
            LaunchMessage::Ready {
                session_id,
                launch_at_ms,
                from,
            } => {
                assert_eq!(session_id, "f125-session-abc123");
                assert_eq!(launch_at_ms, 1712345678900u64);
                assert_eq!(from, "pod_3");
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn launch_message_ack_roundtrip() {
        let msg = LaunchMessage::Ack {
            session_id: "test-sess".to_string(),
            from: "pod_5".to_string(),
        };
        let json = serde_json::to_string(&msg).expect("serialize Ack");
        let decoded: LaunchMessage = serde_json::from_str(&json).expect("deserialize Ack");
        match decoded {
            LaunchMessage::Ack { session_id, from } => {
                assert_eq!(session_id, "test-sess");
                assert_eq!(from, "pod_5");
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn launch_message_launch_roundtrip() {
        let msg = LaunchMessage::Launch {
            session_id: "sess-xyz".to_string(),
        };
        let json = serde_json::to_string(&msg).expect("serialize Launch");
        let decoded: LaunchMessage = serde_json::from_str(&json).expect("deserialize Launch");
        match decoded {
            LaunchMessage::Launch { session_id } => {
                assert_eq!(session_id, "sess-xyz");
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn initiator_selection_lowest_pod_number() {
        let peers = vec![
            "pod_3".to_string(),
            "pod_5".to_string(),
            "pod_7".to_string(),
        ];
        assert!(
            is_initiator("pod_3", &peers),
            "Lowest-numbered pod should be initiator"
        );
        assert!(
            !is_initiator("pod_5", &peers),
            "Higher-numbered pod should not be initiator"
        );
    }

    #[test]
    fn initiator_selection_single_peer() {
        let peers = vec!["pod_5".to_string()];
        assert!(
            is_initiator("pod_5", &peers),
            "Only pod in session should be initiator"
        );
    }

    #[test]
    fn connect_timeout_duration() {
        // Verify the constant is reasonable for LAN
        assert!(
            READY_TIMEOUT_MS <= 200,
            "READY_TIMEOUT_MS must be <= 200 for <500ms total budget"
        );
        assert!(
            READY_TIMEOUT_MS >= 50,
            "READY_TIMEOUT_MS must be >= 50 to allow LAN round-trip"
        );
    }

    #[test]
    fn pod_number_extraction() {
        assert_eq!(pod_number("pod_3"), 3);
        assert_eq!(pod_number("pod_12"), 12);
        assert_eq!(pod_number("pod-5"), 5);
        assert_eq!(pod_number("unknown"), u32::MAX);
        assert_eq!(pod_number(""), u32::MAX);
    }

    #[test]
    fn launch_message_ready_json_has_type_tag() {
        let msg = LaunchMessage::Ready {
            session_id: "test".to_string(),
            launch_at_ms: 0,
            from: "pod_1".to_string(),
        };
        let json = serde_json::to_string(&msg).expect("serialize");
        assert!(
            json.contains(r#""type":"Ready"#),
            "JSON must contain type discriminator"
        );
    }
}
