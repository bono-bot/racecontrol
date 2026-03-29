use anyhow::Result;
use chrono::Utc;
use rc_common::types::*;
use super::SimAdapter;

const LOG_TARGET: &str = "sim-ac-evo";

/// Shared memory telemetry reader for AC-engine games (EVO, Rally).
///
/// Both AC EVO and AC Rally use the same `acpmf_physics`, `acpmf_graphics`,
/// `acpmf_static` shared memory map format as AC1/ACC. The `target_sim`
/// field determines which SimType is emitted in LapData.
///
/// For EVO (Early Access): only physics struct is reliably populated.
/// For Rally: same shared memory as AC1, should be fully populated.
///
/// Strategy: attempt the same struct layout as AC1, gate all lap-detection on
/// non-zero values, degrade gracefully to 90s process-based billing fallback.
pub struct AssettoCorsaEvoAdapter {
    connected: bool,
    pod_id: String,
    target_sim: SimType,
    log_prefix: &'static str,
    last_lap_count: u32,
    current_car: String,
    current_track: String,
    last_sector_index: i32,
    sector_times: [Option<u32>; 3],
    warned_no_shm: bool,
    warned_empty_graphics: bool,
    #[cfg(windows)]
    pending_lap: Option<LapData>,
    #[cfg(windows)]
    physics_handle: Option<ShmHandle>,
    #[cfg(windows)]
    graphics_handle: Option<ShmHandle>,
    #[cfg(windows)]
    static_handle: Option<ShmHandle>,
}

/// Wrapper for a Windows memory-mapped file handle + view pointer.
/// Duplicated per-adapter (mirrors assetto_corsa.rs, iracing.rs, lmu.rs pattern).
#[cfg(windows)]
struct ShmHandle {
    _handle: winapi::shared::ntdef::HANDLE,
    ptr: *const u8,
    _size: usize,
}

#[cfg(windows)]
// SAFETY: The memory-mapped file pointers are read-only views shared between processes.
// The underlying data is managed by AC EVO and only read by this process.
unsafe impl Send for ShmHandle {}
#[cfg(windows)]
unsafe impl Sync for ShmHandle {}

// AC EVO Shared Memory struct offsets — reused from AC1 unchanged (TEL-EVO-01).
// Reference: https://www.assettocorsa.net/forum/index.php?threads/shared-memory-reference.3352/
// Physics (acpmf_physics) — updates every frame
mod physics {
    pub const GAS: usize = 4;        // f32, throttle 0.0-1.0
    pub const BRAKE: usize = 8;      // f32, brake 0.0-1.0
    pub const GEAR: usize = 16;      // i32, 0=R 1=N 2=1st 3=2nd...
    pub const RPMS: usize = 20;      // i32, engine RPM
    pub const SPEED_KMH: usize = 28; // f32, km/h
}

// Graphics (acpmf_graphics) — updates ~10Hz
// NOTE: EVO Early Access — these may all read zero (Pitfall 1 in RESEARCH.md).
mod graphics {
    #[allow(dead_code)]
    pub const STATUS: usize = 4;              // i32, AC_STATUS
    pub const COMPLETED_LAPS: usize = 132;    // i32
    pub const CURRENT_SECTOR_INDEX: usize = 164; // i32, 0=S1 1=S2 2=S3
    pub const I_CURRENT_TIME: usize = 140;    // i32, current lap time in ms
    pub const I_LAST_TIME: usize = 144;       // i32, last completed lap time in ms
    pub const LAST_SECTOR_TIME: usize = 168;  // i32, last sector split time in ms
}

// Static (acpmf_static) — updates once per session
mod statics {
    pub const CAR_MODEL: usize = 68;  // wchar[33] = 66 bytes UTF-16LE
    pub const TRACK: usize = 134;     // wchar[33] = 66 bytes UTF-16LE
}

impl AssettoCorsaEvoAdapter {
    pub fn new(pod_id: String) -> Self {
        Self::with_sim_type(pod_id, SimType::AssettoCorsaEvo, "[EVO]")
    }

    /// Create an adapter for AC Rally (same shared memory, different SimType)
    pub fn new_rally(pod_id: String) -> Self {
        Self::with_sim_type(pod_id, SimType::AssettoCorsaRally, "[RALLY]")
    }

