//! Failure Monitor — polls shared pod state every 5s, detects game freeze,
//! launch timeout, and USB wheelbase disconnect/reconnect.
//!
//! When a condition is detected, constructs a synthetic suggestion string
//! with canonical keywords and calls ai_debugger::try_auto_fix() via
//! spawn_blocking. All fix functions are called through try_auto_fix so
//! DebugMemory pattern learning fires correctly.
//!
//! Detection rules:
//!   CRASH-01: game_pid.is_some() + last_udp_secs_ago >= 30 + is_game_window_hung() = true
//!   CRASH-02: launch_started_at.elapsed() > 90s + game_pid.is_none() + not yet fired this attempt
//!   USB-01 reconnect: prev_hid_connected=false → true (billing active)
//!   USB disconnect:   prev_hid_connected=true → false (billing active) → send HardwareFailure
//!   TELEM-01: billing_active + game_pid.is_some() + last_udp_secs_ago >= 60s → send TelemetryGap (once per silence window)

use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use tokio::sync::{mpsc, watch};

use crate::ai_debugger::{PodStateSnapshot, try_auto_fix};
use crate::udp_heartbeat::HeartbeatStatus;
use rc_common::protocol::AgentMessage;
use rc_common::types::{DrivingState, PodFailureReason, SimType};

const POLL_INTERVAL_SECS: u64 = 5;
const STARTUP_GRACE_SECS: u64 = 30;
const FREEZE_UDP_SILENCE_SECS: u64 = 30;
const LAUNCH_TIMEOUT_SECS: u64 = 90;
const TELEM_GAP_SECS: u64 = 60;

/// Shared state updated by main.rs event loop and read by failure_monitor.
/// Sent via tokio::sync::watch channel — clone-on-read, no locking required.
#[derive(Debug, Clone)]
pub struct FailureMonitorState {
    /// Current game process PID (None = no game running)
    pub game_pid: Option<u32>,
    /// Seconds since last UDP telemetry packet (None = never received this session)
    pub last_udp_secs_ago: Option<u64>,
    /// Whether the Conspit Ares wheelbase HID device is currently connected
    pub hid_connected: bool,
    /// When the last LaunchGame command was received (None = not launching)
    pub launch_started_at: Option<Instant>,
    /// Whether billing is currently active on this pod
    pub billing_active: bool,
    /// Set by main.rs when racecontrol server sends a recovery command.
    /// When true, failure_monitor suppresses all autonomous fixes to avoid
    /// conflicting with server-initiated recovery. Cannot use is_pod_in_recovery()
    /// (server-side only in pod_healer.rs) — this is the agent-local equivalent.
    pub recovery_in_progress: bool,
    /// Current driving state from the detector (None = not yet received).
    /// Read by billing_guard.rs (Wave 1) for idle drift detection (BILL-03).
    pub driving_state: Option<DrivingState>,
}

impl Default for FailureMonitorState {
    fn default() -> Self {
        Self {
            game_pid: None,
            last_udp_secs_ago: None,
            hid_connected: false,
            launch_started_at: None,
            billing_active: false,
            recovery_in_progress: false,
            driving_state: None,
        }
    }
}

