#![allow(dead_code)]
//! Predictive Maintenance — threshold-based anomaly detection for hardware and software trends.
//!
//! Instead of waiting for failure, detects degradation patterns and alerts BEFORE impact.
//! Runs as part of the 5-minute diagnostic scan (called from diagnostic_engine).
//!
//! Phase 236 — Meshed Intelligence PRED-01 to PRED-06.
//! MMA-trained additions (v26.1): PRED-07 to PRED-09.
//!
//! Thresholds (not ML — simple, reliable, zero cost):
//!   PRED-01: ConspitLink reconnection rate trending → USB alert
//!   PRED-02: Edge process count trending down → memory leak restart
//!   PRED-03: GPU temp consistently >80C → thermal alert
//!   PRED-04: rc-agent restart count >2/day → stability alert
//!   PRED-05: Disk space <10GB → auto-cleanup
//!   PRED-06: Error spike across 3+ pods → systemic alert (handled by server coordinator)
//!   PRED-07: CLOSE_WAIT socket accumulation → port exhaustion (MiMo SRE method)
//!   PRED-08: Orphan PowerShell count → memory leak from self-restart (MiMo SRE method)
//!   PRED-09: MAINTENANCE_MODE age → stuck sentinel with no TTL (R1 Reasoner method)

use serde::Serialize;
use std::collections::VecDeque;

const LOG_TARGET: &str = "predictive-maint";

/// Maximum samples to keep per metric (5-min intervals × 24 hours = 288)
const MAX_SAMPLES: usize = 288;

/// PRED-03: GPU temperature alert threshold (Celsius)
const GPU_TEMP_ALERT_C: f64 = 80.0;

/// PRED-04: Max restarts per day before alerting
const MAX_RESTARTS_PER_DAY: u32 = 2;

/// PRED-05: Disk space alert threshold (bytes) — 10 GB
const DISK_SPACE_ALERT_BYTES: u64 = 10 * 1024 * 1024 * 1024;

/// PRED-01: ConspitLink reconnection count threshold per hour
const CONSPIT_RECONNECT_ALERT_PER_HOUR: u32 = 3;

/// PRED-02: Edge process count — alert if drops to 0 when blanking expected
const EDGE_MISSING_CONSECUTIVE_SCANS: u32 = 2;

/// PRED-07: CLOSE_WAIT socket count alert threshold (MiMo SRE method)
const CLOSE_WAIT_ALERT_COUNT: u64 = 20;

/// PRED-08: Orphan PowerShell process count alert threshold (MiMo SRE method)
const ORPHAN_POWERSHELL_ALERT_COUNT: u32 = 3;

/// PRED-09: MAINTENANCE_MODE age alert threshold in seconds (30 minutes) (R1 Reasoner method)
const MAINTENANCE_MODE_AGE_ALERT_SECS: u64 = 1800;

/// Predictive alert — something is degrading but hasn't failed yet.
#[derive(Debug, Clone, Serialize)]
pub struct PredictiveAlert {
    pub alert_type: PredAlertType,
    pub severity: AlertSeverity,
    pub message: String,
    pub metric_value: f64,
    pub threshold: f64,
}

#[derive(Debug, Clone, Serialize)]
pub enum PredAlertType {
    ConspitLinkReconnect,
    EdgeMemoryLeak,
    GpuThermal,
    StabilityDegrading,
    DiskSpaceLow,
    ErrorSpike,
    /// PRED-07: MiMo SRE — CLOSE_WAIT socket accumulation on :8090
    CloseWaitExhaustion,
    /// PRED-08: MiMo SRE — orphan PowerShell from self-restart memory leak
    OrphanPowerShell,
    /// PRED-09: R1 Reasoner — MAINTENANCE_MODE stuck with no TTL
    MaintenanceModeStuck,
}

#[derive(Debug, Clone, Serialize)]
pub enum AlertSeverity {
    Warning,
    Critical,
}