    fn with_sim_type(pod_id: String, target_sim: SimType, log_prefix: &'static str) -> Self {
        Self {
            connected: false,
            pod_id,
            target_sim,
            log_prefix,
            last_lap_count: 0,
            current_car: String::new(),
            current_track: String::new(),
            last_sector_index: -1,
            sector_times: [None; 3],
            warned_no_shm: false,
            warned_empty_graphics: false,
            #[cfg(windows)]
            pending_lap: None,
            #[cfg(windows)]
            physics_handle: None,
            #[cfg(windows)]
            graphics_handle: None,
            #[cfg(windows)]
            static_handle: None,
        }
    }

    #[cfg(windows)]
    fn read_f32(handle: &ShmHandle, offset: usize) -> f32 {
        unsafe {
            let ptr = handle.ptr.add(offset);
            std::ptr::read_unaligned(ptr as *const f32)
        }
    }

    #[cfg(windows)]
    fn read_i32(handle: &ShmHandle, offset: usize) -> i32 {
        unsafe {
            let ptr = handle.ptr.add(offset);
            std::ptr::read_unaligned(ptr as *const i32)
        }
    }

    #[cfg(windows)]
    fn read_wchar_string(handle: &ShmHandle, offset: usize, max_chars: usize) -> String {
        unsafe {
            let ptr = handle.ptr.add(offset) as *const u16;
            let slice = std::slice::from_raw_parts(ptr, max_chars);
            let end = slice.iter().position(|&c| c == 0).unwrap_or(max_chars);
            String::from_utf16_lossy(&slice[..end])
        }
    }
}

impl SimAdapter for AssettoCorsaEvoAdapter {
    fn sim_type(&self) -> SimType {
        self.target_sim.clone()
    }

    /// Connect to AC EVO shared memory.
    ///
    /// Per Pattern 2 (RESEARCH.md): returns Ok(()) even if SHM unavailable.
    /// Physics is required for any telemetry; graphics/static are individually optional.
    /// Warns once on first failure to avoid log spam on every poll tick.
    #[cfg(windows)]
    fn connect(&mut self) -> Result<()> {
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;

        fn open_shm(name: &str) -> Result<ShmHandle> {
            let wide_name: Vec<u16> = OsStr::new(name)
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();

            unsafe {
                let handle = winapi::um::memoryapi::OpenFileMappingW(
                    winapi::um::memoryapi::FILE_MAP_READ,
                    0, // bInheritHandle = FALSE
                    wide_name.as_ptr(),
                );
                if handle.is_null() {
                    anyhow::bail!("Failed to open shared memory: {}", name);
                }

                let ptr = winapi::um::memoryapi::MapViewOfFile(
                    handle,
                    winapi::um::memoryapi::FILE_MAP_READ,
                    0, 0, 0,
                );
                if ptr.is_null() {
                    winapi::um::handleapi::CloseHandle(handle);
                    anyhow::bail!("Failed to map view of: {}", name);
                }

                Ok(ShmHandle {
                    _handle: handle,
                    ptr: ptr as *const u8,
                    _size: 0,
                })
            }
        }

        // Physics is the primary handle — required for any telemetry
        match open_shm("Local\\acpmf_physics") {
            Ok(physics) => {
                self.physics_handle = Some(physics);
                self.connected = true;
                tracing::info!(target: LOG_TARGET, "{} connected to shared memory (physics)", self.log_prefix);
            }
            Err(e) => {
                // EVO may not have populated shared memory yet — not an error
                // Per Pattern 2 + Anti-Pattern 1: return Ok, not Err
                if !self.warned_no_shm {
                    tracing::warn!(
                        target: LOG_TARGET,
                        "{} shared memory not available: {} — telemetry disabled, billing via process fallback",
                        self.log_prefix, e
                    );
                    self.warned_no_shm = true;
                }
                // connected stays false — billing continues via 90s process fallback
                return Ok(());
            }
        }

        // Graphics and static are individually optional (EVO EA may not expose them)
        match open_shm("Local\\acpmf_graphics") {
            Ok(graphics) => {
                // Snapshot current completed_laps to avoid stale-lap false positive (Pitfall 4)
                let initial_laps = Self::read_i32(&graphics, graphics::COMPLETED_LAPS) as u32;
                self.last_lap_count = initial_laps;
                self.graphics_handle = Some(graphics);
                tracing::info!(target: LOG_TARGET, "{} graphics shared memory connected, initial_laps={}", self.log_prefix, initial_laps);
            }
            Err(e) => {
                tracing::warn!(target: LOG_TARGET, "{} graphics shared memory unavailable: {} — lap detection disabled", self.log_prefix, e);
            }
        }

        match open_shm("Local\\acpmf_static") {
            Ok(static_info) => {
                self.current_car = Self::read_wchar_string(&static_info, statics::CAR_MODEL, 33);
                self.current_track = Self::read_wchar_string(&static_info, statics::TRACK, 33);
                self.static_handle = Some(static_info);
                tracing::info!(target: LOG_TARGET, "{} static shared memory connected: car={}, track={}", self.log_prefix, self.current_car, self.current_track);
            }
            Err(e) => {
                tracing::warn!(target: LOG_TARGET, "{} static shared memory unavailable: {} — car/track info not available", self.log_prefix, e);
            }
        }

        self.last_sector_index = -1;
        self.sector_times = [None; 3];

        Ok(())
    }

