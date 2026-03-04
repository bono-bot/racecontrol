use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::state::AppState;
use rc_common::protocol::DashboardEvent;
use rc_common::types::TelemetryFrame;

// ─── Camera Modes ────────────────────────────────────────────────────────────

/// Camera switching strategy inspired by VMS Connect
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CameraMode {
    /// Follow the car closest to crossing the finish line, cycle after crossing
    ClosestCycle,
    /// Always follow the leader (fastest lap overall)
    Leader,
    /// Follow the car closest to crossing the finish line (no cycle)
    Closest,
    /// Cycle through all active cars at a fixed interval
    Cycle,
    /// Disabled — no camera control
    Off,
}

impl Default for CameraMode {
    fn default() -> Self {
        CameraMode::ClosestCycle
    }
}

// ─── Camera State ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CameraFocus {
    pub pod_id: String,
    pub driver_name: String,
    pub reason: String, // e.g. "battle", "about_to_finish", "leader", "cycle"
}

#[derive(Debug)]
struct PodCameraData {
    pod_id: String,
    driver_name: String,
    speed_kmh: f32,
    lap_number: u32,
    lap_time_ms: u32,
    best_lap_ms: Option<u32>,
    last_update: std::time::Instant,
}

pub struct CameraController {
    pub mode: RwLock<CameraMode>,
    pub enabled: RwLock<bool>,
    pod_data: RwLock<HashMap<String, PodCameraData>>,
    current_focus: RwLock<Option<String>>,
    focus_since: RwLock<std::time::Instant>,
    cycle_index: RwLock<usize>,
}

// Minimum time to hold focus on a car before switching (seconds)
const MIN_FOCUS_SECS: u64 = 5;
// Maximum time to hold focus on a car before cycling (seconds)
const MAX_FOCUS_SECS: u64 = 20;
// Cycle mode interval (seconds)
const CYCLE_INTERVAL_SECS: u64 = 12;
// Speed difference threshold for detecting battles (km/h)
const BATTLE_SPEED_DIFF: f32 = 15.0;
// Stale data threshold (seconds) — pods not heard from in this time are excluded
const STALE_THRESHOLD_SECS: u64 = 10;

impl CameraController {
    pub fn new() -> Self {
        Self {
            mode: RwLock::new(CameraMode::ClosestCycle),
            enabled: RwLock::new(false),
            pod_data: RwLock::new(HashMap::new()),
            current_focus: RwLock::new(None),
            focus_since: RwLock::new(std::time::Instant::now()),
            cycle_index: RwLock::new(0),
        }
    }

    /// Update telemetry data for a pod
    pub async fn update_telemetry(&self, frame: &TelemetryFrame) {
        let mut data = self.pod_data.write().await;
        let entry = data.entry(frame.pod_id.clone()).or_insert_with(|| PodCameraData {
            pod_id: frame.pod_id.clone(),
            driver_name: frame.driver_name.clone(),
            speed_kmh: 0.0,
            lap_number: 0,
            lap_time_ms: 0,
            best_lap_ms: None,
            last_update: std::time::Instant::now(),
        });

        entry.driver_name = frame.driver_name.clone();
        entry.speed_kmh = frame.speed_kmh;
        entry.lap_number = frame.lap_number;
        entry.lap_time_ms = frame.lap_time_ms;
        entry.last_update = std::time::Instant::now();

        // Track best lap
        if frame.lap_time_ms > 0 {
            match entry.best_lap_ms {
                Some(best) if frame.lap_time_ms < best => {
                    entry.best_lap_ms = Some(frame.lap_time_ms);
                }
                None if frame.lap_number > 1 => {
                    entry.best_lap_ms = Some(frame.lap_time_ms);
                }
                _ => {}
            }
        }
    }