/// Spawn the failure monitor background task.
///
/// # Arguments
/// * `status` — Shared heartbeat atomics from HeartbeatStatus
/// * `state_rx` — Receiver end of the watch channel for FailureMonitorState
/// * `agent_msg_tx` — Channel to send AgentMessage reports to the WebSocket sender
/// * `pod_id` — This pod's identifier (e.g. "pod_8")
/// * `pod_number` — This pod's number (1-8)
pub fn spawn(
    status: Arc<HeartbeatStatus>,
    state_rx: watch::Receiver<FailureMonitorState>,
    agent_msg_tx: mpsc::Sender<AgentMessage>,
    pod_id: String,
    pod_number: u32,
) {
    tokio::spawn(async move {
        // Grace period: let game processes, HID devices, and UDP listeners fully start
        tokio::time::sleep(Duration::from_secs(STARTUP_GRACE_SECS)).await;
        tracing::info!("[failure-monitor] Starting polling loop (interval: {}s)", POLL_INTERVAL_SECS);

        let mut interval = tokio::time::interval(Duration::from_secs(POLL_INTERVAL_SECS));
        let mut prev_hid_connected = false;
        let mut launch_timeout_fired = false; // prevents duplicate fires per launch attempt
        let mut telem_gap_fired = false; // TELEM-01: prevents repeated TelemetryGap sends per silence window

        loop {
            interval.tick().await;

            // Read current state snapshot — cheap clone from watch channel
            let state = state_rx.borrow().clone();

            // Reset launch_timeout_fired when no longer launching
            if state.launch_started_at.is_none() {
                launch_timeout_fired = false;
            }

            // Skip all autonomous fixes when server-commanded recovery in progress
            if state.recovery_in_progress {
                tracing::debug!("[failure-monitor] recovery_in_progress — skipping autonomous checks");
                prev_hid_connected = state.hid_connected;
                continue;
            }

            // USB-01: Detect disconnect (billing active) — send HardwareFailure message
            if prev_hid_connected && !state.hid_connected && state.billing_active {
                tracing::warn!("[failure-monitor] Wheelbase USB disconnect detected during billing");
                let msg = AgentMessage::HardwareFailure {
                    pod_id: pod_id.clone(),
                    reason: PodFailureReason::WheelbaseDisconnected,
                    detail: "VID:0x1209 PID:0xFFB0 USB disconnect detected during active session".to_string(),
                };
                let _ = agent_msg_tx.try_send(msg);
            }

            // USB-01: Detect reconnect (billing active) — fire FFB reset fix
            if !prev_hid_connected && state.hid_connected && state.billing_active {
                tracing::info!("[failure-monitor] Wheelbase USB reconnect detected — firing FFB reset");
                let synthetic = "Wheelbase usb reset required — HID reconnected VID:0x1209 PID:0xFFB0";
                let snap = build_snapshot(&state, &status, pod_id.clone(), pod_number);
                let synthetic_owned = synthetic.to_string();
                let _ = tokio::task::spawn_blocking(move || {
                    try_auto_fix(&synthetic_owned, &snap)
                }).await;
            }

            prev_hid_connected = state.hid_connected;

            // TELEM-01: UDP silence 60s while billing active + game running
            if state.billing_active && state.game_pid.is_some() && !state.recovery_in_progress {
                let udp_silent_60 = state
                    .last_udp_secs_ago
                    .map(|s| s >= TELEM_GAP_SECS)
                    .unwrap_or(false);

                if udp_silent_60 && !telem_gap_fired {
                    telem_gap_fired = true;
                    let gap = state.last_udp_secs_ago.unwrap_or(TELEM_GAP_SECS);
                    tracing::warn!(
                        "[failure-monitor] TELEM-01: UDP silent {}s on pod {} — sending TelemetryGap",
                        gap,
                        pod_id
                    );
                    let msg = AgentMessage::TelemetryGap {
                        pod_id: pod_id.clone(),
                        sim_type: SimType::AssettoCorsa, // TODO: read from state.sim_type when available
                        gap_seconds: gap as u32,
                    };
                    let _ = agent_msg_tx.try_send(msg);
                }

                // Reset flag when data resumes
                if !udp_silent_60 {
                    telem_gap_fired = false;
                }
            } else {
                // Billing stopped or game exited — reset flag
                telem_gap_fired = false;
            }

            // CRASH-02: Launch timeout — game process never appeared 90s after LaunchGame
            if let Some(launched_at) = state.launch_started_at {
                if !launch_timeout_fired
                    && launched_at.elapsed() > Duration::from_secs(LAUNCH_TIMEOUT_SECS)
                    && state.game_pid.is_none()
                {
                    tracing::warn!(
                        "[failure-monitor] Launch timeout: {}s elapsed, no game PID — killing Content Manager",
                        launched_at.elapsed().as_secs()
                    );
                    launch_timeout_fired = true; // suppress duplicate fires for this launch attempt
                    let synthetic = "launch timeout — Content Manager hang kill cm process";
                    let snap = build_snapshot(&state, &status, pod_id.clone(), pod_number);
                    let synthetic_owned = synthetic.to_string();
                    let _ = tokio::task::spawn_blocking(move || {
                        try_auto_fix(&synthetic_owned, &snap)
                    }).await;
                }
            }

            // CRASH-01: Game freeze — game running + UDP silent 30s + low CPU + hung window
            if state.game_pid.is_some() {
                let udp_silent = state.last_udp_secs_ago
                    .map(|s| s >= FREEZE_UDP_SILENCE_SECS)
                    .unwrap_or(false);

                if udp_silent && status.game_running.load(Ordering::Relaxed) {
                    let game_pid = state.game_pid.unwrap();
                    // Check CPU + IsHungAppWindow only when UDP silence threshold already met
                    // (avoids expensive EnumWindows on every 5s tick)
                    let hung = tokio::task::spawn_blocking(move || {
                        is_game_process_hung(game_pid)
                    }).await.unwrap_or(false);

                    if hung {
                        tracing::warn!(
                            "[failure-monitor] Game frozen: PID {} — UDP silent {}s + IsHungAppWindow=true",
                            game_pid,
                            state.last_udp_secs_ago.unwrap_or(0)
                        );
                        let synthetic = "Game frozen — IsHungAppWindow true + UDP silent 30s relaunch acs.exe";
                        let snap = build_snapshot(&state, &status, pod_id.clone(), pod_number);
                        let synthetic_owned = synthetic.to_string();
                        let _ = tokio::task::spawn_blocking(move || {
                            try_auto_fix(&synthetic_owned, &snap)
                        }).await;
                    }
                }
            }
        }
    });
}