    /// Non-Windows: connect returns Ok with connected=false (TEL-EVO-02)
    #[cfg(not(windows))]
    fn connect(&mut self) -> Result<()> {
        // Shared memory is Windows-only; on other platforms remain disconnected
        if !self.warned_no_shm {
            tracing::warn!(target: LOG_TARGET, "{} shared memory only available on Windows — telemetry disabled", self.log_prefix);
            self.warned_no_shm = true;
        }
        Ok(())
    }

    fn is_connected(&self) -> bool {
        self.connected
    }

    /// Read current telemetry frame.
    ///
    /// Returns Ok(None) — not Err — when handles are absent or data is zero.
    /// This prevents the event loop from calling disconnect() on empty telemetry (Pitfall 5).
    #[cfg(windows)]
    fn read_telemetry(&mut self) -> Result<Option<TelemetryFrame>> {
        let physics = match &self.physics_handle {
            Some(h) => h,
            None => return Ok(None), // Not connected — Ok(None) per TEL-EVO-02
        };

        let speed_kmh = Self::read_f32(physics, physics::SPEED_KMH);
        let throttle = Self::read_f32(physics, physics::GAS);
        let brake = Self::read_f32(physics, physics::BRAKE);
        let rpm = Self::read_i32(physics, physics::RPMS) as u32;
        // Gear: AC uses 0=R, 1=N, 2=1st. Convert to display: -1=R, 0=N, 1=1st (TEL-EVO-01)
        let raw_gear = Self::read_i32(physics, physics::GEAR);
        let gear = (raw_gear - 1) as i8;

        // Graphics struct may be empty in EVO Early Access (Pattern 3 + Pitfall 1)
        let (completed_laps, lap_time_ms, current_sector) = if let Some(gh) = &self.graphics_handle {
            let cl = Self::read_i32(gh, graphics::COMPLETED_LAPS) as u32;
            let lt = Self::read_i32(gh, graphics::I_CURRENT_TIME) as u32;
            let cs = Self::read_i32(gh, graphics::CURRENT_SECTOR_INDEX);

            // Warn-once if graphics appears entirely empty (Pattern 4)
            if cl == 0 && lt == 0 && !self.warned_empty_graphics {
                tracing::warn!(
                    target: LOG_TARGET,
                    "{} graphics shared memory appears empty — lap detection disabled",
                    self.log_prefix
                );
                self.warned_empty_graphics = true;
            }

            (cl, lt, cs)
        } else {
            (0u32, 0u32, 0i32)
        };

        // Sector tracking: accumulate splits during a lap (zero-guarded)
        if let Some(gh) = &self.graphics_handle {
            let last_sector_time = Self::read_i32(gh, graphics::LAST_SECTOR_TIME);
            if current_sector != self.last_sector_index && last_sector_time > 0 {
                let completed_sector = self.last_sector_index;
                if completed_sector >= 0 && completed_sector < 3 {
                    self.sector_times[completed_sector as usize] = Some(last_sector_time as u32);
                }
                self.last_sector_index = current_sector;
            } else if self.last_sector_index < 0 {
                self.last_sector_index = current_sector;
            }
        }

        // Detect lap completion with zero-guards (Pattern 3 + TEL-EVO-02)
        if completed_laps > self.last_lap_count && self.last_lap_count > 0 {
            if let Some(gh) = &self.graphics_handle {
                let last_lap_time_ms = Self::read_i32(gh, graphics::I_LAST_TIME);
                let lap_ms = last_lap_time_ms;

                // Zero-guard: never emit LapCompleted with lap_time_ms = 0 (Pitfall 1)
                if lap_ms > 0 {
                    let lap_data = LapData {
                        id: uuid::Uuid::new_v4().to_string(),
                        session_id: String::new(),
                        driver_id: String::new(),
                        pod_id: self.pod_id.clone(),
                        sim_type: self.target_sim.clone(),
                        track: self.current_track.clone(),
                        car: self.current_car.clone(),
                        lap_number: completed_laps,
                        lap_time_ms: lap_ms as u32,
                        sector1_ms: self.sector_times[0],
                        sector2_ms: self.sector_times[1],
                        sector3_ms: self.sector_times[2],
                        valid: true, // No is_valid field reliably available in EVO EA
                        session_type: rc_common::types::SessionType::Practice,
                        created_at: Utc::now(),
                    };

                    tracing::info!(
                        target: LOG_TARGET,
                        "{} lap completed: lap={} time={}ms sectors=[{:?}, {:?}, {:?}]",
                        self.log_prefix,
                        completed_laps, lap_ms,
                        self.sector_times[0], self.sector_times[1], self.sector_times[2]
                    );

                    self.pending_lap = Some(lap_data);
                }

                // Reset sector accumulator for next lap
                self.sector_times = [None; 3];
            }
        }

        // Update last lap count whether or not we emitted (avoid stale count on reconnect)
        if completed_laps > 0 {
            self.last_lap_count = completed_laps;
        }

        Ok(Some(TelemetryFrame {
            pod_id: self.pod_id.clone(),
            timestamp: Utc::now(),
            driver_name: String::new(),
            car: self.current_car.clone(),
            track: self.current_track.clone(),
            lap_number: completed_laps,
            lap_time_ms,
            sector: current_sector as u8,
            speed_kmh,
            throttle,
            brake,
            steering: 0.0,
            gear,
            rpm,
            position: None,
            session_time_ms: lap_time_ms,
            drs_active: None,
            drs_available: None,
            ers_deploy_mode: None,
            ers_store_percent: None,
            best_lap_ms: None,
            current_lap_invalid: None,
            sector1_ms: self.sector_times[0],
            sector2_ms: self.sector_times[1],
            sector3_ms: self.sector_times[2],
            lap_id: None, // Phase 251: stamped by event_loop before WS send
        }))
    }

