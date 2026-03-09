use std::time::{Duration, Instant};

use rc_common::types::DrivingState;

/// Configuration for the driving detector
#[derive(Debug, Clone)]
pub struct DetectorConfig {
    /// How long to wait before declaring idle (prevents flicker)
    pub idle_threshold: Duration,
    /// Steering deadzone — small values are noise from the wheelbase
    pub steering_deadzone: f32,
    /// Throttle/brake minimum threshold to count as active input
    pub pedal_threshold: f32,
    /// UDP ports to monitor for game telemetry
    pub telemetry_ports: Vec<u16>,
    /// Conspit wheelbase USB Vendor ID (OpenFFBoard default: 0x1209)
    pub wheelbase_vid: u16,
    /// Conspit wheelbase USB Product ID (OpenFFBoard default: 0xFFB0)
    pub wheelbase_pid: u16,
}

impl Default for DetectorConfig {
    fn default() -> Self {
        Self {
            idle_threshold: Duration::from_secs(10),
            steering_deadzone: 0.02, // 2%
            pedal_threshold: 0.05,   // 5%
            telemetry_ports: vec![9996, 20777, 5300, 6789, 5555],
            wheelbase_vid: 0x1209,
            wheelbase_pid: 0xFFB0,
        }
    }
}

/// Signals from the HID and UDP monitoring subsystems
#[derive(Debug, Clone, Copy)]
pub enum DetectorSignal {
    /// HID input detected: pedals/wheel are active
    HidActive,
    /// HID inputs are idle (no movement)
    HidIdle,
    /// HID device disconnected or not found
    HidDisconnected,
    /// UDP telemetry packet received on a monitored port
    UdpActive,
    /// No UDP packets received recently
    UdpIdle,
}

/// Hysteresis-based driving state detector combining USB HID and UDP signals.
///
/// Transitions to Active immediately on any input.
/// Transitions to Idle only after `idle_threshold` consecutive seconds of no input.
pub struct DrivingDetector {
    current_state: DrivingState,
    last_active_input: Option<Instant>,
    idle_threshold: Duration,
    hid_connected: bool,
    hid_active: bool,
    udp_active: bool,
    last_udp_packet: Option<Instant>,
}

impl DrivingDetector {
    pub fn new(config: &DetectorConfig) -> Self {
        Self {
            current_state: DrivingState::NoDevice,
            last_active_input: None,
            idle_threshold: config.idle_threshold,
            hid_connected: false,
            hid_active: false,
            udp_active: false,
            last_udp_packet: None,
        }
    }

    /// Process a signal from the HID or UDP subsystems.
    /// Returns the current state and whether it changed.
    pub fn process_signal(&mut self, signal: DetectorSignal) -> (DrivingState, bool) {
        match signal {
            DetectorSignal::HidActive => {
                self.hid_connected = true;
                self.hid_active = true;
                self.last_active_input = Some(Instant::now());
            }
            DetectorSignal::HidIdle => {
                self.hid_connected = true;
                self.hid_active = false;
            }
            DetectorSignal::HidDisconnected => {
                self.hid_connected = false;
                self.hid_active = false;
            }
            DetectorSignal::UdpActive => {
                self.udp_active = true;
                self.last_udp_packet = Some(Instant::now());
                self.last_active_input = Some(Instant::now());
            }
            DetectorSignal::UdpIdle => {
                self.udp_active = false;
            }
        }

        self.evaluate_state()
    }

    /// Periodic evaluation of driving state (call every ~100ms from main loop)
    pub fn evaluate_state(&mut self) -> (DrivingState, bool) {
        // Check if UDP data is stale (> 2 seconds since last packet)
        if let Some(last_udp) = self.last_udp_packet {
            if last_udp.elapsed() > Duration::from_secs(2) {
                self.udp_active = false;
            }
        }

        let any_active = self.hid_active || self.udp_active;
        let has_any_source = self.hid_connected || self.last_udp_packet.is_some();

        let new_state = if any_active {
            DrivingState::Active
        } else if !has_any_source {
            DrivingState::NoDevice
        } else {
            // Check hysteresis: has it been idle long enough?
            match self.last_active_input {
                Some(last) if last.elapsed() < self.idle_threshold => DrivingState::Active,
                _ => DrivingState::Idle,
            }
        };

        let changed = new_state != self.current_state;
        self.current_state = new_state;
        (new_state, changed)
    }

    pub fn state(&self) -> DrivingState {
        self.current_state
    }

    /// Alias for state() — used by PodStateSnapshot builder.
    pub fn current_state(&self) -> DrivingState {
        self.current_state
    }

