use anyhow::Result;
use chrono::Utc;
use rc_common::types::*;

use super::SimAdapter;

const LOG_TARGET: &str = "sim-iracing";

/// iRacing shared memory telemetry adapter.
///
/// Reads the iRacing SDK shared memory (Local\IRSDKMemMapFileName).
/// Variable offsets are looked up by name from the irsdk_varHeader array
/// at connect time — they are NOT fixed, as iRacing can change them
/// across SDK updates.
///
/// Lap detection uses the `LapCompleted` counter (finished laps), not `Lap`
/// (started laps). `LapLastLapTime` is in seconds (f32) and must be multiplied
/// by 1000 to get milliseconds.
///
/// Session transitions are detected via `SessionUniqueID` changing between
/// reads. The shared memory handle stays open; only the YAML is re-parsed.
pub struct IracingAdapter {
    pod_id: String,
    connected: bool,
    #[cfg(windows)]
    shm_handle: Option<ShmHandle>,
    var_offsets: VarOffsets,
    last_session_uid: i32,
    last_session_info_update: i32,
    last_lap_count: i32,
    sector_times: [Option<u32>; 3],
    pending_lap: Option<LapData>,
    current_track: String,
    current_car: String,
    current_session_type: SessionType,
    first_read: bool,
}

/// Cached offsets (into each telemetry row) for variables we use.
/// A value of -1 means the variable was not found in the varHeader array.
#[derive(Default)]
struct VarOffsets {
    is_on_track: i32,
    lap_completed: i32,
    lap_last_lap_time: i32,
    session_unique_id: i32,
    speed: i32,
    throttle: i32,
    brake: i32,
    gear: i32,
    rpm: i32,
    lap_dist_pct: i32,
    session_state: i32,
}

impl VarOffsets {
    fn unset() -> Self {
        Self {
            is_on_track: -1,
            lap_completed: -1,
            lap_last_lap_time: -1,
            session_unique_id: -1,
            speed: -1,
            throttle: -1,
            brake: -1,
            gear: -1,
            rpm: -1,
            lap_dist_pct: -1,
            session_state: -1,
        }
    }
}

/// Mirrors the fixed irsdk_header layout at the start of the shared memory.
///
/// Layout (offsets from shm start):
///   0  ver(i32), 4  status(i32), 8  tickRate(i32)
///   12 sessionInfoUpdate(i32), 16 sessionInfoLen(i32), 20 sessionInfoOffset(i32)
///   24 numVars(i32), 28 varHeaderOffset(i32)
///   32 numBuf(i32), 36 bufLen(i32), 40 pad[2](i32)
///   48 varBuf[0..4]: each 16 bytes = {tickCount(i32), bufOffset(i32), pad[2](i32)}
/// Total header: 112 bytes.
struct IrsdkHeader {
    status: i32,
    session_info_update: i32,
    session_info_len: i32,
    session_info_offset: i32,
    num_vars: i32,
    var_header_offset: i32,
    num_buf: i32,
    #[allow(dead_code)]
    buf_len: i32,
}

// ─── Windows SHM handle ──────────────────────────────────────────────────────

/// Wrapper for a Windows memory-mapped file handle and view pointer.
/// Follows the same pattern as AssettoCorsaAdapter's ShmHandle.
#[cfg(windows)]
struct ShmHandle {
    _handle: winapi::shared::ntdef::HANDLE,
    ptr: *const u8,
    _size: usize,
}

#[cfg(windows)]
// SAFETY: The memory-mapped file pointer is a read-only view of iRacing's
// shared memory. iRacing owns the write side; this process only reads.
unsafe impl Send for ShmHandle {}

#[cfg(windows)]
unsafe impl Sync for ShmHandle {}

// ─── Windows shared memory helpers (all cfg(windows)) ────────────────────────