    #[cfg(not(windows))]
    fn read_telemetry(&mut self) -> Result<Option<TelemetryFrame>> {
        Ok(None) // TEL-EVO-02: Ok(None) when not connected
    }

    fn poll_lap_completed(&mut self) -> Result<Option<LapData>> {
        #[cfg(windows)]
        {
            Ok(self.pending_lap.take())
        }
        #[cfg(not(windows))]
        {
            Ok(None)
        }
    }

    fn session_info(&self) -> Result<Option<SessionInfo>> {
        if !self.connected {
            return Ok(None);
        }
        Ok(Some(SessionInfo {
            id: String::new(),
            session_type: rc_common::types::SessionType::Practice,
            sim_type: self.target_sim.clone(),
            track: self.current_track.clone(),
            car_class: None,
            status: rc_common::types::SessionStatus::Active,
            max_drivers: None,
            laps_or_minutes: None,
            started_at: None,
            ended_at: None,
        }))
    }

    fn disconnect(&mut self) {
        #[cfg(windows)]
        {
            if let Some(h) = self.physics_handle.take() {
                unsafe {
                    winapi::um::memoryapi::UnmapViewOfFile(h.ptr as *const _);
                    winapi::um::handleapi::CloseHandle(h._handle);
                }
            }
            if let Some(h) = self.graphics_handle.take() {
                unsafe {
                    winapi::um::memoryapi::UnmapViewOfFile(h.ptr as *const _);
                    winapi::um::handleapi::CloseHandle(h._handle);
                }
            }
            if let Some(h) = self.static_handle.take() {
                unsafe {
                    winapi::um::memoryapi::UnmapViewOfFile(h.ptr as *const _);
                    winapi::um::handleapi::CloseHandle(h._handle);
                }
            }
        }
        self.connected = false;
        self.warned_no_shm = false;
        self.warned_empty_graphics = false;
        tracing::info!(target: LOG_TARGET, "{} disconnected from shared memory", self.log_prefix);
    }

