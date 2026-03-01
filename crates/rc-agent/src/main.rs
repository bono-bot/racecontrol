mod driving_detector;
mod sims;

use std::time::Duration;

use anyhow::Result;
use chrono::Utc;
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite::Message};

use driving_detector::{
    DetectorConfig, DetectorSignal, DrivingDetector,
    is_input_active, is_steering_moving, parse_openffboard_report,
};
use rc_common::protocol::AgentMessage;
use rc_common::types::*;
use sims::SimAdapter;
use sims::assetto_corsa::AssettoCorsaAdapter;

#[derive(Debug, Deserialize)]
struct AgentConfig {
    pod: PodConfig,
    core: CoreConfig,
    #[serde(default)]
    wheelbase: WheelbaseConfig,
    #[serde(default)]
    telemetry_ports: TelemetryPortsConfig,
}

#[derive(Debug, Deserialize)]
struct PodConfig {
    number: u32,
    name: String,
    sim: String,
    #[serde(default = "default_sim_ip")]
    sim_ip: String,
    #[serde(default = "default_sim_port")]
    sim_port: u16,
}

#[derive(Debug, Deserialize)]
struct CoreConfig {
    #[serde(default = "default_core_url")]
    url: String,
}

#[derive(Debug, Deserialize)]
struct WheelbaseConfig {
    #[serde(default = "default_wheelbase_vid")]
    vendor_id: u16,
    #[serde(default = "default_wheelbase_pid")]
    product_id: u16,
}