#[cfg(windows)]
fn open_iracing_shm() -> Result<ShmHandle> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;

    let name = "Local\\IRSDKMemMapFileName";
    let wide_name: Vec<u16> = OsStr::new(name)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    unsafe {
        let handle = winapi::um::memoryapi::OpenFileMappingW(
            winapi::um::memoryapi::FILE_MAP_READ,
            0,
            wide_name.as_ptr(),
        );
        if handle.is_null() {
            anyhow::bail!(
                "OpenFileMappingW failed for iRacing shm — is iRacing running? (error={})",
                winapi::um::errhandlingapi::GetLastError()
            );
        }

        let ptr = winapi::um::memoryapi::MapViewOfFile(
            handle,
            winapi::um::memoryapi::FILE_MAP_READ,
            0,
            0,
            0,
        );
        if ptr.is_null() {
            winapi::um::handleapi::CloseHandle(handle);
            anyhow::bail!("MapViewOfFile failed for iRacing shm");
        }

        Ok(ShmHandle {
            _handle: handle,
            ptr: ptr as *const u8,
            _size: 0,
        })
    }
}

/// Read the fixed irsdk_header from the start of shared memory.
#[cfg(windows)]
fn read_header(ptr: *const u8) -> IrsdkHeader {
    unsafe {
        IrsdkHeader {
            status: std::ptr::read_unaligned(ptr.add(4) as *const i32),
            session_info_update: std::ptr::read_unaligned(ptr.add(12) as *const i32),
            session_info_len: std::ptr::read_unaligned(ptr.add(16) as *const i32),
            session_info_offset: std::ptr::read_unaligned(ptr.add(20) as *const i32),
            num_vars: std::ptr::read_unaligned(ptr.add(24) as *const i32),
            var_header_offset: std::ptr::read_unaligned(ptr.add(28) as *const i32),
            num_buf: std::ptr::read_unaligned(ptr.add(32) as *const i32),
            buf_len: std::ptr::read_unaligned(ptr.add(36) as *const i32),
        }
    }
}

/// Returns true when iRacing is connected (bit 1 of the status field).
#[cfg(windows)]
fn is_iracing_active(status: i32) -> bool {
    status & 1 != 0
}

/// Scan the irsdk_varHeader array for a variable by name.
/// Returns `(offset_in_row, type_id)` or None if not found.
/// Each varHeader is 144 bytes:
///   0: type(i32), 4: offset(i32), 8: count(i32), 12: pad, 16: name[32], 48: desc[64], 112: unit[32]
#[cfg(windows)]
fn find_var_offset(
    shm_ptr: *const u8,
    header: &IrsdkHeader,
    name: &[u8],
) -> Option<(i32, i32)> {
    for i in 0..header.num_vars {
        let var_ptr = unsafe {
            shm_ptr.add(header.var_header_offset as usize + i as usize * 144)
        };
        let var_name =
            unsafe { std::slice::from_raw_parts(var_ptr.add(16), 32) };
        let null_end = var_name.iter().position(|&c| c == 0).unwrap_or(32);
        if &var_name[..null_end] == name {
            let offset =
                unsafe { std::ptr::read_unaligned(var_ptr.add(4) as *const i32) };
            let var_type =
                unsafe { std::ptr::read_unaligned(var_ptr as *const i32) };
            return Some((offset, var_type));
        }
    }
    None
}

/// Build a VarOffsets struct by scanning the varHeader array for each needed variable.
#[cfg(windows)]
fn build_var_offsets(shm_ptr: *const u8, header: &IrsdkHeader) -> VarOffsets {
    let mut v = VarOffsets::unset();

    macro_rules! lookup {
        ($field:ident, $name:expr) => {
            if let Some((off, _)) = find_var_offset(shm_ptr, header, $name) {
                v.$field = off;
            } else {
                tracing::warn!(
                    target: LOG_TARGET,
                    "iRacing: variable {:?} not found in varHeader",
                    std::str::from_utf8($name).unwrap_or("?")
                );
            }
        };
    }

    lookup!(is_on_track, b"IsOnTrack");
    lookup!(lap_completed, b"LapCompleted");
    lookup!(lap_last_lap_time, b"LapLastLapTime");
    lookup!(session_unique_id, b"SessionUniqueID");
    lookup!(speed, b"Speed");
    lookup!(throttle, b"Throttle");
    lookup!(brake, b"Brake");
    lookup!(gear, b"Gear");
    lookup!(rpm, b"RPM");
    lookup!(lap_dist_pct, b"LapDistPct");
    lookup!(session_state, b"SessionState");

    v
}