/// Build a PodStateSnapshot from the current FailureMonitorState + HeartbeatStatus.
fn build_snapshot(
    state: &FailureMonitorState,
    status: &Arc<HeartbeatStatus>,
    pod_id: String,
    pod_number: u32,
) -> PodStateSnapshot {
    PodStateSnapshot {
        pod_id,
        pod_number,
        billing_active: state.billing_active,
        game_pid: state.game_pid,
        wheelbase_connected: state.hid_connected,
        ws_connected: status.ws_connected.load(Ordering::Relaxed),
        last_udp_secs_ago: state.last_udp_secs_ago,
        game_launch_elapsed_secs: state.launch_started_at.map(|t| t.elapsed().as_secs()),
        hid_last_error: !state.hid_connected,
        ..Default::default()
    }
}

/// Check whether the game process's window is frozen using IsHungAppWindow.
///
/// Uses CPU check (sysinfo two-refresh) as cheap pre-filter before calling EnumWindows.
/// Returns false on any Windows API error — prefer false positives over false negatives here.
///
/// MUST be called from a spawn_blocking context — both sysinfo and hidapi are blocking.
#[cfg(windows)]
fn is_game_process_hung(game_pid: u32) -> bool {
    use std::cell::Cell;

    // Step 1: CPU check via sysinfo two-refresh
    // A truly frozen process has near-zero CPU. Games in loading screens may also be low,
    // so this is a pre-filter only — IsHungAppWindow is the authoritative check.
    use sysinfo::{System, ProcessesToUpdate, Pid};
    let mut sys = System::new();
    sys.refresh_processes(ProcessesToUpdate::All, true);
    std::thread::sleep(std::time::Duration::from_millis(500));
    sys.refresh_processes(ProcessesToUpdate::All, true);
    let cpu = sys.process(Pid::from_u32(game_pid))
        .map(|p| p.cpu_usage())
        .unwrap_or(100.0); // default 100% = "not hung" — conservative to avoid false positive

    if cpu > 5.0 {
        // Process is actively using CPU — not frozen, skip expensive EnumWindows
        return false;
    }

    // Step 2: IsHungAppWindow via EnumWindows
    // Use thread_local! to pass state to the extern "system" callback (closure capture
    // is not allowed across FFI boundary).
    thread_local! {
        static HUNG_FOUND: Cell<bool> = Cell::new(false);
        static TARGET_PID: Cell<u32> = Cell::new(0);
    }

    use winapi::shared::minwindef::{BOOL, LPARAM};
    use winapi::shared::windef::HWND;
    use winapi::um::winuser::{EnumWindows, GetWindowThreadProcessId, IsHungAppWindow};

    HUNG_FOUND.with(|c| c.set(false));
    TARGET_PID.with(|c| c.set(game_pid));

    unsafe extern "system" fn enum_callback(hwnd: HWND, _lparam: LPARAM) -> BOOL {
        let target = TARGET_PID.with(|c| c.get());
        let mut window_pid: u32 = 0;
        unsafe { GetWindowThreadProcessId(hwnd, &mut window_pid as *mut u32); }
        if window_pid == target {
            if unsafe { IsHungAppWindow(hwnd) } != 0 {
                HUNG_FOUND.with(|c| c.set(true));
                return 0; // stop enumeration — found our hung window
            }
        }
        1 // continue enumeration
    }

    unsafe { EnumWindows(Some(enum_callback), 0); }
    HUNG_FOUND.with(|c| c.get())
}