impl Default for WheelbaseConfig {
    fn default() -> Self {
        Self {
            vendor_id: default_wheelbase_vid(),
            product_id: default_wheelbase_pid(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct TelemetryPortsConfig {
    #[serde(default = "default_telemetry_ports")]
    ports: Vec<u16>,
}

impl Default for TelemetryPortsConfig {
    fn default() -> Self {
        Self {
            ports: default_telemetry_ports(),
        }
    }
}

fn default_sim_ip() -> String { "127.0.0.1".to_string() }
fn default_sim_port() -> u16 { 9996 }
fn default_core_url() -> String { "ws://127.0.0.1:8080/ws/agent".to_string() }
fn default_wheelbase_vid() -> u16 { 0x1209 }
fn default_wheelbase_pid() -> u16 { 0xFFB0 }
fn default_telemetry_ports() -> Vec<u16> { vec![9996, 20777, 5300, 6789, 5555] }

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "rc_agent=info".into()),
        )
        .init();

    println!(r#"
  RaceControl Agent
  Pod Telemetry Bridge
"#);

    // Load config
    let config = load_config()?;
    tracing::info!("Pod #{}: {} (sim: {})", config.pod.number, config.pod.name, config.pod.sim);
    tracing::info!("Core server: {}", config.core.url);

    let pod_id = uuid::Uuid::new_v4().to_string();
    let sim_type = match config.pod.sim.as_str() {
        "assetto_corsa" | "ac" => SimType::AssettocCorsa,
        "iracing" => SimType::IRacing,
        "lmu" | "le_mans_ultimate" => SimType::LeMansUltimate,
        other => {
            tracing::error!("Unknown sim type: {}", other);
            return Ok(());
        }
    };

    // Build pod info
    let pod_info = PodInfo {
        id: pod_id.clone(),
        number: config.pod.number,
        name: config.pod.name.clone(),
        ip_address: local_ip(),
        sim_type,
        status: PodStatus::Idle,
        current_driver: None,
        current_session_id: None,
        last_seen: Some(Utc::now()),
        driving_state: None,
        billing_session_id: None,
    };

    // Connect to core server
    tracing::info!("Connecting to RaceControl core at {}...", config.core.url);
    let (ws_stream, _) = connect_async(&config.core.url).await?;
    let (mut ws_tx, mut ws_rx) = ws_stream.split();
    tracing::info!("Connected to core server");

    // Register this pod
    let register_msg = AgentMessage::Register(pod_info.clone());
    let json = serde_json::to_string(&register_msg)?;
    ws_tx.send(Message::Text(json.into())).await?;
    tracing::info!("Registered as Pod #{}", config.pod.number);

    // Create sim adapter
    let mut adapter: Box<dyn SimAdapter> = match sim_type {
        SimType::AssettocCorsa => Box::new(AssettoCorsaAdapter::new(
            pod_id.clone(),
            config.pod.sim_ip.clone(),
            config.pod.sim_port,
        )),
        _ => {
            tracing::warn!("Sim adapter not yet implemented for {:?}, running in idle mode", sim_type);
            // Run heartbeat loop only
            loop {
                let hb = AgentMessage::Heartbeat(PodInfo {
                    last_seen: Some(Utc::now()),
                    ..pod_info.clone()
                });
                let json = serde_json::to_string(&hb)?;
                if ws_tx.send(Message::Text(json.into())).await.is_err() {
                    break;
                }
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
            return Ok(());
        }
    };

    // Set up driving detector (USB HID + UDP)
    let detector_config = DetectorConfig {
        wheelbase_vid: config.wheelbase.vendor_id,
        wheelbase_pid: config.wheelbase.product_id,
        telemetry_ports: config.telemetry_ports.ports.clone(),
        ..DetectorConfig::default()
    };
    let mut detector = DrivingDetector::new(&detector_config);

    // Channel for detector signals from HID/UDP tasks
    let (signal_tx, mut signal_rx) = mpsc::channel::<DetectorSignal>(256);

    // Spawn USB HID wheelbase monitor (blocking I/O in spawn_blocking)
    let hid_signal_tx = signal_tx.clone();
    let hid_vid = config.wheelbase.vendor_id;
    let hid_pid = config.wheelbase.product_id;
    let hid_pedal_threshold = detector_config.pedal_threshold;
    let hid_steering_deadzone = detector_config.steering_deadzone;
    tokio::spawn(async move {
        run_hid_monitor(hid_vid, hid_pid, hid_pedal_threshold, hid_steering_deadzone, hid_signal_tx).await;
    });

    // Spawn UDP telemetry port listeners
    let udp_signal_tx = signal_tx.clone();
    let udp_ports = config.telemetry_ports.ports.clone();
    tokio::spawn(async move {
        run_udp_monitor(udp_ports, udp_signal_tx).await;
    });

    // Try to connect to sim (for telemetry/laps — separate from billing detection)
    match adapter.connect() {
        Ok(()) => tracing::info!("Connected to {} telemetry", sim_type),
        Err(e) => {
            tracing::warn!("Could not connect to sim: {}. Will retry...", e);
        }
    }

    // Main loop
    let mut heartbeat_interval = tokio::time::interval(Duration::from_secs(5));
    let mut telemetry_interval = tokio::time::interval(Duration::from_millis(100));
    let mut detector_interval = tokio::time::interval(Duration::from_millis(100));
    let mut last_lap_count: u32 = 0;

    loop {
        tokio::select! {
            _ = heartbeat_interval.tick() => {
                let hb = AgentMessage::Heartbeat(PodInfo {
                    status: if adapter.is_connected() { PodStatus::InSession } else { PodStatus::Idle },
                    last_seen: Some(Utc::now()),
                    driving_state: Some(detector.state()),
                    ..pod_info.clone()
                });
                let json = serde_json::to_string(&hb)?;
                if ws_tx.send(Message::Text(json.into())).await.is_err() {
                    tracing::error!("Lost connection to core server");
                    break;
                }
            }
            _ = telemetry_interval.tick() => {
                if !adapter.is_connected() {
                    let _ = adapter.connect();
                    continue;
                }

                match adapter.read_telemetry() {
                    Ok(Some(frame)) => {
                        // Check for lap completion
                        if frame.lap_number > last_lap_count && last_lap_count > 0 {
                            let lap = LapData {
                                id: uuid::Uuid::new_v4().to_string(),
                                session_id: String::new(),
                                driver_id: String::new(),
                                pod_id: pod_id.clone(),
                                sim_type,
                                track: frame.track.clone(),
                                car: frame.car.clone(),
                                lap_number: last_lap_count,
                                lap_time_ms: frame.lap_time_ms,
                                sector1_ms: None,
                                sector2_ms: None,
                                sector3_ms: None,
                                valid: true,
                                created_at: Utc::now(),
                            };
                            let msg = AgentMessage::LapCompleted(lap);
                            let json = serde_json::to_string(&msg)?;
                            let _ = ws_tx.send(Message::Text(json.into())).await;
                        }
                        last_lap_count = frame.lap_number;

                        // Send telemetry frame
                        let msg = AgentMessage::Telemetry(frame);
                        let json = serde_json::to_string(&msg)?;
                        let _ = ws_tx.send(Message::Text(json.into())).await;
                    }
                    Ok(None) => {}
                    Err(e) => {
                        tracing::warn!("Telemetry read error: {}", e);
                        adapter.disconnect();
                    }
                }
            }
            // Process driving detector signals from HID/UDP tasks
            Some(signal) = signal_rx.recv() => {
                let (_, changed) = detector.process_signal(signal);
                if changed {
                    let msg = AgentMessage::DrivingStateUpdate {
                        pod_id: pod_id.clone(),
                        state: detector.state(),
                    };
                    let json = serde_json::to_string(&msg)?;
                    let _ = ws_tx.send(Message::Text(json.into())).await;
                    tracing::info!("Driving state changed: {:?}", detector.state());
                }
            }
            // Periodic detector evaluation (catches idle timeout transitions)
            _ = detector_interval.tick() => {
                let (_, changed) = detector.evaluate_state();
                if changed {
                    let msg = AgentMessage::DrivingStateUpdate {
                        pod_id: pod_id.clone(),
                        state: detector.state(),
                    };
                    let json = serde_json::to_string(&msg)?;
                    let _ = ws_tx.send(Message::Text(json.into())).await;
                    tracing::info!("Driving state changed (timeout): {:?}", detector.state());
                }
            }
            msg = ws_rx.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        tracing::debug!("Received from core: {}", text);
                        // Handle billing messages from core
                        if let Ok(core_msg) = serde_json::from_str::<rc_common::protocol::CoreToAgentMessage>(&text) {
                            match core_msg {
                                rc_common::protocol::CoreToAgentMessage::BillingStarted { billing_session_id, driver_name, allocated_seconds } => {
                                    tracing::info!("Billing started: {} for {} ({}s)", billing_session_id, driver_name, allocated_seconds);
                                }
                                rc_common::protocol::CoreToAgentMessage::BillingStopped { billing_session_id } => {
                                    tracing::info!("Billing stopped: {}", billing_session_id);
                                }
                                _ => {}
                            }
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => {
                        tracing::info!("Core server closed connection");
                        break;
                    }
                    _ => {}
                }
            }
        }
    }

    adapter.disconnect();
    Ok(())
}

fn load_config() -> Result<AgentConfig> {
    let paths = ["rc-agent.toml", "/etc/racecontrol/rc-agent.toml"];
    for path in paths {
        if let Ok(content) = std::fs::read_to_string(path) {
            let config: AgentConfig = toml::from_str(&content)?;
            tracing::info!("Loaded config from {}", path);
            return Ok(config);
        }
    }

    // Default config
    tracing::warn!("No config file found, using defaults");
    Ok(AgentConfig {
        pod: PodConfig {
            number: 1,
            name: "Pod 01".to_string(),
            sim: "assetto_corsa".to_string(),
            sim_ip: default_sim_ip(),
            sim_port: default_sim_port(),
        },
        core: CoreConfig {
            url: default_core_url(),
        },
        wheelbase: WheelbaseConfig::default(),
        telemetry_ports: TelemetryPortsConfig::default(),
    })
}

fn local_ip() -> String {
    // Simple local IP detection
    std::net::UdpSocket::bind("0.0.0.0:0")
        .and_then(|s| {
            s.connect("8.8.8.8:80")?;
            s.local_addr()
        })
        .map(|addr| addr.ip().to_string())
        .unwrap_or_else(|_| "127.0.0.1".to_string())
}

/// USB HID wheelbase monitor — runs in a spawned task.
/// Reads input reports from the Conspit wheelbase and sends signals to the detector.
async fn run_hid_monitor(
    vid: u16,
    pid: u16,
    pedal_threshold: f32,
    steering_deadzone: f32,
    signal_tx: mpsc::Sender<DetectorSignal>,
) {
    let config = DetectorConfig {
        wheelbase_vid: vid,
        wheelbase_pid: pid,
        pedal_threshold,
        steering_deadzone,
        ..DetectorConfig::default()
    };

    loop {
        // Try to open the HID device (blocking operation)
        let result = tokio::task::spawn_blocking(move || {
            hidapi::HidApi::new()
        })
        .await;

        let api = match result {
            Ok(Ok(api)) => api,
            Ok(Err(e)) => {
                tracing::warn!("Failed to initialize HID API: {}", e);
                let _ = signal_tx.send(DetectorSignal::HidDisconnected).await;
                tokio::time::sleep(Duration::from_secs(5)).await;
                continue;
            }
            Err(e) => {
                tracing::error!("HID task panic: {}", e);
                tokio::time::sleep(Duration::from_secs(5)).await;
                continue;
            }
        };

        // Try to find and open the wheelbase
        let device = api.open(vid, pid);
        match device {
            Ok(dev) => {
                tracing::info!(
                    "Connected to wheelbase HID device (VID:{:#06x} PID:{:#06x})",
                    vid, pid
                );
                dev.set_blocking_mode(false).ok();

                let mut prev_steering: f32 = 0.0;
                let mut buf = [0u8; 64];

                loop {
                    match dev.read_timeout(&mut buf, 10) {
                        Ok(len) if len > 0 => {
                            if let Some(input) = parse_openffboard_report(&buf[..len]) {
                                let pedal_active = is_input_active(&input, &config);
                                let steer_moving =
                                    is_steering_moving(input.steering, prev_steering, 0.005);
                                prev_steering = input.steering;

                                let signal = if pedal_active || steer_moving {
                                    DetectorSignal::HidActive
                                } else {
                                    DetectorSignal::HidIdle
                                };
                                if signal_tx.send(signal).await.is_err() {
                                    return; // Main loop exited
                                }
                            }
                        }
                        Ok(_) => {
                            // No data (non-blocking)
                            tokio::time::sleep(Duration::from_millis(10)).await;
                        }
                        Err(e) => {
                            tracing::warn!("HID read error: {}", e);
                            let _ = signal_tx.send(DetectorSignal::HidDisconnected).await;
                            break;
                        }
                    }
                }
            }
            Err(_) => {
                tracing::debug!(
                    "Wheelbase HID device not found (VID:{:#06x} PID:{:#06x}), retrying...",
                    vid, pid
                );
                let _ = signal_tx.send(DetectorSignal::HidDisconnected).await;
            }
        }

        // Retry after delay
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}

/// UDP telemetry port monitor — listens on multiple game telemetry ports.
/// If any data arrives on any port, signals that a game is actively outputting telemetry.
async fn run_udp_monitor(ports: Vec<u16>, signal_tx: mpsc::Sender<DetectorSignal>) {
    use tokio::net::UdpSocket;

    // Spawn a listener task per port — each sends UdpActive signals independently
    for port in ports {
        let tx = signal_tx.clone();
        tokio::spawn(async move {
            let sock = match UdpSocket::bind(format!("0.0.0.0:{}", port)).await {
                Ok(s) => {
                    tracing::info!("Listening for game telemetry on UDP port {}", port);
                    s
                }
                Err(e) => {
                    tracing::warn!(
                        "Could not bind UDP port {}: {} (game may already be using it)",
                        port, e
                    );
                    return;
                }
            };

            let mut buf = [0u8; 2048];
            loop {
                match sock.recv_from(&mut buf).await {
                    Ok((len, _)) if len > 0 => {
                        if tx.send(DetectorSignal::UdpActive).await.is_err() {
                            return;
                        }
                    }
                    Ok(_) => {}
                    Err(e) => {
                        tracing::warn!("UDP recv error on port {}: {}", port, e);
                        tokio::time::sleep(Duration::from_secs(1)).await;
                    }
                }
            }
        });
    }

    // Keep this task alive; the per-port tasks do the actual work.
    // Send periodic UdpIdle signals so the detector knows UDP monitoring is running
    // but no data is arriving (if no per-port tasks have sent UdpActive recently).
    loop {
        tokio::time::sleep(Duration::from_secs(3)).await;
        if signal_tx.send(DetectorSignal::UdpIdle).await.is_err() {
            return;
        }
    }
}