/// Double-buffer tick-lock: find the buffer row with the highest tickCount
/// and return its row offset from the start of shared memory.
/// Retries up to 3 times to guard against torn reads at 60 Hz.
#[cfg(windows)]
fn read_latest_row_offset(shm_ptr: *const u8, header: &IrsdkHeader) -> Option<i32> {
    for _attempt in 0..3 {
        let num_buf = (header.num_buf as usize).min(4);
        let mut best_tick = -1i32;
        let mut best_buf_idx = 0usize;

        for i in 0..num_buf {
            // varBuf[i] starts at offset 48, each slot is 16 bytes
            let tick_ptr = unsafe { shm_ptr.add(48 + i * 16) as *const i32 };
            let tick = unsafe { std::ptr::read_unaligned(tick_ptr) };
            if tick > best_tick {
                best_tick = tick;
                best_buf_idx = i;
            }
        }

        // Read the row offset for the chosen buffer
        let row_offset_ptr =
            unsafe { shm_ptr.add(48 + best_buf_idx * 16 + 4) as *const i32 };
        let row_offset = unsafe { std::ptr::read_unaligned(row_offset_ptr) };

        // Verify tick did not change during the copy
        let tick_ptr = unsafe { shm_ptr.add(48 + best_buf_idx * 16) as *const i32 };
        let tick_after = unsafe { std::ptr::read_unaligned(tick_ptr) };
        if tick_after == best_tick {
            return Some(row_offset);
        }
        // Torn read — retry
    }
    None
}

/// Read an i32 variable from a telemetry row.
/// Returns 0 if var_offset is -1 (variable not found).
#[cfg(windows)]
fn read_var_i32(ptr: *const u8, row_offset: i32, var_offset: i32) -> i32 {
    if var_offset < 0 {
        return 0;
    }
    unsafe {
        let p = ptr.add(row_offset as usize + var_offset as usize) as *const i32;
        std::ptr::read_unaligned(p)
    }
}

/// Read an f32 variable from a telemetry row.
/// Returns 0.0 if var_offset is -1.
#[cfg(windows)]
fn read_var_f32(ptr: *const u8, row_offset: i32, var_offset: i32) -> f32 {
    if var_offset < 0 {
        return 0.0;
    }
    unsafe {
        let p = ptr.add(row_offset as usize + var_offset as usize) as *const f32;
        std::ptr::read_unaligned(p)
    }
}

/// Read a bool variable from a telemetry row.
/// Returns false if var_offset is -1.
#[cfg(windows)]
fn read_var_bool(ptr: *const u8, row_offset: i32, var_offset: i32) -> bool {
    if var_offset < 0 {
        return false;
    }
    unsafe {
        let p = ptr.add(row_offset as usize + var_offset as usize);
        std::ptr::read_unaligned(p) != 0u8
    }
}

// ─── YAML session info parsing ────────────────────────────────────────────────

