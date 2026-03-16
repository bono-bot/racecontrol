//! UDP Heartbeat Listener — runs on racecontrol alongside WebSocket.
//!
//! Receives ping packets from rc-agents every 2s, responds with pong.
//! Provides instant disconnect detection (6s) independent of TCP/WebSocket state.
//! Updates pod last_seen timestamps and can command agents to force-reconnect.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use tokio::net::UdpSocket;

use crate::state::AppState;
use rc_common::types::{DrivingState, GameState, PodStatus};
use rc_common::protocol::DashboardEvent;
use rc_common::udp_protocol::*;

/// Per-pod UDP tracking state
struct UdpPodState {
    last_ping: Instant,
    last_sequence: u32,
    /// Source address for sending pongs back
    addr: std::net::SocketAddr,
    /// Was this pod marked dead by UDP? (prevents repeated log spam)
    marked_dead: bool,
}

/// Spawn the UDP heartbeat listener as a background task.
pub fn spawn(state: Arc<AppState>) {
    tokio::spawn(async move {
        if let Err(e) = run(state).await {
            tracing::error!("UDP heartbeat listener exited with error: {}", e);
        }
    });
}

async fn run(state: Arc<AppState>) -> anyhow::Result<()> {
    let socket = UdpSocket::bind(format!("0.0.0.0:{}", HEARTBEAT_PORT)).await?;
    tracing::info!("UDP heartbeat listener started on port {}", HEARTBEAT_PORT);

    let mut buf = [0u8; 64]; // Ping is 12 bytes, generous buffer
    let mut pod_states: HashMap<u8, UdpPodState> = HashMap::new();

    // Staleness check interval — every 2 seconds
    let mut check_interval = tokio::time::interval(std::time::Duration::from_secs(PING_INTERVAL_SECS));

    loop {
        tokio::select! {
            // Receive ping from agent
            result = socket.recv_from(&mut buf) => {
                match result {
                    Ok((len, addr)) => {
                        if let Some(ping) = HeartbeatPing::from_bytes(&buf[..len]) {
                            handle_ping(&socket, &state, &mut pod_states, ping, addr).await;
                        }
                        // Silently ignore malformed packets
                    }
                    Err(e) => {
                        tracing::warn!("UDP heartbeat recv error: {}", e);
                    }
                }
            }

            // Periodic staleness check
            _ = check_interval.tick() => {
                check_stale_pods(&state, &mut pod_states).await;
            }
        }
    }
}

async fn handle_ping(
    socket: &UdpSocket,
    state: &Arc<AppState>,
    pod_states: &mut HashMap<u8, UdpPodState>,
    ping: HeartbeatPing,
    addr: std::net::SocketAddr,
) {
    let now = Instant::now();
    let pod_id = format!("pod_{}", ping.pod_number);

    // Update tracking state
    let ps = pod_states.entry(ping.pod_number).or_insert(UdpPodState {
        last_ping: now,
        last_sequence: 0,
        addr,
        marked_dead: false,
    });
    ps.last_ping = now;
    ps.last_sequence = ping.sequence;
    ps.addr = addr;

    // If pod was marked dead, it's back
    if ps.marked_dead {
        tracing::info!(
            "Pod {} UDP heartbeat recovered (seq={})",
            ping.pod_number, ping.sequence
        );
        ps.marked_dead = false;
    }

    // Update pod last_seen in shared state (lightweight — just timestamp)
    {
        let mut pods = state.pods.write().await;
        if let Some(pod) = pods.get_mut(&pod_id) {
            pod.last_seen = Some(chrono::Utc::now());
        }
    }

    // Build pong response
    let has_ws = state.agent_senders.read().await.contains_key(&pod_id);
    let mut flags = ServerFlags::new();
    flags.set_ws_expected(true);

    // If agent reports ws_connected=false but we also don't have a sender,
    // tell it to reconnect (nudge through a stuck state)
    if !ping.status.ws_connected() && !has_ws {
        flags.set_force_reconnect(true);
    }

    let pong = HeartbeatPong {
        pod_number: ping.pod_number,
        sequence: ping.sequence,
        server_timestamp: chrono::Utc::now().timestamp() as u32,
        flags,
    };

    let _ = socket.send_to(&pong.to_bytes(), addr).await;
}

async fn check_stale_pods(
    state: &Arc<AppState>,
    pod_states: &mut HashMap<u8, UdpPodState>,
) {
    let now = Instant::now();
    let dead_threshold = std::time::Duration::from_secs(DEAD_TIMEOUT_SECS);

    for (pod_num, ps) in pod_states.iter_mut() {
        if ps.marked_dead {
            continue; // Already handled
        }

        if now.duration_since(ps.last_ping) > dead_threshold {
            let pod_id = format!("pod_{}", pod_num);
            tracing::warn!(
                "Pod {} UDP heartbeat dead (no ping for {}s)",
                pod_num, DEAD_TIMEOUT_SECS
            );
            ps.marked_dead = true;

            // Mark pod offline immediately (much faster than pod_monitor)
            let mut pods = state.pods.write().await;
            if let Some(pod) = pods.get_mut(&pod_id) {
                if pod.status != PodStatus::Disabled && pod.status != PodStatus::Offline {
                    pod.status = PodStatus::Offline;
                    pod.driving_state = Some(DrivingState::NoDevice);

                    // Only reset game state if no active billing
                    let has_billing = state.billing.active_timers.read().await.contains_key(&pod_id);
                    if !has_billing {
                        pod.game_state = Some(GameState::Idle);
                        pod.current_game = None;
                    }

                    let _ = state.dashboard_tx.send(DashboardEvent::PodUpdate(pod.clone()));
                }
            }
        }
    }
}