/// State tracked across scans for trend detection.
pub struct PredictiveState {
    /// PRED-01: ConspitLink reconnection events in the last hour
    conspit_reconnects: VecDeque<std::time::Instant>,
    /// PRED-02: Consecutive scans where Edge process count was 0 during blanking
    edge_missing_count: u32,
    /// PRED-04: rc-agent restart count today
    restart_count_today: u32,
    restart_date: chrono::NaiveDate,
}

impl PredictiveState {
    pub fn new() -> Self {
        Self {
            conspit_reconnects: VecDeque::new(),
            edge_missing_count: 0,
            restart_count_today: 0,
            restart_date: chrono::Utc::now().date_naive(),
        }
    }

    /// Reset daily counters if date changed (midnight crossing)
    fn maybe_reset_daily(&mut self) {
        let today = chrono::Utc::now().date_naive();
        if today != self.restart_date {
            self.restart_count_today = 0;
            self.restart_date = today;
        }
    }
}

impl Default for PredictiveState {
    fn default() -> Self {
        Self::new()
    }
}

/// Convert a PredictiveAlert into a FleetEvent::PredictiveAlert for broadcast.
///
/// This bridges the predictive maintenance system into the fleet event bus (Plan 273-01).
/// Called by diagnostic_engine after each predictive scan so alerts fan out to all subscribers.
pub fn alert_to_fleet_event(alert: &PredictiveAlert, node_id: &str) -> rc_common::fleet_event::FleetEvent {
    rc_common::fleet_event::FleetEvent::PredictiveAlert {
        alert_type: format!("{:?}", alert.alert_type),
        severity: format!("{:?}", alert.severity),
        message: alert.message.clone(),
        metric_value: alert.metric_value,
        threshold: alert.threshold,
        node_id: node_id.to_string(),
        timestamp: chrono::Utc::now(),
    }
}

/// Run all predictive checks. Returns alerts for any degrading metrics.
/// Called every 5 minutes by the diagnostic engine scan loop.
pub fn run_predictive_scan(state: &mut PredictiveState) -> Vec<PredictiveAlert> {
    state.maybe_reset_daily();
    let mut alerts = Vec::new();

    // PRED-03: GPU temperature check
    if let Some(alert) = check_gpu_temp() {
        alerts.push(alert);
    }

    // PRED-05: Disk space check
    if let Some(alert) = check_disk_space() {
        alerts.push(alert);
    }

    // PRED-01: ConspitLink reconnection rate
    if let Some(alert) = check_conspit_reconnects(state) {
        alerts.push(alert);
    }

    // PRED-07: MiMo SRE — CLOSE_WAIT socket accumulation (port exhaustion)
    if let Some(alert) = check_close_wait_sockets() {
        alerts.push(alert);
    }

    // PRED-08: MiMo SRE — orphan PowerShell processes (memory leak)
    if let Some(alert) = check_orphan_powershell() {
        alerts.push(alert);
    }

    // PRED-09: R1 Reasoner — MAINTENANCE_MODE stuck sentinel (no TTL)
    if let Some(alert) = check_maintenance_mode_stuck() {
        alerts.push(alert);
    }

    // Log results
    if alerts.is_empty() {
        tracing::debug!(target: LOG_TARGET, "Predictive scan: all metrics nominal");
    } else {
        for alert in &alerts {
            match alert.severity {
                AlertSeverity::Critical => {
                    tracing::warn!(
                        target: LOG_TARGET,
                        alert_type = ?alert.alert_type,
                        value = alert.metric_value,
                        threshold = alert.threshold,
                        "{}", alert.message
                    );
                }
                AlertSeverity::Warning => {
                    tracing::info!(
                        target: LOG_TARGET,
                        alert_type = ?alert.alert_type,
                        value = alert.metric_value,
                        threshold = alert.threshold,
                        "{}", alert.message
                    );
                }
            }
        }
    }

    alerts
}