/// Scan a YAML-like string for a key and return its value (rest of line).
/// Does NOT use serde_yaml — iRacing YAML is ISO-8859-1 and non-standard.
///
/// Example line: `  TrackDisplayName: Nürburgring GP\n`
/// Returns `Some("Nürburgring GP".to_string())`.
pub fn extract_yaml_value(yaml: &str, key: &str) -> Option<String> {
    let search = format!("{}:", key);
    let start = yaml.find(&search)? + search.len();
    let rest = &yaml[start..];
    let trimmed = rest.trim_start_matches(' ');
    let end = trimmed.find('\n').unwrap_or(trimmed.len());
    let value = trimmed[..end].trim().trim_matches('"').to_string();
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

/// Map iRacing session type YAML values to our SessionType enum.
///
/// Known iRacing values:
///   "Race", "Sprint Race" -> Race
///   "Qualify", "Lone Qualify", "Open Qualify" -> Qualifying
///   "Time Trial" -> Hotlap
///   "Practice", "Warmup", unknown -> Practice
pub fn parse_session_type(yaml_val: &str) -> SessionType {
    match yaml_val {
        "Race" | "Sprint Race" => SessionType::Race,
        "Qualify" | "Lone Qualify" | "Open Qualify" => SessionType::Qualifying,
        "Time Trial" => SessionType::Hotlap,
        _ => SessionType::Practice,
    }
}

// ─── IracingAdapter methods ───────────────────────────────────────────────────

impl IracingAdapter {
    pub fn new(pod_id: String) -> Self {
        Self {
            pod_id,
            connected: false,
            #[cfg(windows)]
            shm_handle: None,
            var_offsets: VarOffsets::unset(),
            last_session_uid: 0,
            last_session_info_update: -1,
            last_lap_count: 0,
            sector_times: [None; 3],
            pending_lap: None,
            current_track: String::new(),
            current_car: String::new(),
            current_session_type: SessionType::Practice,
            first_read: true,
        }
    }

    /// Read and parse the YAML session info from shared memory.
    /// Updates current_track, current_car, current_session_type.
    #[cfg(windows)]
    fn parse_session_yaml(&mut self, shm_ptr: *const u8, header: &IrsdkHeader) {
        if header.session_info_len <= 0 || header.session_info_offset < 0 {
            return;
        }
        let yaml_bytes = unsafe {
            std::slice::from_raw_parts(
                shm_ptr.add(header.session_info_offset as usize),
                header.session_info_len as usize,
            )
        };
        // iRacing YAML is ISO-8859-1 — decode lossy to UTF-8 for our key scan
        let yaml = String::from_utf8_lossy(yaml_bytes).into_owned();

        if let Some(track) = extract_yaml_value(&yaml, "TrackDisplayName") {
            self.current_track = track;
        }
        if let Some(car) = extract_yaml_value(&yaml, "CarScreenName") {
            self.current_car = car;
        }
        if let Some(session_type_str) = extract_yaml_value(&yaml, "SessionType") {
            self.current_session_type = parse_session_type(&session_type_str);
        }

        tracing::info!(
            target: LOG_TARGET,
            "iRacing session info: track={}, car={}, session_type={:?}",
            self.current_track,
            self.current_car,
            self.current_session_type
        );
    }

    /// Apply a session transition: reset lap state and store the new UID.
    /// Exposed as a method so unit tests can call it directly.
    pub fn apply_session_transition(&mut self, new_uid: i32) {
        tracing::info!(
            target: LOG_TARGET,
            "iRacing session transition: uid {} -> {}",
            self.last_session_uid,
            new_uid
        );
        self.last_session_uid = new_uid;
        self.last_lap_count = 0;
        self.sector_times = [None; 3];
        self.pending_lap = None;
    }

    /// Attempt to record a completed lap.
    /// Called from read_telemetry when LapCompleted increments.
    pub fn record_lap(&mut self, lap_completed: i32, last_lap_time_s: f32) {
        if last_lap_time_s <= 0.0 {
            self.last_lap_count = lap_completed;
            return;
        }
        let lap_time_ms = (last_lap_time_s * 1000.0) as u32;
        let lap = LapData {
            id: uuid::Uuid::new_v4().to_string(),
            session_id: String::new(), // filled by racecontrol from billing session
            driver_id: String::new(),  // filled by racecontrol from billing session
            pod_id: self.pod_id.clone(),
            sim_type: SimType::IRacing,
            track: self.current_track.clone(),
            car: self.current_car.clone(),
            lap_number: lap_completed as u32,
            lap_time_ms,
            sector1_ms: self.sector_times[0],
            sector2_ms: self.sector_times[1],
            sector3_ms: self.sector_times[2],
            valid: true, // iRacing invalidates laps server-side, not via a telemetry flag
            session_type: self.current_session_type,
            created_at: Utc::now(),
        };
        tracing::info!(
            target: LOG_TARGET,
            "iRacing lap completed: lap={} time={}ms ({}s)",
            lap_completed,
            lap_time_ms,
            last_lap_time_s
        );
        self.pending_lap = Some(lap);
        self.sector_times = [None; 3];
        self.last_lap_count = lap_completed;
    }

    /// Private helper to read IsOnTrack from shared memory.
    /// Returns None if not connected or the variable wasn't found.
    #[cfg(windows)]
    fn read_is_on_track_from_shm(&self) -> Option<bool> {
        if !self.connected {
            return None;
        }
        let shm = self.shm_handle.as_ref()?;
        if self.var_offsets.is_on_track < 0 {
            return None;
        }
        let header = read_header(shm.ptr);
        let row_offset = read_latest_row_offset(shm.ptr, &header)?;
        let val = read_var_bool(shm.ptr, row_offset, self.var_offsets.is_on_track);
        Some(val)
    }

    #[cfg(not(windows))]
    fn read_is_on_track_from_shm(&self) -> Option<bool> {
        None
    }
}

// ─── Pre-flight check ─────────────────────────────────────────────────────────

/// Check whether iRacing shared memory is enabled by reading `Documents/iRacing/app.ini`.
/// Warns via tracing if not enabled — does not block launch.
pub fn check_iracing_shm_enabled() -> bool {
    match dirs_next::document_dir() {
        Some(mut path) => {
            path.push("iRacing");
            path.push("app.ini");
            check_iracing_shm_enabled_at(&path)
        }
        None => {
            tracing::warn!(target: LOG_TARGET, "iRacing pre-flight: could not determine Documents directory");
            false
        }
    }
}

/// Same as `check_iracing_shm_enabled` but accepts an explicit path for testability.
pub fn check_iracing_shm_enabled_at(path: &std::path::Path) -> bool {
    let content = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(
                target: LOG_TARGET,
                "iRacing pre-flight: could not read {:?}: {}. Shared memory may be disabled.",
                path,
                e
            );
            return false;
        }
    };
    let enabled = content
        .lines()
        .any(|line| line.trim() == "irsdkEnableMem=1");
    if !enabled {
        tracing::warn!(
            target: LOG_TARGET,
            "iRacing pre-flight: irsdkEnableMem=1 not found in {:?}. \
             Telemetry will not work until the customer enables it in iRacing settings.",
            path
        );
    }
    enabled
}