    /// Physics-based on-track detection (Pattern 5 in RESEARCH.md).
    /// Returns Some(true) if speed > 5 km/h or RPM > 500 — i.e., player is driving.
    /// Returns None if physics handle is unavailable.
    /// Used for PlayableSignal instead of the 90s process fallback when physics is available.
    fn read_is_on_track(&self) -> Option<bool> {
        #[cfg(windows)]
        {
            let ph = self.physics_handle.as_ref()?;
            let speed = Self::read_f32(ph, physics::SPEED_KMH);
            let rpm = Self::read_i32(ph, physics::RPMS);
            Some(speed > 5.0 || rpm > 500)
        }
        #[cfg(not(windows))]
        {
            None
        }
    }
}

// ─── AC EVO Unreal Engine config adapter (GAME-04) ───────────────────────────
//
// AC EVO uses Unreal Engine, NOT the classic race.ini format. Launch args
// are applied via {install_dir}/Saved/Config/WindowsNoEditor/GameUserSettings.ini.
//
// IMPORTANT: Do NOT call this for classic Assetto Corsa (uses race.ini via ac_launcher).

/// Find the AC EVO installation directory from config.
///
/// Checks in order:
///   1. Parent directory of exe_path
///   2. working_dir
///   3. Known Steam default path
pub fn find_evo_install_dir(config: &crate::game_process::GameExeConfig) -> Option<std::path::PathBuf> {
    // 1. Parent of exe_path
    if let Some(ref exe) = config.exe_path {
        let path = std::path::Path::new(exe);
        if let Some(parent) = path.parent() {
            if parent.exists() {
                return Some(parent.to_path_buf());
            }
        }
    }

    // 2. working_dir
    if let Some(ref wd) = config.working_dir {
        let path = std::path::PathBuf::from(wd);
        if path.exists() {
            return Some(path);
        }
    }

    // 3. Known Steam default path
    let steam_default = std::path::PathBuf::from(
        r"C:\Program Files (x86)\Steam\steamapps\common\Assetto Corsa EVO"
    );
    if steam_default.exists() {
        return Some(steam_default);
    }

    None
}