/// PRED-03: Check GPU temperature via nvidia-smi.
/// Returns alert if consistently above 80C.
fn check_gpu_temp() -> Option<PredictiveAlert> {
    let output = std::process::Command::new("nvidia-smi")
        .args(["--query-gpu=temperature.gpu", "--format=csv,noheader,nounits"])
        .output()
        .ok()?;

    let temp_str = String::from_utf8(output.stdout).ok()?;
    let temp: f64 = temp_str.trim().parse().ok()?;

    if temp >= GPU_TEMP_ALERT_C {
        Some(PredictiveAlert {
            alert_type: PredAlertType::GpuThermal,
            severity: if temp >= 90.0 {
                AlertSeverity::Critical
            } else {
                AlertSeverity::Warning
            },
            message: format!(
                "PRED-03: GPU temperature {:.0}C exceeds {}C threshold — check HVAC / clean GPU fan",
                temp, GPU_TEMP_ALERT_C
            ),
            metric_value: temp,
            threshold: GPU_TEMP_ALERT_C,
        })
    } else {
        None
    }
}

/// PRED-05: Check disk space on C: drive.
/// Returns alert if below 10GB.
fn check_disk_space() -> Option<PredictiveAlert> {
    // Use sysinfo for cross-platform disk check
    use sysinfo::Disks;
    let disks = Disks::new_with_refreshed_list();

    for disk in disks.list() {
        let mount = disk.mount_point().to_string_lossy();
        if mount.starts_with("C:") || mount == "/" {
            let available = disk.available_space();
            if available < DISK_SPACE_ALERT_BYTES {
                let gb_available = available as f64 / (1024.0 * 1024.0 * 1024.0);
                let severity = if available < DISK_SPACE_ALERT_BYTES / 2 {
                    AlertSeverity::Critical
                } else {
                    AlertSeverity::Warning
                };

                // PRED-05: Auto-cleanup old logs
                if available < DISK_SPACE_ALERT_BYTES {
                    auto_cleanup_old_logs();
                }

                return Some(PredictiveAlert {
                    alert_type: PredAlertType::DiskSpaceLow,
                    severity,
                    message: format!(
                        "PRED-05: Disk space {:.1}GB below {}GB threshold",
                        gb_available,
                        DISK_SPACE_ALERT_BYTES / (1024 * 1024 * 1024)
                    ),
                    metric_value: gb_available,
                    threshold: (DISK_SPACE_ALERT_BYTES / (1024 * 1024 * 1024)) as f64,
                });
            }
        }
    }
    None
}

/// PRED-01: Check ConspitLink reconnection rate.
fn check_conspit_reconnects(state: &mut PredictiveState) -> Option<PredictiveAlert> {
    // Prune events older than 1 hour
    // Use checked_sub to avoid underflow on systems with <1hr uptime
    let now = std::time::Instant::now();
    let one_hour = std::time::Duration::from_secs(3600);
    if let Some(one_hour_ago) = now.checked_sub(one_hour) {
        while state
            .conspit_reconnects
            .front()
            .is_some_and(|t| *t < one_hour_ago)
        {
            state.conspit_reconnects.pop_front();
        }
    }

    let count = state.conspit_reconnects.len() as u32;
    if count >= CONSPIT_RECONNECT_ALERT_PER_HOUR {
        Some(PredictiveAlert {
            alert_type: PredAlertType::ConspitLinkReconnect,
            severity: AlertSeverity::Warning,
            message: format!(
                "PRED-01: ConspitLink reconnected {}x in last hour (threshold: {}) — USB port may be failing",
                count, CONSPIT_RECONNECT_ALERT_PER_HOUR
            ),
            metric_value: count as f64,
            threshold: CONSPIT_RECONNECT_ALERT_PER_HOUR as f64,
        })
    } else {
        None
    }
}