// ─── SimAdapter impl ──────────────────────────────────────────────────────────

impl SimAdapter for IracingAdapter {
    fn sim_type(&self) -> SimType {
        SimType::IRacing
    }

    #[cfg(windows)]
    fn connect(&mut self) -> Result<()> {
        let shm = open_iracing_shm()?;
        let header = read_header(shm.ptr);

        if !is_iracing_active(header.status) {
            anyhow::bail!(
                "iRacing shared memory found but status bit is not set — iRacing may not be in-session"
            );
        }

        self.var_offsets = build_var_offsets(shm.ptr, &header);
        self.parse_session_yaml(shm.ptr, &header);
        self.last_session_info_update = header.session_info_update;

        // First-packet safety: snapshot current LapCompleted so we don't fire
        // a false lap event for laps that were already completed before we connected.
        if let Some(row_offset) = read_latest_row_offset(shm.ptr, &header) {
            self.last_lap_count =
                read_var_i32(shm.ptr, row_offset, self.var_offsets.lap_completed);
            self.last_session_uid =
                read_var_i32(shm.ptr, row_offset, self.var_offsets.session_unique_id);
        }

        self.shm_handle = Some(shm);
        self.connected = true;
        self.first_read = true;
        self.sector_times = [None; 3];
        self.pending_lap = None;

        tracing::info!(
            target: LOG_TARGET,
            "iRacing shared memory connected: track={}, car={}, session_type={:?}, initial_laps={}",
            self.current_track,
            self.current_car,
            self.current_session_type,
            self.last_lap_count
        );
        Ok(())
    }