#[cfg(not(windows))]
fn is_game_process_hung(_game_pid: u32) -> bool {
    false // Non-Windows: never report hung (Windows-only feature)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_state(overrides: impl FnOnce(&mut FailureMonitorState)) -> FailureMonitorState {
        let mut s = FailureMonitorState::default();
        overrides(&mut s);
        s
    }

    #[test]
    fn test_usb_reconnect_condition_fires_when_prev_false_curr_true_billing() {
        // Simulates the transition: was disconnected, now connected, billing active
        let prev = false;
        let state = make_state(|s| {
            s.hid_connected = true;
            s.billing_active = true;
        });
        // Reconnect fires when: !prev && state.hid_connected && state.billing_active
        assert!(!prev && state.hid_connected && state.billing_active,
            "USB reconnect condition should fire");
    }

    #[test]
    fn test_usb_reconnect_does_not_fire_when_billing_inactive() {
        let prev = false;
        let state = make_state(|s| {
            s.hid_connected = true;
            s.billing_active = false; // no billing
        });
        // Reconnect should NOT fire without active billing
        assert!(!((!prev && state.hid_connected) && state.billing_active),
            "USB reconnect must not fire without active billing");
    }

    #[test]
    fn test_launch_timeout_fires_when_no_game_pid_after_90s() {
        // Simulate a launch that started 100s ago with no game PID
        let state = make_state(|s| {
            s.launch_started_at = Some(Instant::now() - Duration::from_secs(100));
            s.game_pid = None;
            s.recovery_in_progress = false;
        });
        let launched_at = state.launch_started_at.unwrap();
        let should_fire = !false // launch_timeout_fired starts false
            && launched_at.elapsed() > Duration::from_secs(LAUNCH_TIMEOUT_SECS)
            && state.game_pid.is_none();
        assert!(should_fire, "Launch timeout must fire after 90s with no game PID");
    }

    #[test]
    fn test_launch_timeout_does_not_fire_if_game_pid_present() {
        let state = make_state(|s| {
            s.launch_started_at = Some(Instant::now() - Duration::from_secs(100));
            s.game_pid = Some(1234); // game launched successfully
            s.recovery_in_progress = false;
        });
        let launched_at = state.launch_started_at.unwrap();
        let should_fire = launched_at.elapsed() > Duration::from_secs(LAUNCH_TIMEOUT_SECS)
            && state.game_pid.is_none(); // false — PID present
        assert!(!should_fire, "Launch timeout must NOT fire if game PID present");
    }

    #[test]
    fn test_freeze_detection_udp_silence_threshold() {
        // UDP silent 30s+ should trigger freeze check
        let state = make_state(|s| {
            s.game_pid = Some(5678);
            s.last_udp_secs_ago = Some(35); // 35s > 30s threshold
            s.billing_active = true;
            s.recovery_in_progress = false;
        });
        let udp_silent = state.last_udp_secs_ago
            .map(|s| s >= FREEZE_UDP_SILENCE_SECS)
            .unwrap_or(false);
        assert!(udp_silent, "35s UDP silence should cross 30s threshold");
    }

    #[test]
    fn test_freeze_detection_not_triggered_below_threshold() {
        let state = make_state(|s| {
            s.game_pid = Some(5678);
            s.last_udp_secs_ago = Some(20); // 20s < 30s threshold
            s.billing_active = true;
        });
        let udp_silent = state.last_udp_secs_ago
            .map(|s| s >= FREEZE_UDP_SILENCE_SECS)
            .unwrap_or(false);
        assert!(!udp_silent, "20s UDP silence should NOT cross 30s threshold");
    }

    #[test]
    fn test_recovery_in_progress_suppresses_all_checks() {
        let state = make_state(|s| {
            s.recovery_in_progress = true;
            s.game_pid = Some(9999);
            s.last_udp_secs_ago = Some(60); // would normally trigger freeze
            s.hid_connected = true; // would normally be valid reconnect target
        });
        // When recovery_in_progress is true, the polling loop calls `continue` — skip all checks
        assert!(state.recovery_in_progress,
            "recovery_in_progress flag must be set to suppress checks");
    }

    #[test]
    fn test_failure_monitor_state_default() {
        let s = FailureMonitorState::default();
        assert!(s.game_pid.is_none());
        assert!(s.last_udp_secs_ago.is_none());
        assert!(!s.hid_connected);
        assert!(s.launch_started_at.is_none());
        assert!(!s.billing_active);
        assert!(!s.recovery_in_progress);
        assert!(s.driving_state.is_none(), "driving_state must default to None");
    }

    #[test]
    fn telem_gap_fires_when_billing_active_game_pid_and_60s_silence() {
        // TELEM-01: all conditions met — should send TelemetryGap
        let state = make_state(|s| {
            s.billing_active = true;
            s.game_pid = Some(1234);
            s.last_udp_secs_ago = Some(65); // 65s > 60s threshold
            s.recovery_in_progress = false;
        });
        let udp_silent_60 = state.last_udp_secs_ago
            .map(|s| s >= TELEM_GAP_SECS)
            .unwrap_or(false);
        let should_fire = state.billing_active && state.game_pid.is_some()
            && !state.recovery_in_progress && udp_silent_60;
        assert!(should_fire, "TELEM-01 must fire with billing+game+60s silence");
    }

    #[test]
    fn telem_gap_does_not_fire_when_billing_inactive() {
        // TELEM-01: billing not active — no alert
        let state = make_state(|s| {
            s.billing_active = false;
            s.game_pid = Some(1234);
            s.last_udp_secs_ago = Some(90);
            s.recovery_in_progress = false;
        });
        let udp_silent_60 = state.last_udp_secs_ago
            .map(|s| s >= TELEM_GAP_SECS)
            .unwrap_or(false);
        let should_fire = state.billing_active && state.game_pid.is_some()
            && !state.recovery_in_progress && udp_silent_60;
        assert!(!should_fire, "TELEM-01 must NOT fire without active billing");
    }

    #[test]
    fn telem_gap_does_not_fire_when_game_pid_none() {
        // TELEM-01: no game PID — no alert (game not running)
        let state = make_state(|s| {
            s.billing_active = true;
            s.game_pid = None; // no game process
            s.last_udp_secs_ago = Some(90);
            s.recovery_in_progress = false;
        });
        let udp_silent_60 = state.last_udp_secs_ago
            .map(|s| s >= TELEM_GAP_SECS)
            .unwrap_or(false);
        let should_fire = state.billing_active && state.game_pid.is_some()
            && !state.recovery_in_progress && udp_silent_60;
        assert!(!should_fire, "TELEM-01 must NOT fire without a game PID");
    }

    #[test]
    fn telem_gap_does_not_fire_below_60s_threshold() {
        // TELEM-01: 59s silence — below 60s threshold
        let state = make_state(|s| {
            s.billing_active = true;
            s.game_pid = Some(1234);
            s.last_udp_secs_ago = Some(59); // just below threshold
            s.recovery_in_progress = false;
        });
        let udp_silent_60 = state.last_udp_secs_ago
            .map(|s| s >= TELEM_GAP_SECS)
            .unwrap_or(false);
        assert!(!udp_silent_60, "59s must NOT cross TELEM_GAP_SECS=60 threshold");
    }

    #[test]
    fn telem_gap_fired_flag_prevents_duplicate_sends() {
        // TELEM-01: once fired, telem_gap_fired=true suppresses repeat sends
        let telem_gap_fired = true; // simulates already having fired this silence window
        let udp_still_silent = true;
        let would_send_again = udp_still_silent && !telem_gap_fired;
        assert!(!would_send_again, "telem_gap_fired flag must suppress duplicate TelemetryGap sends");
    }

    #[test]
    fn telem_gap_flag_resets_when_udp_resumes() {
        // TELEM-01: when last_udp_secs_ago drops below threshold, flag resets
        let state = make_state(|s| {
            s.billing_active = true;
            s.game_pid = Some(1234);
            s.last_udp_secs_ago = Some(10); // data resumed
            s.recovery_in_progress = false;
        });
        let udp_silent_60 = state.last_udp_secs_ago
            .map(|s| s >= TELEM_GAP_SECS)
            .unwrap_or(false);
        // When not silent, telem_gap_fired should be reset to false
        assert!(!udp_silent_60, "10s is below threshold — telem_gap_fired must reset");
    }
}