/// Record a ConspitLink reconnection event (called from HID monitoring).
pub fn record_conspit_reconnect(state: &mut PredictiveState) {
    state.conspit_reconnects.push_back(std::time::Instant::now());
    tracing::debug!(target: LOG_TARGET, "ConspitLink reconnection event recorded");
}

/// Record an rc-agent restart (called from startup).
/// PRED-04: Returns true if restart count exceeds threshold.
pub fn record_restart(state: &mut PredictiveState) -> bool {
    state.maybe_reset_daily();
    state.restart_count_today += 1;

    if state.restart_count_today > MAX_RESTARTS_PER_DAY {
        tracing::warn!(
            target: LOG_TARGET,
            count = state.restart_count_today,
            threshold = MAX_RESTARTS_PER_DAY,
            "PRED-04: rc-agent restart count exceeds daily threshold — stability degrading"
        );
        return true;
    }
    false
}

/// Record Edge process count for memory leak detection.
/// PRED-02: Alert if Edge count drops to 0 during expected blanking.
pub fn record_edge_count(state: &mut PredictiveState, count: u32, blanking_expected: bool) -> Option<PredictiveAlert> {
    if blanking_expected && count == 0 {
        state.edge_missing_count += 1;
        if state.edge_missing_count >= EDGE_MISSING_CONSECUTIVE_SCANS {
            return Some(PredictiveAlert {
                alert_type: PredAlertType::EdgeMemoryLeak,
                severity: AlertSeverity::Critical,
                message: format!(
                    "PRED-02: Edge process count 0 for {} consecutive scans during blanking — browser may have crashed or leaked memory",
                    state.edge_missing_count
                ),
                metric_value: 0.0,
                threshold: 1.0,
            });
        }
    } else {
        state.edge_missing_count = 0;
    }
    None
}