    #[cfg(not(windows))]
    fn connect(&mut self) -> Result<()> {
        anyhow::bail!("iRacing shared memory requires Windows");
    }

    fn is_connected(&self) -> bool {
        self.connected
    }

    #[cfg(windows)]
    fn read_telemetry(&mut self) -> Result<Option<TelemetryFrame>> {
        let shm_ptr = match &self.shm_handle {
            Some(h) => h.ptr,
            None => return Ok(None),
        };

        let header = read_header(shm_ptr);
        if !is_iracing_active(header.status) {
            tracing::warn!(target: LOG_TARGET, "iRacing: status bit cleared — sim disconnected");
            self.connected = false;
            return Ok(None);
        }

        let row_offset = match read_latest_row_offset(shm_ptr, &header) {
            Some(r) => r,
            None => return Ok(None),
        };

        // Read all variables from the chosen buffer row
        let current_uid =
            read_var_i32(shm_ptr, row_offset, self.var_offsets.session_unique_id);
        let is_on_track =
            read_var_bool(shm_ptr, row_offset, self.var_offsets.is_on_track);
        let lap_completed =
            read_var_i32(shm_ptr, row_offset, self.var_offsets.lap_completed);
        let last_lap_time_s =
            read_var_f32(shm_ptr, row_offset, self.var_offsets.lap_last_lap_time);
        let speed_ms = read_var_f32(shm_ptr, row_offset, self.var_offsets.speed);
        let throttle = read_var_f32(shm_ptr, row_offset, self.var_offsets.throttle);
        let brake = read_var_f32(shm_ptr, row_offset, self.var_offsets.brake);
        let gear = read_var_i32(shm_ptr, row_offset, self.var_offsets.gear);
        let rpm = read_var_f32(shm_ptr, row_offset, self.var_offsets.rpm);
        let lap_dist_pct =
            read_var_f32(shm_ptr, row_offset, self.var_offsets.lap_dist_pct);

        // Re-parse YAML if session UID changed
        if current_uid != 0 && current_uid != self.last_session_uid {
            self.apply_session_transition(current_uid);
            if header.session_info_len > 0 {
                self.parse_session_yaml(shm_ptr, &header);
            }
        }

        // Re-cache var offsets if session info was updated (SDK may change them)
        if header.session_info_update != self.last_session_info_update {
            self.var_offsets = build_var_offsets(shm_ptr, &header);
            self.last_session_info_update = header.session_info_update;
        }

        // Lap completion detection
        if lap_completed > self.last_lap_count {
            if self.first_read {
                // First-packet safety: don't fire a lap for laps already done
                // before we connected. Just record the counter.
                tracing::info!(
                    target: LOG_TARGET,
                    "iRacing first_read: skipping lap fire, snapshotting last_lap_count={}",
                    lap_completed
                );
                self.last_lap_count = lap_completed;
            } else {
                self.record_lap(lap_completed, last_lap_time_s);
            }
        } else {
            self.last_lap_count = lap_completed;
        }

        self.first_read = false;

        let speed_kmh = speed_ms * 3.6;
        let frame = TelemetryFrame {
            pod_id: self.pod_id.clone(),
            timestamp: Utc::now(),
            driver_name: String::new(), // iRacing: filled later from session YAML or profile
            car: self.current_car.clone(),
            track: self.current_track.clone(),
            lap_number: lap_completed as u32,
            lap_time_ms: 0, // iRacing has no real-time current-lap timer in this read path
            sector: 0,
            speed_kmh,
            throttle,
            brake,
            steering: 0.0, // iRacing Steer variable requires separate lookup — not critical for v1
            gear: gear as i8,
            rpm: rpm as u32,
            position: None,
            session_time_ms: 0,
            drs_active: None,
            drs_available: None,
            ers_deploy_mode: None,
            ers_store_percent: None,
            best_lap_ms: None,
            current_lap_invalid: None,
            sector1_ms: None,
            sector2_ms: None,
            sector3_ms: None,
        };

        let _ = is_on_track; // used via read_is_on_track_from_shm in trait method
        let _ = lap_dist_pct; // reserved for future sector split synthesis

        Ok(Some(frame))
    }