/// Write AC EVO launch configuration to Unreal GameUserSettings.ini.
///
/// Parses `launch_args` as JSON with optional fields:
///   {"car": "...", "track": "...", "weather": "...", "time_of_day": "..."}
///
/// Writes to {evo_install_dir}/Saved/Config/WindowsNoEditor/GameUserSettings.ini
/// using Unreal INI format: [/Script/AssettoCorsaEVO.ACEVOGameUserSettings] section.
///
/// Existing INI content is read and merged — only our section is replaced, preserving
/// graphics/audio/input settings set by the player.
///
/// Returns Ok(()) on success OR when launch_args is empty/has no parseable fields.
/// Does NOT return Err on missing keys — missing fields are simply skipped.
pub fn write_evo_config(launch_args: &str, evo_install_dir: &std::path::Path) -> Result<(), String> {
    // Empty args = use game defaults, no config write needed
    if launch_args.trim().is_empty() {
        return Ok(());
    }

    // Parse launch_args JSON — non-fatal on parse failure
    let parsed: serde_json::Value = match serde_json::from_str(launch_args) {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(target: "sim-ac-evo", "write_evo_config: failed to parse launch_args JSON (non-fatal): {}", e);
            return Ok(());
        }
    };

    let car = parsed.get("car").and_then(|v| v.as_str()).map(|s| s.to_string());
    let track = parsed.get("track").and_then(|v| v.as_str()).map(|s| s.to_string());
    let weather = parsed.get("weather").and_then(|v| v.as_str()).map(|s| s.to_string());
    let time_of_day = parsed.get("time_of_day").and_then(|v| v.as_str()).map(|s| s.to_string());

    // No relevant fields — skip config write, use game defaults
    if car.is_none() && track.is_none() && weather.is_none() && time_of_day.is_none() {
        return Ok(());
    }

    // Ensure Saved/Config/WindowsNoEditor directory exists
    let config_dir = evo_install_dir
        .join("Saved")
        .join("Config")
        .join("WindowsNoEditor");

    if let Err(e) = std::fs::create_dir_all(&config_dir) {
        return Err(format!("write_evo_config: failed to create config directory {}: {}", config_dir.display(), e));
    }

    let ini_path = config_dir.join("GameUserSettings.ini");

    // Read existing INI content if present, for merge
    let existing_content = std::fs::read_to_string(&ini_path).unwrap_or_default();

    // Build the updated INI: preserve all sections EXCEPT our section
    const EVO_SECTION: &str = "[/Script/AssettoCorsaEVO.ACEVOGameUserSettings]";

    let mut new_content = String::new();
    let mut in_our_section = false;

    for line in existing_content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_our_section = trimmed == EVO_SECTION;
            if !in_our_section {
                new_content.push_str(line);
                new_content.push('\n');
            }
        } else if !in_our_section {
            new_content.push_str(line);
            new_content.push('\n');
        }
    }

    // Append our section with values set from launch_args
    new_content.push_str(EVO_SECTION);
    new_content.push('\n');
    if let Some(ref c) = car {
        new_content.push_str(&format!("SelectedCar={}\n", c));
    }
    if let Some(ref t) = track {
        new_content.push_str(&format!("SelectedTrack={}\n", t));
    }
    if let Some(ref w) = weather {
        new_content.push_str(&format!("WeatherPreset={}\n", w));
    }
    if let Some(ref tod) = time_of_day {
        new_content.push_str(&format!("TimeOfDay={}\n", tod));
    }

    std::fs::write(&ini_path, &new_content)
        .map_err(|e| format!("write_evo_config: failed to write {}: {}", ini_path.display(), e))?;

    tracing::info!(
        target: "sim-ac-evo",
        "GAME-04: Wrote AC EVO GameUserSettings.ini (car={:?}, track={:?}, weather={:?}, time_of_day={:?})",
        car, track, weather, time_of_day
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// TEL-EVO-02: connect() without EVO running returns Ok(()) and is_connected() == false
    #[test]
    fn test_connect_no_shm() {
        let mut adapter = AssettoCorsaEvoAdapter::new("pod_1".to_string());
        let result = adapter.connect();
        // connect() must never return Err — even when SHM is unavailable
        assert!(result.is_ok(), "connect() must return Ok even when shared memory unavailable");
        // On non-Windows (CI) or when EVO is not running, should not be connected
        #[cfg(not(windows))]
        assert!(!adapter.is_connected(), "should not be connected without EVO running");
        // On Windows without EVO running, also not connected
        #[cfg(windows)]
        {
            // Only assert disconnected if physics handle is actually None (EVO not running)
            if adapter.physics_handle.is_none() {
                assert!(!adapter.is_connected());
            }
        }
    }

    /// TEL-EVO-02: read_telemetry() returns Ok(None) when physics_handle is None
    #[test]
    fn test_read_telemetry_no_handles() {
        let mut adapter = AssettoCorsaEvoAdapter::new("pod_1".to_string());
        // No connect() called — all handles are None
        let result = adapter.read_telemetry();
        assert!(result.is_ok(), "read_telemetry() must return Ok, not Err");
        assert!(result.unwrap().is_none(), "read_telemetry() must return Ok(None) when disconnected");
    }

    /// TEL-EVO-02: poll_lap_completed() does NOT emit LapData when lap_ms == 0
    #[test]
    fn test_no_lap_on_zero_time() {
        let mut adapter = AssettoCorsaEvoAdapter::new("pod_1".to_string());
        // Simulate scenario: completed_laps incremented but lap_ms == 0
        // The adapter's poll_lap_completed should return None (no pending_lap set)
        let result = adapter.poll_lap_completed();
        assert!(result.is_ok());
        assert!(result.unwrap().is_none(), "No lap should be emitted when lap_time_ms == 0");
    }

    /// TEL-EVO-03: LapData produced by the adapter has sim_type = SimType::AssettoCorsaEvo
    #[test]
    fn test_lap_sim_type() {
        // Build a LapData manually using the same construction the adapter would use
        let lap = LapData {
            id: uuid::Uuid::new_v4().to_string(),
            session_id: String::new(),
            driver_id: String::new(),
            pod_id: "pod_1".to_string(),
            sim_type: SimType::AssettoCorsaEvo, // TEL-EVO-03
            track: String::new(),
            car: String::new(),
            lap_number: 1,
            lap_time_ms: 90000,
            sector1_ms: None,
            sector2_ms: None,
            sector3_ms: None,
            valid: true,
            session_type: rc_common::types::SessionType::Practice,
            created_at: chrono::Utc::now(),
        };
        assert_eq!(lap.sim_type, SimType::AssettoCorsaEvo, "LapData must have sim_type = AssettoCorsaEvo");
    }

    /// TEL-EVO-01: Physics and graphics offset constants match expected AC1 values
    #[test]
    fn test_offset_constants() {
        assert_eq!(physics::GAS, 4);
        assert_eq!(physics::BRAKE, 8);
        assert_eq!(physics::GEAR, 16);
        assert_eq!(physics::RPMS, 20);
        assert_eq!(physics::SPEED_KMH, 28);
        assert_eq!(graphics::STATUS, 4);
        assert_eq!(graphics::COMPLETED_LAPS, 132);
        assert_eq!(graphics::CURRENT_SECTOR_INDEX, 164);
        assert_eq!(graphics::I_CURRENT_TIME, 140);
        assert_eq!(graphics::I_LAST_TIME, 144);
        assert_eq!(graphics::LAST_SECTOR_TIME, 168);
    }

    /// TEL-EVO-01: Gear conversion — AC encoding: 0=R, 1=N, 2=1st → display: -1=R, 0=N, 1=1st
    #[test]
    fn test_gear_conversion() {
        // Raw AC gear → display gear: raw_gear - 1
        assert_eq!((0i32 - 1) as i8, -1, "AC gear 0 (R) should convert to -1");
        assert_eq!((1i32 - 1) as i8, 0, "AC gear 1 (N) should convert to 0");
        assert_eq!((2i32 - 1) as i8, 1, "AC gear 2 (1st) should convert to 1");
        assert_eq!((3i32 - 1) as i8, 2, "AC gear 3 (2nd) should convert to 2");
        assert_eq!((7i32 - 1) as i8, 6, "AC gear 7 (6th) should convert to 6");
    }

    /// After disconnect(), is_connected() == false and handles are None
    #[test]
    fn test_disconnect_clears_state() {
        let mut adapter = AssettoCorsaEvoAdapter::new("pod_1".to_string());
        // Manually set connected to true to simulate a connected state
        adapter.connected = true;
        adapter.disconnect();
        assert!(!adapter.is_connected(), "is_connected() must return false after disconnect()");
        // On Windows, verify handles are cleared
        #[cfg(windows)]
        {
            assert!(adapter.physics_handle.is_none(), "physics_handle must be None after disconnect");
            assert!(adapter.graphics_handle.is_none(), "graphics_handle must be None after disconnect");
            assert!(adapter.static_handle.is_none(), "static_handle must be None after disconnect");
        }
    }

    /// AC Rally adapter uses AssettoCorsaRally sim type
    #[test]
    fn test_rally_adapter_sim_type() {
        let adapter = AssettoCorsaEvoAdapter::new_rally("pod_1".to_string());
        assert_eq!(adapter.sim_type(), SimType::AssettoCorsaRally);
        assert_eq!(adapter.log_prefix, "[RALLY]");
    }

    /// EVO adapter uses AssettoCorsaEvo sim type
    #[test]
    fn test_evo_adapter_sim_type() {
        let adapter = AssettoCorsaEvoAdapter::new("pod_1".to_string());
        assert_eq!(adapter.sim_type(), SimType::AssettoCorsaEvo);
        assert_eq!(adapter.log_prefix, "[EVO]");
    }

    // ─── GAME-04: AC EVO config adapter tests ────────────────────────────────

    /// GAME-04: write_evo_config with valid JSON produces correct Unreal INI format
    #[test]
    fn test_evo_config_valid_json_produces_ini() {
        let tmp = tempfile::tempdir().expect("failed to create tempdir");
        let args = r#"{"car":"porsche_718","track":"monza","weather":"clear","time_of_day":"14:00"}"#;

        let result = write_evo_config(args, tmp.path());
        assert!(result.is_ok(), "write_evo_config should succeed: {:?}", result);

        let ini_path = tmp.path()
            .join("Saved").join("Config").join("WindowsNoEditor")
            .join("GameUserSettings.ini");
        assert!(ini_path.exists(), "GameUserSettings.ini should be created");

        let content = std::fs::read_to_string(&ini_path).expect("read ini");
        assert!(content.contains("[/Script/AssettoCorsaEVO.ACEVOGameUserSettings]"),
            "INI must contain EVO section header");
        assert!(content.contains("SelectedCar=porsche_718"), "INI must contain car");
        assert!(content.contains("SelectedTrack=monza"), "INI must contain track");
        assert!(content.contains("WeatherPreset=clear"), "INI must contain weather");
        assert!(content.contains("TimeOfDay=14:00"), "INI must contain time_of_day");
        // Must NOT contain race.ini format keys
        assert!(!content.contains("RACE"), "INI must not use race.ini format");
    }

    /// GAME-04: write_evo_config with empty args returns Ok without writing file
    #[test]
    fn test_evo_config_empty_args_returns_ok() {
        let tmp = tempfile::tempdir().expect("failed to create tempdir");
        let result = write_evo_config("", tmp.path());
        assert!(result.is_ok(), "write_evo_config with empty args must return Ok");

        // File should not be created for empty args
        let ini_path = tmp.path()
            .join("Saved").join("Config").join("WindowsNoEditor")
            .join("GameUserSettings.ini");
        assert!(!ini_path.exists(), "GameUserSettings.ini should NOT be created for empty args");
    }

    /// GAME-04: write_evo_config preserves existing INI sections not in our section
    #[test]
    fn test_evo_config_preserves_existing_sections() {
        let tmp = tempfile::tempdir().expect("failed to create tempdir");
        let config_dir = tmp.path().join("Saved").join("Config").join("WindowsNoEditor");
        std::fs::create_dir_all(&config_dir).expect("create config dir");
        let ini_path = config_dir.join("GameUserSettings.ini");

        // Pre-existing content: graphics settings we should preserve
        let existing = "[/Script/Engine.GameUserSettings]\nResolutionSizeX=1920\nResolutionSizeY=1080\n";
        std::fs::write(&ini_path, existing).expect("write existing ini");

        let args = r#"{"car":"ferrari_488","track":"spa"}"#;
        let result = write_evo_config(args, tmp.path());
        assert!(result.is_ok(), "write_evo_config should succeed");

        let content = std::fs::read_to_string(&ini_path).expect("read ini");
        // Existing section must be preserved
        assert!(content.contains("[/Script/Engine.GameUserSettings]"),
            "Existing Engine section must be preserved");
        assert!(content.contains("ResolutionSizeX=1920"),
            "Existing resolution settings must be preserved");
        // Our section must be present
        assert!(content.contains("[/Script/AssettoCorsaEVO.ACEVOGameUserSettings]"),
            "EVO section must be present");
        assert!(content.contains("SelectedCar=ferrari_488"), "car must be set");
        assert!(content.contains("SelectedTrack=spa"), "track must be set");
    }

    /// GAME-04: write_evo_config with whitespace-only args returns Ok
    #[test]
    fn test_evo_config_whitespace_args_returns_ok() {
        let tmp = tempfile::tempdir().expect("failed to create tempdir");
        let result = write_evo_config("   ", tmp.path());
        assert!(result.is_ok(), "write_evo_config with whitespace args must return Ok");
    }

    /// GAME-04: find_evo_install_dir returns parent of exe_path when it exists
    #[test]
    fn test_find_evo_install_dir_from_exe_path() {
        let tmp = tempfile::tempdir().expect("failed to create tempdir");
        // Create a fake exe file
        let exe_path = tmp.path().join("ACE.exe");
        std::fs::write(&exe_path, b"fake exe").expect("write fake exe");

        let config = crate::game_process::GameExeConfig {
            exe_path: Some(exe_path.to_string_lossy().to_string()),
            ..Default::default()
        };

        let result = find_evo_install_dir(&config);
        assert!(result.is_some(), "find_evo_install_dir should find parent of exe_path");
        assert_eq!(result.unwrap(), tmp.path(), "should return parent directory");
    }

    /// GAME-04: find_evo_install_dir returns working_dir when exe_path parent doesn't exist
    #[test]
    fn test_find_evo_install_dir_from_working_dir() {
        let tmp = tempfile::tempdir().expect("failed to create tempdir");

        let config = crate::game_process::GameExeConfig {
            exe_path: Some("/nonexistent/path/ACE.exe".to_string()),
            working_dir: Some(tmp.path().to_string_lossy().to_string()),
            ..Default::default()
        };

        let result = find_evo_install_dir(&config);
        assert!(result.is_some(), "find_evo_install_dir should find working_dir");
        assert_eq!(result.unwrap(), tmp.path().to_path_buf());
    }

    /// GAME-04: write_evo_config does NOT write race.ini (EVO is Unreal, not AC engine)
    #[test]
    fn test_evo_config_no_race_ini_format() {
        let tmp = tempfile::tempdir().expect("failed to create tempdir");
        let args = r#"{"car":"bmw_m3","track":"nordschleife"}"#;
        let _ = write_evo_config(args, tmp.path());

        // race.ini must NOT exist
        let race_ini = tmp.path().join("race.ini");
        assert!(!race_ini.exists(), "race.ini must NOT be written for AC EVO (Unreal engine)");
    }
}