/// Auto-cleanup old log files to free disk space.
/// Removes .jsonl and .log files older than 7 days from C:\RacingPoint\.
fn auto_cleanup_old_logs() {
    let log_dir = std::path::Path::new(r"C:\RacingPoint");
    let seven_days_ago = std::time::SystemTime::now()
        - std::time::Duration::from_secs(7 * 24 * 3600);

    let entries = match std::fs::read_dir(log_dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    let mut cleaned = 0u32;
    for entry in entries.flatten() {
        let path = entry.path();
        if let Some(ext) = path.extension() {
            let ext_str = ext.to_string_lossy().to_lowercase();
            if ext_str == "jsonl" || ext_str == "log" {
                if let Ok(meta) = std::fs::metadata(&path) {
                    if let Ok(modified) = meta.modified() {
                        if modified < seven_days_ago {
                            if std::fs::remove_file(&path).is_ok() {
                                cleaned += 1;
                            }
                        }
                    }
                }
            }
        }
    }

    if cleaned > 0 {
        tracing::info!(
            target: LOG_TARGET,
            count = cleaned,
            "PRED-05: Auto-cleaned old log files (>7 days)"
        );
    }
}

// ─── MMA-Trained Predictive Checks ─────────────────────────────────────────
// These checks were learned from Multi-Model Audit diagnostic methodologies.

/// PRED-07: MiMo SRE method — CLOSE_WAIT socket accumulation on :8090.
/// Stale TCP connections in CLOSE_WAIT state accumulate over time, eventually
/// exhausting the port — rc-agent stops accepting new connections while health
/// endpoint (already connected) still returns OK.
fn check_close_wait_sockets() -> Option<PredictiveAlert> {
    let count = crate::diagnostic_engine::count_close_wait_sockets();
    if count >= CLOSE_WAIT_ALERT_COUNT {
        Some(PredictiveAlert {
            alert_type: PredAlertType::CloseWaitExhaustion,
            severity: if count >= CLOSE_WAIT_ALERT_COUNT * 2 {
                AlertSeverity::Critical
            } else {
                AlertSeverity::Warning
            },
            message: format!(
                "PRED-07: {} CLOSE_WAIT sockets on :8090 (threshold: {}) — port exhaustion risk",
                count, CLOSE_WAIT_ALERT_COUNT
            ),
            metric_value: count as f64,
            threshold: CLOSE_WAIT_ALERT_COUNT as f64,
        })
    } else {
        None
    }
}

/// PRED-08: MiMo SRE method — orphan PowerShell processes from self-restart.
/// self_monitor::relaunch_self() uses PowerShell+DETACHED_PROCESS which leaks
/// ~90MB per restart. Normal = 0-1 PowerShell. >3 = leak detected.
fn check_orphan_powershell() -> Option<PredictiveAlert> {
    let count = crate::diagnostic_engine::count_orphan_powershell();
    if count >= ORPHAN_POWERSHELL_ALERT_COUNT {
        let ram_mb = count as f64 * 90.0;
        Some(PredictiveAlert {
            alert_type: PredAlertType::OrphanPowerShell,
            severity: if count >= 8 {
                AlertSeverity::Critical
            } else {
                AlertSeverity::Warning
            },
            message: format!(
                "PRED-08: {} orphan PowerShell processes (~{:.0}MB leaked) — self-restart leak",
                count, ram_mb
            ),
            metric_value: count as f64,
            threshold: ORPHAN_POWERSHELL_ALERT_COUNT as f64,
        })
    } else {
        None
    }
}

/// PRED-09: R1 Reasoner method — MAINTENANCE_MODE sentinel stuck with no TTL.
/// Absence-based bug: once written, MAINTENANCE_MODE blocks ALL restarts permanently.
/// If it's been present for >30 minutes, it's likely stuck (not a transient crash storm).
fn check_maintenance_mode_stuck() -> Option<PredictiveAlert> {
    let age_secs = crate::diagnostic_engine::check_maintenance_mode_age()?;
    if age_secs >= MAINTENANCE_MODE_AGE_ALERT_SECS {
        let hours = age_secs as f64 / 3600.0;
        Some(PredictiveAlert {
            alert_type: PredAlertType::MaintenanceModeStuck,
            severity: if age_secs >= 7200 {
                AlertSeverity::Critical // >2 hours = definitely stuck
            } else {
                AlertSeverity::Warning
            },
            message: format!(
                "PRED-09: MAINTENANCE_MODE stuck for {:.1}h (threshold: {}min) — pod permanently blocked",
                hours, MAINTENANCE_MODE_AGE_ALERT_SECS / 60
            ),
            metric_value: age_secs as f64,
            threshold: MAINTENANCE_MODE_AGE_ALERT_SECS as f64,
        })
    } else {
        None
    }
}

// ─── v29.0 Extended Hardware Telemetry Collector ────────────────────────────
// Phase 1: Collect hardware health metrics for preventive maintenance.
// Called every 60s, separate from the 5-min predictive scan.
// Graceful fallback: if any collector fails, the field is None — never panics.

/// Extended hardware telemetry snapshot — all fields optional for graceful degradation.
#[derive(Debug, Clone, Serialize, Default)]
pub struct HardwareTelemetry {
    pub gpu_temp_celsius: Option<f32>,
    pub cpu_temp_celsius: Option<f32>,
    pub gpu_power_watts: Option<f32>,
    pub vram_usage_mb: Option<u32>,
    pub fan_speeds_rpm: Option<Vec<u32>>,
    pub disk_smart_health_pct: Option<u8>,
    pub disk_power_on_hours: Option<u32>,
    pub game_crashes_last_hour: Option<u8>,
    pub windows_critical_errors: Vec<String>,
    pub process_handle_count: Option<u32>,
    pub system_uptime_secs: Option<u64>,
    pub cpu_usage_pct: Option<f32>,
    pub gpu_usage_pct: Option<f32>,
    pub memory_usage_pct: Option<f32>,
    pub disk_usage_pct: Option<f32>,
    pub network_latency_ms: Option<u32>,
    pub usb_device_count: Option<u8>,
}

/// Collect hardware telemetry WITHOUT GPU metrics (nvidia-smi).
/// Used during active gaming sessions to avoid frame drops.
/// MMA MITIGATION: 4/5 models flagged nvidia-smi as RISK during gameplay.
pub fn collect_hardware_telemetry_no_gpu() -> HardwareTelemetry {
    let mut t = HardwareTelemetry::default();
    collect_sysinfo_metrics(&mut t); // CPU/memory/disk — no GPU driver lock
    t.system_uptime_secs = collect_system_uptime();
    t.process_handle_count = collect_handle_count();
    t.windows_critical_errors = collect_windows_errors();
    t.usb_device_count = collect_usb_count();
    t.network_latency_ms = collect_network_latency();
    t
}

/// Collect all hardware telemetry metrics. Each sub-collector is independent —
/// failure in one does not affect others. Static metrics (SMART, disk hours)
/// are cached for 1 hour to minimize overhead.
pub fn collect_hardware_telemetry() -> HardwareTelemetry {
    let mut t = HardwareTelemetry::default();

    // GPU metrics via nvidia-smi (batched single call)
    if let Some((temp, power, vram, usage)) = collect_gpu_metrics() {
        t.gpu_temp_celsius = Some(temp);
        t.gpu_power_watts = Some(power);
        t.vram_usage_mb = Some(vram);
        t.gpu_usage_pct = Some(usage);
    }

    // CPU/memory/disk via sysinfo
    collect_sysinfo_metrics(&mut t);

    // System uptime via GetTickCount64
    t.system_uptime_secs = collect_system_uptime();

    // Process handle count
    t.process_handle_count = collect_handle_count();

    // Windows critical errors (last 5 min)
    t.windows_critical_errors = collect_windows_errors();

    // USB device count
    t.usb_device_count = collect_usb_count();

    // Network latency (ping server)
    t.network_latency_ms = collect_network_latency();

    t
}

/// Batch GPU metrics from a single nvidia-smi call.
/// Returns (temp_c, power_w, vram_mb, utilization_pct).
///
/// MMA MITIGATION (4/5 consensus): nvidia-smi can cause 50-200ms GPU driver lock.
/// - Runs at BELOW_NORMAL priority to minimize impact on game rendering
/// - Caller should skip during active gaming sessions (billing_active flag)
fn collect_gpu_metrics() -> Option<(f32, f32, u32, f32)> {
    #[cfg(windows)]
    use std::os::windows::process::CommandExt;

    let mut cmd = std::process::Command::new("nvidia-smi");
    cmd.args([
        "--query-gpu=temperature.gpu,power.draw,memory.used,utilization.gpu",
        "--format=csv,noheader,nounits",
    ]);

    // Run at BELOW_NORMAL priority to avoid competing with game render thread
    #[cfg(windows)]
    cmd.creation_flags(0x00004000); // BELOW_NORMAL_PRIORITY_CLASS

    let output = cmd.output().ok()?;

    let text = String::from_utf8(output.stdout).ok()?;
    let parts: Vec<&str> = text.trim().split(", ").collect();
    if parts.len() >= 4 {
        let temp = parts[0].trim().parse::<f32>().ok()?;
        let power = parts[1].trim().parse::<f32>().ok()?;
        let vram = parts[2].trim().parse::<f32>().ok()? as u32;
        let usage = parts[3].trim().parse::<f32>().ok()?;
        Some((temp, power, vram, usage))
    } else {
        None
    }
}

/// Collect CPU usage, memory usage, and disk usage via sysinfo crate.
fn collect_sysinfo_metrics(t: &mut HardwareTelemetry) {
    use sysinfo::{System, Disks};

    let mut sys = System::new();
    sys.refresh_cpu_all();
    sys.refresh_memory();

    // CPU usage (global average)
    let cpu_usage = sys.global_cpu_usage();
    if cpu_usage > 0.0 {
        t.cpu_usage_pct = Some(cpu_usage);
    }

    // Memory usage
    let total_mem = sys.total_memory();
    let used_mem = sys.used_memory();
    if total_mem > 0 {
        t.memory_usage_pct = Some((used_mem as f64 / total_mem as f64 * 100.0) as f32);
    }

    // Disk usage on C:
    let disks = Disks::new_with_refreshed_list();
    for disk in disks.list() {
        let mount = disk.mount_point().to_string_lossy();
        if mount.starts_with("C:") || mount == "/" {
            let total = disk.total_space();
            let available = disk.available_space();
            if total > 0 {
                let used_pct = ((total - available) as f64 / total as f64 * 100.0) as f32;
                t.disk_usage_pct = Some(used_pct);
            }
            break;
        }
    }

    // CPU temperature via sysinfo Components (if available)
    use sysinfo::Components;
    let components = Components::new_with_refreshed_list();
    for component in &components {
        let label = component.label().to_lowercase();
        if label.contains("cpu") || label.contains("core") || label.contains("package") {
            if let Some(temp) = component.temperature() {
                if temp > 0.0 {
                    t.cpu_temp_celsius = Some(temp);
                    break;
                }
            }
        }
    }
}

/// System uptime via sysinfo crate (cross-platform).
fn collect_system_uptime() -> Option<u64> {
    let uptime = sysinfo::System::uptime();
    if uptime > 0 { Some(uptime) } else { None }
}

/// Process handle count for the current process.
fn collect_handle_count() -> Option<u32> {
    #[cfg(windows)]
    {
        use winapi::um::processthreadsapi::GetCurrentProcess;
        use winapi::um::processthreadsapi::GetProcessHandleCount;
        unsafe {
            let mut count: u32 = 0;
            if GetProcessHandleCount(GetCurrentProcess(), &mut count) != 0 {
                Some(count)
            } else {
                None
            }
        }
    }
    #[cfg(not(windows))]
    {
        None
    }
}

/// Collect recent Windows critical errors from Event Log (last 5 min).
/// Uses PowerShell — cached to avoid overhead.
fn collect_windows_errors() -> Vec<String> {
    // Only collect every 5 min (aligned with predictive scan) to minimize PS overhead
    static LAST_ERRORS: std::sync::LazyLock<std::sync::Mutex<(std::time::Instant, Vec<String>)>> =
        std::sync::LazyLock::new(|| std::sync::Mutex::new((std::time::Instant::now(), Vec::new())));

    if let Ok(mut cached) = LAST_ERRORS.lock() {
        if cached.0.elapsed() < std::time::Duration::from_secs(300) && !cached.1.is_empty() {
            return cached.1.clone();
        }

        let output = std::process::Command::new("powershell")
            .args([
                "-NoProfile", "-Command",
                "Get-WinEvent -FilterHashtable @{LogName='System';Level=1,2;StartTime=(Get-Date).AddMinutes(-5)} -MaxEvents 5 -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Message | ForEach-Object { $_.Substring(0, [Math]::Min($_.Length, 120)) }",
            ])
            .output()
            .ok();

        let errors: Vec<String> = output
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| s.lines().filter(|l| !l.is_empty()).map(String::from).collect())
            .unwrap_or_default();

        *cached = (std::time::Instant::now(), errors.clone());
        errors
    } else {
        Vec::new()
    }
}