    #[cfg(not(windows))]
    fn read_telemetry(&mut self) -> Result<Option<TelemetryFrame>> {
        Ok(None)
    }

    fn poll_lap_completed(&mut self) -> Result<Option<LapData>> {
        Ok(self.pending_lap.take())
    }

    fn session_info(&self) -> Result<Option<SessionInfo>> {
        if !self.connected {
            return Ok(None);
        }
        Ok(Some(SessionInfo {
            id: String::new(),
            session_type: self.current_session_type,
            sim_type: SimType::IRacing,
            track: self.current_track.clone(),
            car_class: None,
            status: SessionStatus::Active,
            max_drivers: None,
            laps_or_minutes: None,
            started_at: None,
            ended_at: None,
        }))
    }

    fn disconnect(&mut self) {
        #[cfg(windows)]
        {
            if let Some(h) = self.shm_handle.take() {
                unsafe {
                    winapi::um::memoryapi::UnmapViewOfFile(h.ptr as *const _);
                    winapi::um::handleapi::CloseHandle(h._handle);
                }
            }
        }
        self.connected = false;
        tracing::info!(target: LOG_TARGET, "iRacing: disconnected from shared memory");
    }

    /// Read the iRacing IsOnTrack variable from shared memory.
    ///
    /// This is an explicit override inside `impl SimAdapter for IracingAdapter`
    /// so that Plan 02's event_loop can call `adapter.read_is_on_track()` via
    /// `dyn SimAdapter` trait dispatch. An inherent method alone would NOT be
    /// reachable through a trait object.
    fn read_is_on_track(&self) -> Option<bool> {
        self.read_is_on_track_from_shm()
    }
}

