//! Off-track detection for mid-session blanking (VMS-inspired Feature 3).
//!
//! Monitors AC shared memory telemetry to detect when the car goes off-track.
//! Uses `current_lap_invalid` transitions as the primary signal with a debounce
//! to avoid flickering the branded overlay.
//!
//! Conservative design: false negatives (missing some off-tracks) are preferred
//! over false positives (blanking during normal driving).

use std::time::{Duration, Instant};

const LOG_TARGET: &str = "off-track";

/// Debounce: how long the off-track signal must persist before triggering overlay.
/// 300ms debounce + 200ms telemetry poll = ~500ms total response time.
const DEBOUNCE_ON: Duration = Duration::from_millis(300);
/// Debounce: how long the on-track signal must persist before hiding overlay.
const DEBOUNCE_OFF: Duration = Duration::from_millis(500);

/// Tracks off-track state with debounced transitions.
pub struct OffTrackDetector {
    /// Whether we're currently showing the off-track overlay
    overlay_active: bool,
    /// When the raw signal last changed state
    last_transition: Instant,
    /// Previous value of `current_lap_invalid`
    prev_lap_invalid: bool,
    /// Raw (un-debounced) off-track state
    raw_off_track: bool,
    /// Feature enabled flag
    enabled: bool,
}

impl OffTrackDetector {
    pub fn new(enabled: bool) -> Self {
        Self {
            overlay_active: false,
            last_transition: Instant::now(),
            prev_lap_invalid: false,
            raw_off_track: false,
            enabled,
        }
    }

    /// Update with new telemetry data. Returns the overlay state change.
    /// `current_lap_invalid`: from AC shared memory graphics (isValidLap field).
    /// `speed_kmh`: current car speed (used to filter out stationary cuts).
    /// `is_in_pit`: true when car is in pit box/pit lane -- suppresses off-track blanking.
    pub fn update(&mut self, current_lap_invalid: bool, speed_kmh: f32, is_in_pit: bool) -> OverlayChange {
        if !self.enabled {
            return OverlayChange::NoChange;
        }

        // Suppress off-track blanking during pit stops
        if is_in_pit {
            if self.overlay_active {
                self.overlay_active = false;
                self.raw_off_track = false;
                self.prev_lap_invalid = current_lap_invalid;
                tracing::info!(target: LOG_TARGET, "Off-track overlay HIDDEN (car entered pit)");
                return OverlayChange::Hide;
            }
            self.prev_lap_invalid = current_lap_invalid;
            return OverlayChange::NoChange;
        }

        // Detect transition from valid to invalid lap (off-track cut detected by AC)
        // Only trigger if car is moving (>5 km/h)
        let off_track_signal = current_lap_invalid && !self.prev_lap_invalid && speed_kmh > 5.0;
        let on_track_signal = !current_lap_invalid && self.prev_lap_invalid;

        self.prev_lap_invalid = current_lap_invalid;

        // Update raw state on transitions
        if off_track_signal {
            if !self.raw_off_track {
                self.raw_off_track = true;
                self.last_transition = Instant::now();
                tracing::debug!(target: LOG_TARGET, "Off-track signal detected (speed: {:.0} km/h)", speed_kmh);
            }
        } else if on_track_signal {
            if self.raw_off_track {
                self.raw_off_track = false;
                self.last_transition = Instant::now();
                tracing::debug!(target: LOG_TARGET, "On-track signal detected");
            }
        }

        // Apply debounce
        let elapsed = self.last_transition.elapsed();

        if self.raw_off_track && !self.overlay_active && elapsed >= DEBOUNCE_ON {
            self.overlay_active = true;
            tracing::info!(target: LOG_TARGET, "Off-track overlay ACTIVATED (debounce: {:?})", elapsed);
            return OverlayChange::Show;
        }

        if !self.raw_off_track && self.overlay_active && elapsed >= DEBOUNCE_OFF {
            self.overlay_active = false;
            tracing::info!(target: LOG_TARGET, "Off-track overlay HIDDEN (debounce: {:?})", elapsed);
            return OverlayChange::Hide;
        }

        OverlayChange::NoChange
    }

    /// Reset state (call when game exits or session ends).
    pub fn reset(&mut self) {
        self.overlay_active = false;
        self.raw_off_track = false;
        self.prev_lap_invalid = false;
        self.last_transition = Instant::now();
    }

    pub fn is_overlay_active(&self) -> bool {
        self.overlay_active
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled && self.overlay_active {
            self.overlay_active = false;
        }
    }
}

/// Result of an off-track detection update.
#[derive(Debug, PartialEq)]
pub enum OverlayChange {
    /// Show the branded off-track overlay
    Show,
    /// Hide the overlay (car back on track)
    Hide,
    /// No state change
    NoChange,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_trigger_when_disabled() {
        let mut detector = OffTrackDetector::new(false);
        assert_eq!(detector.update(true, 100.0, false), OverlayChange::NoChange);
    }

    #[test]
    fn test_no_trigger_at_low_speed() {
        let mut detector = OffTrackDetector::new(true);
        detector.update(false, 3.0, false);
        assert_eq!(detector.update(true, 3.0, false), OverlayChange::NoChange);
    }

    #[test]
    fn test_debounce_prevents_immediate_trigger() {
        let mut detector = OffTrackDetector::new(true);
        detector.update(false, 100.0, false);
        let result = detector.update(true, 100.0, false);
        assert_eq!(result, OverlayChange::NoChange);
    }

    #[test]
    fn test_no_trigger_in_pit() {
        let mut detector = OffTrackDetector::new(true);
        detector.update(false, 100.0, false);
        assert_eq!(detector.update(true, 100.0, true), OverlayChange::NoChange);
    }

    #[test]
    fn test_pit_hides_active_overlay() {
        let mut detector = OffTrackDetector::new(true);
        detector.update(false, 100.0, false);
        detector.update(true, 100.0, false);
        detector.overlay_active = true;
        detector.raw_off_track = true;
        assert_eq!(detector.update(true, 60.0, true), OverlayChange::Hide);
    }
}