    /// Whether the USB HID wheelbase is currently connected.
    pub fn is_hid_connected(&self) -> bool {
        self.hid_connected
    }
}

/// Parsed HID input report from a Conspit/OpenFFBoard wheelbase
#[derive(Debug, Clone, Copy)]
pub struct WheelbaseInput {
    /// Steering wheel angle, normalized to -1.0 .. 1.0
    pub steering: f32,
    /// Throttle pedal, 0.0 .. 1.0
    pub throttle: f32,
    /// Brake pedal, 0.0 .. 1.0
    pub brake: f32,
}

/// Analyze a raw HID input report from an OpenFFBoard-based wheelbase.
///
/// OpenFFBoard HID reports typically use:
///   Bytes 0-3: X axis (steering) as 16-bit or 32-bit value
///   Bytes 4-5: Y axis (throttle) as 16-bit value
///   Bytes 6-7: Z axis (brake) as 16-bit value
///   Remaining: buttons, hat switch, etc.
///
/// Exact format depends on the HID report descriptor. This function handles
/// the common OpenFFBoard layout. If parsing fails, returns None.
pub fn parse_openffboard_report(data: &[u8]) -> Option<WheelbaseInput> {
    if data.len() < 8 {
        return None;
    }

    // OpenFFBoard typically sends 16-bit axes
    // X axis (steering): bytes 0-1, unsigned 0..65535, center at 32768
    let steer_raw = u16::from_le_bytes([data[0], data[1]]);
    let steering = (steer_raw as f32 - 32768.0) / 32768.0;

    // Y axis (throttle): bytes 2-3, unsigned 0..65535
    let throttle_raw = u16::from_le_bytes([data[2], data[3]]);
    let throttle = throttle_raw as f32 / 65535.0;

    // Z axis (brake): bytes 4-5, unsigned 0..65535
    let brake_raw = u16::from_le_bytes([data[4], data[5]]);
    let brake = brake_raw as f32 / 65535.0;

    Some(WheelbaseInput {
        steering,
        throttle,
        brake,
    })
}

/// Determine if HID input indicates active driving
pub fn is_input_active(input: &WheelbaseInput, config: &DetectorConfig) -> bool {
    input.throttle > config.pedal_threshold
        || input.brake > config.pedal_threshold
        || input.steering.abs() > config.steering_deadzone
}

/// Detect active input by comparing current and previous wheel angle
/// (catches the case where the customer is turning the wheel without pedal input)
pub fn is_steering_moving(current: f32, previous: f32, threshold: f32) -> bool {
    (current - previous).abs() > threshold
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_detector() -> DrivingDetector {
        DrivingDetector::new(&DetectorConfig::default())
    }

    #[test]
    fn initial_state_is_no_device() {
        let d = make_detector();
        assert_eq!(d.state(), DrivingState::NoDevice);
    }

    #[test]
    fn hid_active_transitions_to_active() {
        let mut d = make_detector();
        let (state, changed) = d.process_signal(DetectorSignal::HidActive);
        assert_eq!(state, DrivingState::Active);
        assert!(changed);
    }

    #[test]
    fn udp_active_transitions_to_active() {
        let mut d = make_detector();
        let (state, changed) = d.process_signal(DetectorSignal::UdpActive);
        assert_eq!(state, DrivingState::Active);
        assert!(changed);
    }

    #[test]
    fn stays_active_within_idle_threshold() {
        let mut d = make_detector();
        d.process_signal(DetectorSignal::HidActive);
        d.process_signal(DetectorSignal::HidIdle);
        // Should still be active because idle_threshold hasn't elapsed
        let (state, _) = d.evaluate_state();
        assert_eq!(state, DrivingState::Active);
    }

    #[test]
    fn hid_disconnect_without_udp_goes_to_no_device() {
        let mut d = make_detector();
        d.process_signal(DetectorSignal::HidDisconnected);
        let (state, _) = d.evaluate_state();
        assert_eq!(state, DrivingState::NoDevice);
    }

    #[test]
    fn input_active_detection() {
        let config = DetectorConfig::default();
        let input = WheelbaseInput {
            steering: 0.0,
            throttle: 0.1,
            brake: 0.0,
        };
        assert!(is_input_active(&input, &config));

        let idle_input = WheelbaseInput {
            steering: 0.01,
            throttle: 0.01,
            brake: 0.01,
        };
        assert!(!is_input_active(&idle_input, &config));
    }

    #[test]
    fn steering_movement_detection() {
        assert!(is_steering_moving(0.5, 0.0, 0.02));
        assert!(!is_steering_moving(0.01, 0.0, 0.02));
    }
}
