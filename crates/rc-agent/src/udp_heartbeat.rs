//! UDP Heartbeat Sender — runs on rc-agent alongside WebSocket.
//!
//! Sends ping packets to racecontrol every 2s, listens for pong responses.
//! If 3 pongs are missed (6s), signals the main loop to force-reconnect WebSocket.
//! Independent of TCP state — detects half-open connections faster.

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::net::UdpSocket;
use tokio::sync::mpsc;

use rc_common::udp_protocol::*;

const LOG_TARGET: &str = "udp";

/// Events from the UDP heartbeat to the main event loop
#[derive(Debug)]
pub enum HeartbeatEvent {
    /// Core is unreachable — force WebSocket reconnect
    CoreDead,
    /// Core responded — it's alive
    CoreAlive,
    /// Core told us to force reconnect WebSocket
    ForceReconnect,
    /// Core told us to restart
    ForceRestart,
}

/// Shared state that the main loop updates so the heartbeat can report accurate status
pub struct HeartbeatStatus {
    pub ws_connected: AtomicBool,
    pub game_running: AtomicBool,
    pub driving_active: AtomicBool,
    pub billing_active: AtomicBool,
    pub game_id: AtomicU32,
    /// Epoch-millis of last SwitchController received. 0 = no recent switch.
    /// self_monitor suppresses WS-dead relaunch for 60s after a switch.
    pub last_switch_ms: AtomicU64,
}

impl HeartbeatStatus {
    pub fn new() -> Self {
        Self {
            ws_connected: AtomicBool::new(false),
            game_running: AtomicBool::new(false),
            driving_active: AtomicBool::new(false),
            billing_active: AtomicBool::new(false),
            game_id: AtomicU32::new(0),
            last_switch_ms: AtomicU64::new(0),
        }
    }
}

/// Start the UDP heartbeat sender.
///
/// # Arguments
/// * `core_ip` - IP address of racecontrol (parsed from WebSocket URL)
/// * `pod_number` - This pod's number (1-8)
/// * `status` - Shared atomic state updated by the main loop
/// * `event_tx` - Channel to notify main loop of heartbeat events
pub async fn run(
    core_ip: String,
    pod_number: u8,
    status: Arc<HeartbeatStatus>,
    event_tx: mpsc::Sender<HeartbeatEvent>,
) {
    loop {
        if let Err(e) = run_inner(&core_ip, pod_number, &status, &event_tx).await {
            tracing::warn!(target: LOG_TARGET, "UDP heartbeat error: {} — restarting in 5s", e);
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    }
}

async fn run_inner(
    core_ip: &str,
    pod_number: u8,
    status: &Arc<HeartbeatStatus>,
    event_tx: &mpsc::Sender<HeartbeatEvent>,
) -> anyhow::Result<()> {
    // Bind to any available port (we're the client)
    let socket = UdpSocket::bind("0.0.0.0:0").await?;
    let core_addr = format!("{}:{}", core_ip, HEARTBEAT_PORT);
    socket.connect(&core_addr).await?;

    tracing::info!(target: LOG_TARGET, "UDP heartbeat started → {}", core_addr);

    let mut sequence: u32 = 0;
    let mut last_pong = Instant::now();
    let mut core_was_dead = false;
    let mut recv_buf = [0u8; 64];

    let ping_interval = Duration::from_secs(PING_INTERVAL_SECS);
    let dead_timeout = Duration::from_secs(DEAD_TIMEOUT_SECS);

    let mut ticker = tokio::time::interval(ping_interval);

    loop {
        tokio::select! {
            // Send ping on interval
            _ = ticker.tick() => {
                // Build status bitfield from shared atomics
                let mut pod_status = PodStatusBits::new();
                pod_status.set_ws_connected(status.ws_connected.load(Ordering::Relaxed));
                pod_status.set_game_running(status.game_running.load(Ordering::Relaxed));
                pod_status.set_driving_active(status.driving_active.load(Ordering::Relaxed));
                pod_status.set_billing_active(status.billing_active.load(Ordering::Relaxed));
                pod_status.set_game_id(status.game_id.load(Ordering::Relaxed) as u8);

                let ping = HeartbeatPing {
                    pod_number,
                    sequence,
                    status: pod_status,
                };

                // Fire and forget — UDP send should never block
                let _ = socket.send(&ping.to_bytes()).await;
                sequence = sequence.wrapping_add(1);

                // Check if core is dead (no pong for DEAD_TIMEOUT_SECS)
                if last_pong.elapsed() > dead_timeout {
                    if !core_was_dead {
                        tracing::warn!(
                            target: LOG_TARGET,
                            "UDP heartbeat: racecontrol unreachable for {}s — signaling reconnect",
                            DEAD_TIMEOUT_SECS
                        );
                        let _ = event_tx.send(HeartbeatEvent::CoreDead).await;
                        core_was_dead = true;
                    }
                }
            }

            // Receive pong from core
            result = socket.recv(&mut recv_buf) => {
                match result {
                    Ok(len) => {
                        if let Some(pong) = HeartbeatPong::from_bytes(&recv_buf[..len]) {
                            last_pong = Instant::now();

                            // If we thought core was dead, it's back
                            if core_was_dead {
                                tracing::info!(target: LOG_TARGET, "UDP heartbeat: racecontrol recovered");
                                let _ = event_tx.send(HeartbeatEvent::CoreAlive).await;
                                core_was_dead = false;
                            }

                            // Handle server flags
                            if pong.flags.force_reconnect() {
                                tracing::info!(target: LOG_TARGET, "UDP heartbeat: core requested force reconnect");
                                let _ = event_tx.send(HeartbeatEvent::ForceReconnect).await;
                            }
                            if pong.flags.force_restart() {
                                tracing::warn!(target: LOG_TARGET, "UDP heartbeat: core requested force restart");
                                let _ = event_tx.send(HeartbeatEvent::ForceRestart).await;
                            }
                        }
                    }
                    Err(e) => {
                        // ICMP port unreachable or similar — not fatal
                        tracing::debug!(target: LOG_TARGET, "UDP heartbeat recv error: {}", e);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::Ordering;

    #[test]
    fn heartbeat_status_last_switch_ms_defaults_to_zero() {
        let status = HeartbeatStatus::new();
        assert_eq!(status.last_switch_ms.load(Ordering::Relaxed), 0);
    }
}