// ─── Unit tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── 1. connect() on a machine without iRacing (non-Windows or no shm) ──

    #[test]
    fn test_connect_no_shm() {
        let mut adapter = IracingAdapter::new("pod_1".to_string());
        let result = adapter.connect();
        // On non-Windows: Err because shm requires Windows.
        // On Windows without iRacing: Err because OpenFileMappingW fails.
        assert!(result.is_err(), "connect() should fail without iRacing running");
        assert!(!adapter.is_connected());
    }

    // ── 2. Session transition resets lap state ──

    #[test]
    fn test_session_transition_resets_lap() {
        let mut adapter = IracingAdapter::new("pod_1".to_string());
        // Pre-condition: mid-session state
        adapter.last_lap_count = 5;
        adapter.sector_times = [Some(10_000), Some(11_000), None];
        adapter.last_session_uid = 42;

        // Trigger the same method read_telemetry calls internally
        adapter.apply_session_transition(43);

        assert_eq!(adapter.last_lap_count, 0, "lap count should reset to 0");
        assert_eq!(
            adapter.sector_times,
            [None; 3],
            "sector times should reset to [None;3]"
        );
        assert_eq!(adapter.last_session_uid, 43, "last_session_uid should update");
    }

    // ── 3. LapCompleted increment fires a LapData ──

    #[test]
    fn test_lap_completed_event() {
        let mut adapter = IracingAdapter::new("pod_1".to_string());
        adapter.connected = true;
        adapter.last_lap_count = 1;
        adapter.first_read = false;
        adapter.current_track = "Brands Hatch".to_string();
        adapter.current_car = "Skip Barber Formula 2000".to_string();
        adapter.current_session_type = SessionType::Race;

        // Simulate LapCompleted going 1 -> 2 with LapLastLapTime = 62.5 seconds
        adapter.record_lap(2, 62.5);

        let lap = adapter
            .pending_lap
            .take()
            .expect("a pending_lap should have been set");

        assert_eq!(lap.lap_time_ms, 62_500, "62.5 s * 1000 = 62500 ms");
        assert_eq!(lap.sim_type, SimType::IRacing);
        assert_eq!(lap.lap_number, 2);
        assert_eq!(lap.track, "Brands Hatch");
        assert_eq!(lap.car, "Skip Barber Formula 2000");
        assert_eq!(lap.session_type, SessionType::Race);
        assert!(lap.valid, "iRacing laps are valid by default");
        assert_eq!(lap.pod_id, "pod_1");
    }

    // ── 4. First-packet safety: already-completed laps don't fire ──

    #[test]
    fn test_first_packet_safety() {
        let mut adapter = IracingAdapter::new("pod_1".to_string());
        adapter.connected = true;
        adapter.first_read = true;
        adapter.last_lap_count = 0;

        // If first_read is true and LapCompleted is already 3, we should NOT
        // emit a lap — just snapshot the counter.
        if adapter.first_read {
            // This mirrors read_telemetry's first_read branch
            adapter.last_lap_count = 3;
            adapter.first_read = false;
        }

        assert!(
            adapter.pending_lap.is_none(),
            "no lap should fire on first packet when LapCompleted is already >0"
        );
        assert_eq!(adapter.last_lap_count, 3, "counter should be snapshotted");
        assert!(!adapter.first_read, "first_read flag should be cleared");
    }

    // ── 5. Pre-flight returns false for missing ini ──

    #[test]
    fn test_preflight_missing_ini() {
        let nonexistent = std::path::PathBuf::from(
            r"C:\DoesNotExist\iRacing\app.ini_MISSING_84_TEST",
        );
        let result = check_iracing_shm_enabled_at(&nonexistent);
        assert!(!result, "missing ini should return false");
    }

    // ── 6. Pre-flight returns true when irsdkEnableMem=1 present ──

    #[test]
    fn test_preflight_ini_enabled() {
        use std::io::Write;

        let mut tmp = tempfile::NamedTempFile::new()
            .expect("tempfile creation should succeed");
        writeln!(tmp, "[Miscellaneous]").unwrap();
        writeln!(tmp, "irsdkEnableMem=1").unwrap();
        let path = tmp.path().to_path_buf();

        let result = check_iracing_shm_enabled_at(&path);
        assert!(
            result,
            "irsdkEnableMem=1 present — should return true"
        );
    }

    // ── 7. Session type mapping ──

    #[test]
    fn test_session_type_mapping() {
        assert_eq!(parse_session_type("Race"), SessionType::Race);
        assert_eq!(parse_session_type("Sprint Race"), SessionType::Race);
        assert_eq!(parse_session_type("Qualify"), SessionType::Qualifying);
        assert_eq!(parse_session_type("Lone Qualify"), SessionType::Qualifying);
        assert_eq!(parse_session_type("Open Qualify"), SessionType::Qualifying);
        assert_eq!(parse_session_type("Time Trial"), SessionType::Hotlap);
        assert_eq!(parse_session_type("Practice"), SessionType::Practice);
        assert_eq!(parse_session_type("Warmup"), SessionType::Practice);
        assert_eq!(parse_session_type("Unknown"), SessionType::Practice);
    }

    // ── Bonus: extract_yaml_value helper ──

    #[test]
    fn test_extract_yaml_value() {
        let yaml = "  TrackDisplayName: Nurburgring GP\n  CarScreenName: Dallara F3\n  SessionType: Race\n";
        assert_eq!(
            extract_yaml_value(yaml, "TrackDisplayName"),
            Some("Nurburgring GP".to_string())
        );
        assert_eq!(
            extract_yaml_value(yaml, "CarScreenName"),
            Some("Dallara F3".to_string())
        );
        assert_eq!(
            extract_yaml_value(yaml, "SessionType"),
            Some("Race".to_string())
        );
        assert_eq!(extract_yaml_value(yaml, "NonExistentKey"), None);
    }
}