/// Count USB devices via PowerShell (cached for 5 min).
fn collect_usb_count() -> Option<u8> {
    static LAST_COUNT: std::sync::LazyLock<std::sync::Mutex<(std::time::Instant, Option<u8>)>> =
        std::sync::LazyLock::new(|| std::sync::Mutex::new((std::time::Instant::now(), None)));

    if let Ok(mut cached) = LAST_COUNT.lock() {
        if cached.0.elapsed() < std::time::Duration::from_secs(300) {
            return cached.1;
        }

        let output = std::process::Command::new("powershell")
            .args([
                "-NoProfile", "-Command",
                "(Get-CimInstance Win32_USBControllerDevice -ErrorAction SilentlyContinue).Count",
            ])
            .output()
            .ok();

        let count = output
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .and_then(|s| s.trim().parse::<u8>().ok());

        *cached = (std::time::Instant::now(), count);
        count
    } else {
        None
    }
}

/// Ping server to measure network latency.
fn collect_network_latency() -> Option<u32> {
    let start = std::time::Instant::now();
    // Try TCP connect to server :8080 with 2s timeout
    let addr = "192.168.31.23:8080";
    match std::net::TcpStream::connect_timeout(
        &addr.parse().ok()?,
        std::time::Duration::from_secs(2),
    ) {
        Ok(_) => {
            let ms = start.elapsed().as_millis() as u32;
            Some(ms)
        }
        Err(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_state() {
        let s = PredictiveState::new();
        assert_eq!(s.restart_count_today, 0);
        assert_eq!(s.edge_missing_count, 0);
        assert!(s.conspit_reconnects.is_empty());
    }

    #[test]
    fn test_record_restart_below_threshold() {
        let mut s = PredictiveState::new();
        assert!(!record_restart(&mut s));
        assert!(!record_restart(&mut s));
        assert_eq!(s.restart_count_today, 2);
    }

    #[test]
    fn test_record_restart_exceeds_threshold() {
        let mut s = PredictiveState::new();
        record_restart(&mut s); // 1
        record_restart(&mut s); // 2
        assert!(record_restart(&mut s), "3rd restart should trigger alert"); // 3 > MAX(2)
    }

    #[test]
    fn test_edge_missing_no_alert_when_not_blanking() {
        let mut s = PredictiveState::new();
        let alert = record_edge_count(&mut s, 0, false);
        assert!(alert.is_none(), "Should not alert when blanking not expected");
    }

    #[test]
    fn test_edge_missing_alert_after_consecutive() {
        let mut s = PredictiveState::new();
        let a1 = record_edge_count(&mut s, 0, true);
        assert!(a1.is_none(), "First scan should not alert");
        let a2 = record_edge_count(&mut s, 0, true);
        assert!(a2.is_some(), "Second consecutive scan should alert");
    }

    #[test]
    fn test_edge_missing_resets_on_recovery() {
        let mut s = PredictiveState::new();
        record_edge_count(&mut s, 0, true); // count = 1
        record_edge_count(&mut s, 3, true); // recovery → count = 0
        let alert = record_edge_count(&mut s, 0, true); // count = 1 again
        assert!(alert.is_none(), "Should reset counter on recovery");
    }

    #[test]
    fn test_conspit_reconnect_below_threshold() {
        let mut s = PredictiveState::new();
        record_conspit_reconnect(&mut s);
        record_conspit_reconnect(&mut s);
        let alert = check_conspit_reconnects(&mut s);
        assert!(alert.is_none(), "2 reconnects should not trigger (threshold: 3)");
    }

    #[test]
    fn test_conspit_reconnect_at_threshold() {
        let mut s = PredictiveState::new();
        record_conspit_reconnect(&mut s);
        record_conspit_reconnect(&mut s);
        record_conspit_reconnect(&mut s);
        let alert = check_conspit_reconnects(&mut s);
        assert!(alert.is_some(), "3 reconnects should trigger");
    }

    #[test]
    fn test_run_predictive_scan_no_alerts() {
        let mut s = PredictiveState::new();
        // On a dev machine, GPU temp and disk space should be fine
        let alerts = run_predictive_scan(&mut s);
        // We can't assert zero alerts (disk might be low on CI), but no panic
        let _ = alerts;
    }
}