    /// Evaluate and possibly switch camera focus
    pub async fn tick(&self) -> Option<CameraFocus> {
        let enabled = *self.enabled.read().await;
        if !enabled {
            return None;
        }

        let mode = *self.mode.read().await;
        if mode == CameraMode::Off {
            return None;
        }

        let data = self.pod_data.read().await;
        let now = std::time::Instant::now();

        // Filter to active pods (with recent data)
        let active: Vec<&PodCameraData> = data
            .values()
            .filter(|p| now.duration_since(p.last_update).as_secs() < STALE_THRESHOLD_SECS)
            .filter(|p| p.speed_kmh > 1.0) // Must be moving
            .collect();

        if active.is_empty() {
            return None;
        }

        let current_focus = self.current_focus.read().await.clone();
        let focus_since = *self.focus_since.read().await;
        let focus_duration = now.duration_since(focus_since).as_secs();

        // Don't switch too quickly (unless there's no current focus)
        if current_focus.is_some() && focus_duration < MIN_FOCUS_SECS {
            return None;
        }

        let new_focus = match mode {
            CameraMode::ClosestCycle => {
                self.evaluate_closest_cycle(&active, &current_focus, focus_duration).await
            }
            CameraMode::Leader => self.evaluate_leader(&active),
            CameraMode::Closest => self.evaluate_closest(&active),
            CameraMode::Cycle => {
                self.evaluate_cycle(&active, &current_focus, focus_duration).await
            }
            CameraMode::Off => None,
        };

        if let Some(ref focus) = new_focus {
            if current_focus.as_deref() != Some(&focus.pod_id) {
                let mut cf = self.current_focus.write().await;
                *cf = Some(focus.pod_id.clone());
                let mut fs = self.focus_since.write().await;
                *fs = now;
                return new_focus;
            }
        }

        None
    }

    /// ClosestCycle: Focus car closest to finishing a lap, switch after they cross
    async fn evaluate_closest_cycle(
        &self,
        active: &[&PodCameraData],
        current_focus: &Option<String>,
        focus_duration: u64,
    ) -> Option<CameraFocus> {
        // Check for battles first (two cars with similar lap times on same lap)
        if let Some(battle) = self.detect_battle(active) {
            return Some(battle);
        }

        // If focused too long, cycle to next interesting car
        if focus_duration > MAX_FOCUS_SECS {
            return self.pick_next_interesting(active, current_focus);
        }

        // Find car closest to completing a lap (highest lap_time_ms within same lap)
        let closest = active
            .iter()
            .max_by_key(|p| p.lap_time_ms);

        closest.map(|p| CameraFocus {
            pod_id: p.pod_id.clone(),
            driver_name: p.driver_name.clone(),
            reason: "about_to_finish".to_string(),
        })
    }

    /// Leader: Always focus on the car with the best lap time
    fn evaluate_leader(&self, active: &[&PodCameraData]) -> Option<CameraFocus> {
        let leader = active
            .iter()
            .filter(|p| p.best_lap_ms.is_some())
            .min_by_key(|p| p.best_lap_ms.unwrap());

        // If no one has set a best lap, pick the one on the highest lap number
        let leader = leader.or_else(|| active.iter().max_by_key(|p| p.lap_number));

        leader.map(|p| CameraFocus {
            pod_id: p.pod_id.clone(),
            driver_name: p.driver_name.clone(),
            reason: "leader".to_string(),
        })
    }

    /// Closest: Focus on car closest to crossing the line
    fn evaluate_closest(&self, active: &[&PodCameraData]) -> Option<CameraFocus> {
        let closest = active
            .iter()
            .max_by_key(|p| p.lap_time_ms);

        closest.map(|p| CameraFocus {
            pod_id: p.pod_id.clone(),
            driver_name: p.driver_name.clone(),
            reason: "closest".to_string(),
        })
    }

    /// Cycle: Round-robin through all active cars
    async fn evaluate_cycle(
        &self,
        active: &[&PodCameraData],
        _current_focus: &Option<String>,
        focus_duration: u64,
    ) -> Option<CameraFocus> {
        if focus_duration < CYCLE_INTERVAL_SECS {
            return None;
        }

        let mut idx = self.cycle_index.write().await;
        *idx = (*idx + 1) % active.len();
        let p = active[*idx];

        Some(CameraFocus {
            pod_id: p.pod_id.clone(),
            driver_name: p.driver_name.clone(),
            reason: "cycle".to_string(),
        })
    }

    /// Detect a battle between two cars (similar lap times on the same track)
    fn detect_battle(&self, active: &[&PodCameraData]) -> Option<CameraFocus> {
        if active.len() < 2 {
            return None;
        }

        // Find pairs with close speeds and same lap number (potential on-track battle)
        for i in 0..active.len() {
            for j in (i + 1)..active.len() {
                let a = active[i];
                let b = active[j];

                // Same lap number and close speeds suggest they're near each other
                if a.lap_number == b.lap_number {
                    let speed_diff = (a.speed_kmh - b.speed_kmh).abs();
                    if speed_diff < BATTLE_SPEED_DIFF && a.speed_kmh > 50.0 && b.speed_kmh > 50.0 {
                        // Focus on the faster car in the battle
                        let focus = if a.speed_kmh >= b.speed_kmh { a } else { b };
                        return Some(CameraFocus {
                            pod_id: focus.pod_id.clone(),
                            driver_name: focus.driver_name.clone(),
                            reason: format!(
                                "battle with {}",
                                if a.speed_kmh >= b.speed_kmh {
                                    &b.driver_name
                                } else {
                                    &a.driver_name
                                }
                            ),
                        });
                    }
                }
            }
        }

        None
    }

    /// Pick the next most interesting car to watch
    fn pick_next_interesting(
        &self,
        active: &[&PodCameraData],
        current_focus: &Option<String>,
    ) -> Option<CameraFocus> {
        // Sort by speed descending — fastest car is most exciting
        let mut sorted: Vec<&&PodCameraData> = active.iter().collect();
        sorted.sort_by(|a, b| b.speed_kmh.partial_cmp(&a.speed_kmh).unwrap_or(std::cmp::Ordering::Equal));

        // Pick the first car that isn't currently focused
        for p in sorted {
            if current_focus.as_deref() != Some(&p.pod_id) {
                return Some(CameraFocus {
                    pod_id: p.pod_id.clone(),
                    driver_name: p.driver_name.clone(),
                    reason: "interesting".to_string(),
                });
            }
        }

        // Fall back to first car
        active.first().map(|p| CameraFocus {
            pod_id: p.pod_id.clone(),
            driver_name: p.driver_name.clone(),
            reason: "fallback".to_string(),
        })
    }

    /// Remove stale pod data
    pub async fn cleanup_stale(&self) {
        let mut data = self.pod_data.write().await;
        let now = std::time::Instant::now();
        data.retain(|_, v| now.duration_since(v.last_update).as_secs() < 60);
    }
}

// ─── Integration with main loop ──────────────────────────────────────────────

/// Called from the telemetry handler in ws/mod.rs when a telemetry frame arrives
pub async fn on_telemetry(state: &Arc<AppState>, frame: &TelemetryFrame) {
    state.camera.update_telemetry(frame).await;
}

/// Periodic camera tick — evaluates focus and broadcasts if changed
pub async fn tick(state: &Arc<AppState>) {
    if let Some(focus) = state.camera.tick().await {
        let _ = state.dashboard_tx.send(DashboardEvent::CameraFocusUpdate {
            pod_id: focus.pod_id,
            driver_name: focus.driver_name,
            reason: focus.reason,
        });
    }
}

/// Handle camera mode change from dashboard
pub async fn set_mode(state: &Arc<AppState>, mode: CameraMode) {
    let mut m = state.camera.mode.write().await;
    *m = mode;
    tracing::info!("Camera mode set to {:?}", mode);
}

/// Toggle camera on/off
pub async fn set_enabled(state: &Arc<AppState>, enabled: bool) {
    let mut e = state.camera.enabled.write().await;
    *e = enabled;
    tracing::info!("Camera control {}", if enabled { "enabled" } else { "disabled" });

    if !enabled {
        // Clear focus when disabled
        let _ = state.dashboard_tx.send(DashboardEvent::CameraFocusUpdate {
            pod_id: String::new(),
            driver_name: String::new(),
            reason: "disabled".to_string(),
        });
    }
}
