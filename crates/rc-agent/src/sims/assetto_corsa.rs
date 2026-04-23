use anyhow::Result;
use chrono::Utc;
use rc_common::types::*;
use rc_common::types::AcStatus;
use super::SimAdapter;

const LOG_TARGET: &str = "sim-ac";

/// Assetto Corsa shared memory telemetry reader.
///
/// Reads AC's memory-mapped files (acpmf_physics, acpmf_graphics, acpmf_static)
/// which are always available when AC is running and support multiple readers.
///
/// ADAPTER-SHM-01 (2026-04-23, James): All SHM access is per-poll ephemeral —
/// every read_telemetry() / read_ac_status() / etc. performs a fresh
/// OpenFileMappingW + MapViewOfFile + memcpy + UnmapViewOfFile + CloseHandle.
/// No handles are cached across polls. This eliminates the "stale section"
/// class of bug where AC recreates the SHM backing (new session, car change,
/// content-manager restart) but our cached MapViewOfFile pointer still refers
/// to the old kernel pages. Symptoms previously observed: HUD shows frozen
/// values, `completed_laps` never increments, `total_laps` stays at 0 for the
/// entire session while the live SHM page has fresh data at canonical offsets.
/// Per-poll cost: ~3 syscalls × 3 pages × 10 Hz ≈ 90 syscalls/sec — negligible.
pub struct AssettoCorsaAdapter {
    connected: bool,
    pod_id: String,
    last_lap_count: u32,
    current_driver: String,
    current_car: String,
    current_track: String,
    max_rpm: u32,
    // Sector tracking: accumulate splits during a lap
    last_sector_index: i32,
    sector_times: [Option<u32>; 3], // S1, S2, S3 in ms
    // Completed lap ready for pickup
    #[cfg(windows)]
    pending_lap: Option<LapData>,
    /// True if the RaceControl AC plugin (rcpmf_telemetry) is present.
    /// When true, read_telemetry() uses the plugin buffer instead of raw AC pages.
    using_rc_plugin: bool,
    /// Telemetry read counter — used for periodic diagnostic logging and for
    /// throttled re-reading of static fields (car/track/driver) to pick up
    /// mid-session changes without paying the cost every poll.
    telemetry_read_count: u64,
    /// LOG-SPAM-FIX: True if "plugin not found" was already logged once.
    plugin_missing_logged: bool,
}

// AC Shared Memory struct offsets
// Reference: https://www.assettocorsa.net/forum/index.php?threads/shared-memory-reference.3352/
// Physics (acpmf_physics) — updates every frame
mod physics {
    pub const GAS: usize = 4;        // f32, throttle 0.0-1.0
    pub const BRAKE: usize = 8;      // f32, brake 0.0-1.0
    pub const GEAR: usize = 16;      // i32, 0=R 1=N 2=1st 3=2nd...
    pub const RPMS: usize = 20;      // i32, engine RPM
    pub const STEER_ANGLE: usize = 24; // f32, radians
    pub const SPEED_KMH: usize = 28;  // f32, km/h
    // Assist state fields (Phase 6: mid-session controls)
    pub const TC: usize = 204;           // f32, 0.0=off, >0=active level
    pub const ABS: usize = 252;          // f32, 0.0=off, >0=active level
    pub const AUTO_SHIFTER_ON: usize = 264; // i32, 0=manual, 1=auto
}

// Graphics (acpmf_graphics) — updates ~10Hz
mod graphics {
    pub const STATUS: usize = 4;              // i32, AC_STATUS: 0=OFF 1=REPLAY 2=LIVE 3=PAUSE
    pub const COMPLETED_LAPS: usize = 132;    // i32
    pub const CURRENT_SECTOR_INDEX: usize = 164; // i32, 0=S1 1=S2 2=S3
    pub const I_CURRENT_TIME: usize = 140;    // i32, current lap time in ms
    pub const I_LAST_TIME: usize = 144;       // i32, last completed lap time in ms
    pub const I_BEST_TIME: usize = 148;       // i32, session best lap time in ms
    #[allow(dead_code)]
    pub const IS_IN_PIT: usize = 160;         // i32, 1 if in pit lane
    pub const LAST_SECTOR_TIME: usize = 168;  // i32, last sector split time in ms
    #[allow(dead_code)]
    pub const NUMBER_OF_LAPS: usize = 172;    // i32
    pub const NORMALIZED_CAR_POSITION: usize = 248; // f32, 0.0-1.0
    pub const IS_VALID_LAP: usize = 180;      // i32 (approximate offset in truncated page)
}

// Static (acpmf_static) — updates once per session
mod statics {
    pub const CAR_MODEL: usize = 68;    // wchar[33] = 66 bytes UTF-16LE
    pub const TRACK: usize = 134;       // wchar[33] = 66 bytes UTF-16LE
    pub const PLAYER_NAME: usize = 200; // wchar[33] = 66 bytes UTF-16LE
    #[allow(dead_code)]
    pub const NUM_SECTORS: usize = 398; // i32, sector count (usually 3)
    pub const MAX_RPM: usize = 410;     // i32, car's max RPM
}

// Snapshot sizes — large enough to cover every offset we read from each page.
// AC's actual SHM pages are 512 bytes (acpmf_physics/graphics/static) but we
// over-allocate 1024/2048 to cover any offset extensions we add later.
const PHYSICS_SNAPSHOT_SIZE: usize = 1024;
const GRAPHICS_SNAPSHOT_SIZE: usize = 2048;
const STATIC_SNAPSHOT_SIZE: usize = 512;
const PLUGIN_SNAPSHOT_SIZE: usize = 512;

impl AssettoCorsaAdapter {
    pub fn new(pod_id: String, _server_ip: String, _server_port: u16) -> Self {
        Self {
            connected: false,
            pod_id,
            last_lap_count: 0,
            current_driver: String::new(),
            current_car: String::new(),
            current_track: String::new(),
            max_rpm: 8000,
            last_sector_index: -1,
            sector_times: [None; 3],
            #[cfg(windows)]
            pending_lap: None,
            using_rc_plugin: false,
            telemetry_read_count: 0,
            plugin_missing_logged: false,
        }
    }

    /// Open a named SHM section, map a view, copy `size` bytes into an owned
    /// Vec<u8>, then unmap + close. Returns None if the section does not exist
    /// or the view cannot be mapped. The returned buffer is always safe to
    /// read from — the dangerous cross-process pointer is dropped before
    /// return.
    ///
    /// This function is the only SHM ingress point for this adapter. Because
    /// every call re-resolves the name to the current kernel section object,
    /// it automatically picks up AC's re-created sections (new session, car
    /// change, content-manager relaunch) without any reconnect handshake.
    #[cfg(windows)]
    fn open_snapshot(name: &str, size: usize) -> Option<Vec<u8>> {
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;
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
                return None;
            }
            let ptr = winapi::um::memoryapi::MapViewOfFile(
                handle,
                winapi::um::memoryapi::FILE_MAP_READ,
                0, 0, 0,
            );
            if ptr.is_null() {
                winapi::um::handleapi::CloseHandle(handle);
                return None;
            }
            // Pre-check the pointer range before the memcpy. If AC tore the
            // section down between OpenFileMapping and this point, IsBadReadPtr
            // returns non-zero and we bail cleanly instead of faulting.
            let bad = winapi::um::winbase::IsBadReadPtr(
                ptr as *const winapi::ctypes::c_void,
                size,
            );
            if bad != 0 {
                winapi::um::memoryapi::UnmapViewOfFile(ptr);
                winapi::um::handleapi::CloseHandle(handle);
                return None;
            }
            let mut buf = vec![0u8; size];
            std::ptr::copy_nonoverlapping(ptr as *const u8, buf.as_mut_ptr(), size);
            winapi::um::memoryapi::UnmapViewOfFile(ptr);
            winapi::um::handleapi::CloseHandle(handle);
            Some(buf)
        }
    }

    #[cfg(not(windows))]
    fn open_snapshot(_name: &str, _size: usize) -> Option<Vec<u8>> {
        None
    }

    fn read_f32_buf(buf: &[u8], offset: usize) -> f32 {
        if offset + 4 > buf.len() { return 0.0; }
        f32::from_le_bytes([buf[offset], buf[offset+1], buf[offset+2], buf[offset+3]])
    }

    fn read_i32_buf(buf: &[u8], offset: usize) -> i32 {
        if offset + 4 > buf.len() { return 0; }
        i32::from_le_bytes([buf[offset], buf[offset+1], buf[offset+2], buf[offset+3]])
    }

    /// Decode a UTF-16LE wchar array starting at `offset`, up to `max_chars`.
    fn read_wchar_buf(buf: &[u8], offset: usize, max_chars: usize) -> String {
        let byte_len = max_chars * 2;
        if offset + byte_len > buf.len() { return String::new(); }
        let mut u16s: Vec<u16> = Vec::with_capacity(max_chars);
        for i in 0..max_chars {
            let lo = buf[offset + i*2];
            let hi = buf[offset + i*2 + 1];
            let c = u16::from_le_bytes([lo, hi]);
            if c == 0 { break; }
            u16s.push(c);
        }
        String::from_utf16_lossy(&u16s)
    }

    /// Probe whether AC's acpmf_physics section is currently open in the
    /// kernel. Cheap OpenFileMappingW + CloseHandle — no view mapped. Used
    /// as a fast early-exit in read_telemetry before the heavier snapshot.
    #[cfg(windows)]
    fn verify_shm_alive(&self) -> bool {
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;
        let name: Vec<u16> = OsStr::new("Local\\acpmf_physics")
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        unsafe {
            let handle = winapi::um::memoryapi::OpenFileMappingW(
                winapi::um::memoryapi::FILE_MAP_READ,
                0,
                name.as_ptr(),
            );
            if handle.is_null() {
                return false;
            }
            winapi::um::handleapi::CloseHandle(handle);
            true
        }
    }

    #[cfg(not(windows))]
    fn verify_shm_alive(&self) -> bool { true }

    /// Cheap probe of a specific SHM name (open + close, no view).
    #[cfg(windows)]
    fn shm_name_exists(name: &str) -> bool {
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;
        let wide: Vec<u16> = OsStr::new(name)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        unsafe {
            let handle = winapi::um::memoryapi::OpenFileMappingW(
                winapi::um::memoryapi::FILE_MAP_READ,
                0,
                wide.as_ptr(),
            );
            if handle.is_null() {
                return false;
            }
            winapi::um::handleapi::CloseHandle(handle);
            true
        }
    }

    #[cfg(not(windows))]
    fn shm_name_exists(_name: &str) -> bool { false }

    /// Refresh cached static fields (car/track/driver/max_rpm) from the current
    /// acpmf_static page. Called at connect() and periodically from
    /// read_telemetry() so that mid-session car swaps are reflected in the HUD
    /// and in subsequent TelemetryFrame records.
    fn refresh_static_fields(&mut self) {
        let Some(buf) = Self::open_snapshot("Local\\acpmf_static", STATIC_SNAPSHOT_SIZE) else {
            return;
        };
        let car = Self::read_wchar_buf(&buf, statics::CAR_MODEL, 33);
        let track = Self::read_wchar_buf(&buf, statics::TRACK, 33);
        let driver = Self::read_wchar_buf(&buf, statics::PLAYER_NAME, 33);
        let raw_max_rpm = Self::read_i32_buf(&buf, statics::MAX_RPM);
        if !car.is_empty() { self.current_car = car; }
        if !track.is_empty() { self.current_track = track; }
        if !driver.is_empty() { self.current_driver = driver; }
        if raw_max_rpm > 0 { self.max_rpm = raw_max_rpm as u32; }
    }

    /// Refresh cached static fields from the RC plugin's rcpmf_telemetry page.
    fn refresh_static_fields_from_plugin(&mut self, buf: &[u8]) {
        // Offsets from RcTelemetryPage ctypes layout (_pack_=4):
        //   36:  car_model     (wchar[33])
        //  102:  track_name    (wchar[33])
        //  234:  driver_name   (wchar[33])
        //  304:  max_rpm       (i32)
        let car = Self::read_wchar_buf(buf, 36, 33);
        let track = Self::read_wchar_buf(buf, 102, 33);
        let driver = Self::read_wchar_buf(buf, 234, 33);
        let raw_max_rpm = Self::read_i32_buf(buf, 304);
        if !car.is_empty() { self.current_car = car; }
        if !track.is_empty() { self.current_track = track; }
        if !driver.is_empty() { self.current_driver = driver; }
        if raw_max_rpm > 0 { self.max_rpm = raw_max_rpm as u32; }
    }

    /// Read telemetry from the RaceControl AC plugin via rcpmf_telemetry.
    #[cfg(windows)]
    fn read_telemetry_from_plugin(&mut self) -> Result<Option<TelemetryFrame>> {
        let buf = match Self::open_snapshot("rcpmf_telemetry", PLUGIN_SNAPSHOT_SIZE)
            .or_else(|| Self::open_snapshot("Local\\rcpmf_telemetry", PLUGIN_SNAPSHOT_SIZE))
        {
            Some(b) => b,
            None => {
                // Plugin gone — fall back to direct path on the next poll.
                self.using_rc_plugin = false;
                return Ok(None);
            }
        };

        let mem_status = Self::read_i32_buf(&buf, 0);
        match mem_status {
            2 => {} // RC_SM_IDLE
            1 => return Ok(None), // RC_SM_WRITING — skip this tick
            3 | 0 => {
                tracing::warn!(target: LOG_TARGET, "RC plugin mem_status={} — disconnecting", mem_status);
                self.using_rc_plugin = false;
                return Ok(None);
            }
            _ => return Ok(None),
        }

        let ac_status = Self::read_i32_buf(&buf, 16);
        if ac_status == 0 {
            return Ok(None); // AC_OFF
        }

        // Refresh static-ish fields every ~5s (50 reads at 10Hz) and also on
        // first read to pick up the plugin's current car/track/driver.
        if self.telemetry_read_count == 0 || self.telemetry_read_count % 50 == 1 {
            self.refresh_static_fields_from_plugin(&buf);
        }

        let is_in_pit = Self::read_i32_buf(&buf, 28) != 0;
        let is_in_pit_lane = Self::read_i32_buf(&buf, 32) != 0;

        let speed_kmh = Self::read_f32_buf(&buf, 312);
        let rpm = Self::read_i32_buf(&buf, 316) as u32;
        let raw_gear = Self::read_i32_buf(&buf, 320);
        let gear = (raw_gear - 1) as i8;
        let throttle = Self::read_f32_buf(&buf, 324);
        let brake = Self::read_f32_buf(&buf, 328);
        let steering = Self::read_f32_buf(&buf, 332);
        let completed_laps = Self::read_i32_buf(&buf, 340) as u32;
        let lap_time_ms = Self::read_i32_buf(&buf, 344) as u32;
        let last_lap_time_ms = Self::read_i32_buf(&buf, 348);
        let best_lap_ms = Self::read_i32_buf(&buf, 352);
        let current_sector = Self::read_i32_buf(&buf, 356);
        let last_sector_time = Self::read_i32_buf(&buf, 360);
        let is_valid = Self::read_i32_buf(&buf, 364);
        let normalized_pos = Self::read_f32_buf(&buf, 368);

        if current_sector != self.last_sector_index && last_sector_time > 0 {
            let completed_sector = self.last_sector_index;
            if (0..3).contains(&completed_sector) {
                self.sector_times[completed_sector as usize] = Some(last_sector_time as u32);
            }
            self.last_sector_index = current_sector;
        } else if self.last_sector_index < 0 {
            self.last_sector_index = current_sector;
        }

        self.telemetry_read_count += 1;
        if self.telemetry_read_count % 300 == 1 {
            tracing::info!(
                target: LOG_TARGET,
                "plugin telemetry: completed_laps={} last_lap_count={} lap_time_ms={} last_lap_ms={} speed={:.1} pos={:.3} sector={}",
                completed_laps, self.last_lap_count, lap_time_ms, last_lap_time_ms, speed_kmh, normalized_pos, current_sector
            );
        }

        if completed_laps > self.last_lap_count {
            let lap_ms = if last_lap_time_ms > 0 { last_lap_time_ms as u32 } else { 0 };
            if lap_ms > 0 {
                let lap_data = LapData {
                    id: uuid::Uuid::new_v4().to_string(),
                    session_id: String::new(),
                    driver_id: String::new(),
                    pod_id: self.pod_id.clone(),
                    sim_type: SimType::AssettoCorsa,
                    track: self.current_track.clone(),
                    car: self.current_car.clone(),
                    lap_number: completed_laps,
                    lap_time_ms: lap_ms,
                    sector1_ms: self.sector_times[0],
                    sector2_ms: self.sector_times[1],
                    sector3_ms: self.sector_times[2],
                    valid: is_valid != 0,
                    session_type: SessionType::Practice,
                    created_at: Utc::now(),
                };
                tracing::info!(
                    target: LOG_TARGET,
                    "AC lap completed (plugin): lap={} time={}ms valid={}",
                    completed_laps, lap_ms, is_valid != 0
                );
                self.pending_lap = Some(lap_data);
            }
            self.sector_times = [None; 3];
        }
        self.last_lap_count = completed_laps;

        Ok(Some(TelemetryFrame {
            pod_id: self.pod_id.clone(),
            timestamp: Utc::now(),
            driver_name: self.current_driver.clone(),
            car: self.current_car.clone(),
            track: self.current_track.clone(),
            lap_number: completed_laps,
            lap_time_ms,
            sector: current_sector as u8,
            speed_kmh,
            throttle,
            brake,
            steering,
            gear,
            rpm,
            position: None,
            session_time_ms: lap_time_ms,
            drs_active: None,
            drs_available: None,
            ers_deploy_mode: None,
            ers_store_percent: None,
            best_lap_ms: if best_lap_ms > 0 { Some(best_lap_ms as u32) } else { None },
            current_lap_invalid: Some(is_valid == 0),
            sector1_ms: self.sector_times[0],
            sector2_ms: self.sector_times[1],
            sector3_ms: self.sector_times[2],
            lap_id: None,
            sim_type: Some(SimType::AssettoCorsa),
            normalized_car_position: if (0.0..=1.0).contains(&normalized_pos) {
                Some(normalized_pos)
            } else {
                None
            },
            is_in_pit: Some(is_in_pit || is_in_pit_lane),
        }))
    }

    #[cfg(not(windows))]
    fn read_telemetry_from_plugin(&mut self) -> Result<Option<TelemetryFrame>> {
        Ok(None)
    }
}

impl SimAdapter for AssettoCorsaAdapter {
    fn sim_type(&self) -> SimType {
        SimType::AssettoCorsa
    }

    #[cfg(windows)]
    fn connect(&mut self) -> Result<()> {
        // Try RC plugin first (safe — plugin manages SHM lifecycle).
        if Self::shm_name_exists("rcpmf_telemetry") || Self::shm_name_exists("Local\\rcpmf_telemetry") {
            tracing::info!(target: LOG_TARGET, "RC AC plugin detected — using rcpmf_telemetry");
            if let Some(buf) = Self::open_snapshot("rcpmf_telemetry", PLUGIN_SNAPSHOT_SIZE)
                .or_else(|| Self::open_snapshot("Local\\rcpmf_telemetry", PLUGIN_SNAPSHOT_SIZE))
            {
                self.refresh_static_fields_from_plugin(&buf);
                // Seed last_lap_count from current completed_laps to avoid false
                // fire of N past laps when reconnecting mid-session.
                let initial_laps = Self::read_i32_buf(&buf, 340) as u32;
                self.last_lap_count = initial_laps;
            }
            self.using_rc_plugin = true;
            self.connected = true;
            self.plugin_missing_logged = false;
            self.telemetry_read_count = 0;
            self.last_sector_index = -1;
            self.sector_times = [None; 3];
            return Ok(());
        }

        if !self.plugin_missing_logged {
            tracing::info!(target: LOG_TARGET, "RC AC plugin not found — using direct AC shared memory.");
            self.plugin_missing_logged = true;
        }

        // Direct path: probe required pages.
        if !Self::shm_name_exists("Local\\acpmf_physics") {
            anyhow::bail!("Failed to open shared memory: Local\\acpmf_physics");
        }
        if !Self::shm_name_exists("Local\\acpmf_graphics") {
            anyhow::bail!("Failed to open shared memory: Local\\acpmf_graphics");
        }
        if !Self::shm_name_exists("Local\\acpmf_static") {
            anyhow::bail!("Failed to open shared memory: Local\\acpmf_static");
        }

        self.refresh_static_fields();

        // Seed last_lap_count from current completed_laps.
        let initial_laps = Self::open_snapshot("Local\\acpmf_graphics", GRAPHICS_SNAPSHOT_SIZE)
            .map(|b| Self::read_i32_buf(&b, graphics::COMPLETED_LAPS) as u32)
            .unwrap_or(0);

        self.using_rc_plugin = false;
        self.connected = true;
        self.last_lap_count = initial_laps;
        self.last_sector_index = -1;
        self.sector_times = [None; 3];
        self.telemetry_read_count = 0;

        tracing::info!(
            target: LOG_TARGET,
            "AC shared memory connected (ephemeral): driver={}, car={}, track={}, max_rpm={}, initial_laps={}",
            self.current_driver, self.current_car, self.current_track, self.max_rpm, initial_laps
        );
        Ok(())
    }

    #[cfg(not(windows))]
    fn connect(&mut self) -> Result<()> {
        anyhow::bail!("AC shared memory is only available on Windows");
    }

    fn is_connected(&self) -> bool {
        self.connected
    }

    #[cfg(windows)]
    fn read_telemetry(&mut self) -> Result<Option<TelemetryFrame>> {
        // Fast early-exit: if the physics section doesn't exist, the game has
        // exited. verify_shm_alive is a cheap kernel handle probe (no view).
        if !self.verify_shm_alive() {
            tracing::warn!(target: LOG_TARGET, "AC shared memory gone — disconnecting adapter");
            self.disconnect();
            return Ok(None);
        }

        if self.using_rc_plugin {
            return self.read_telemetry_from_plugin();
        }

        // Per-poll ephemeral snapshots — every call re-resolves the section
        // name, so AC session/car changes are picked up automatically.
        let physics_buf = match Self::open_snapshot("Local\\acpmf_physics", PHYSICS_SNAPSHOT_SIZE) {
            Some(b) => b,
            None => {
                tracing::warn!(target: LOG_TARGET, "AC physics snapshot failed — disconnecting adapter");
                self.disconnect();
                return Ok(None);
            }
        };
        let graphics_buf = match Self::open_snapshot("Local\\acpmf_graphics", GRAPHICS_SNAPSHOT_SIZE) {
            Some(b) => b,
            None => {
                tracing::warn!(target: LOG_TARGET, "AC graphics snapshot failed — disconnecting adapter");
                self.disconnect();
                return Ok(None);
            }
        };

        let speed_kmh = Self::read_f32_buf(&physics_buf, physics::SPEED_KMH);
        let throttle = Self::read_f32_buf(&physics_buf, physics::GAS);
        let brake = Self::read_f32_buf(&physics_buf, physics::BRAKE);
        let steering = Self::read_f32_buf(&physics_buf, physics::STEER_ANGLE);
        let rpm = Self::read_i32_buf(&physics_buf, physics::RPMS) as u32;
        let raw_gear = Self::read_i32_buf(&physics_buf, physics::GEAR);
        let gear = (raw_gear - 1) as i8;

        let completed_laps = Self::read_i32_buf(&graphics_buf, graphics::COMPLETED_LAPS) as u32;
        let lap_time_ms = Self::read_i32_buf(&graphics_buf, graphics::I_CURRENT_TIME) as u32;
        let last_lap_time_ms = Self::read_i32_buf(&graphics_buf, graphics::I_LAST_TIME);
        let best_lap_ms = Self::read_i32_buf(&graphics_buf, graphics::I_BEST_TIME);
        let current_sector = Self::read_i32_buf(&graphics_buf, graphics::CURRENT_SECTOR_INDEX);
        let last_sector_time = Self::read_i32_buf(&graphics_buf, graphics::LAST_SECTOR_TIME);
        let is_valid = Self::read_i32_buf(&graphics_buf, graphics::IS_VALID_LAP);
        let normalized_car_position = Self::read_f32_buf(&graphics_buf, graphics::NORMALIZED_CAR_POSITION);
        let is_in_pit_direct = Self::read_i32_buf(&graphics_buf, graphics::IS_IN_PIT) != 0;

        // Refresh static fields every ~5s — picks up mid-session car/track
        // swaps without the cost of re-reading static every poll.
        self.telemetry_read_count += 1;
        if self.telemetry_read_count % 50 == 1 {
            self.refresh_static_fields();
        }

        if current_sector != self.last_sector_index && last_sector_time > 0 {
            let completed_sector = self.last_sector_index;
            if (0..3).contains(&completed_sector) {
                self.sector_times[completed_sector as usize] = Some(last_sector_time as u32);
            }
            self.last_sector_index = current_sector;
        } else if self.last_sector_index < 0 {
            self.last_sector_index = current_sector;
        }

        if self.telemetry_read_count % 300 == 1 {
            tracing::info!(
                target: LOG_TARGET,
                "direct telemetry: completed_laps={} last_lap_count={} lap_time_ms={} last_lap_ms={} speed={:.1} pos={:.3} sector={}",
                completed_laps, self.last_lap_count, lap_time_ms, last_lap_time_ms, speed_kmh, normalized_car_position, current_sector
            );
        }

        if completed_laps > self.last_lap_count {
            let lap_ms = if last_lap_time_ms > 0 { last_lap_time_ms as u32 } else { 0 };
            if lap_ms > 0 {
                let lap_data = LapData {
                    id: uuid::Uuid::new_v4().to_string(),
                    session_id: String::new(),
                    driver_id: String::new(),
                    pod_id: self.pod_id.clone(),
                    sim_type: SimType::AssettoCorsa,
                    track: self.current_track.clone(),
                    car: self.current_car.clone(),
                    lap_number: completed_laps,
                    lap_time_ms: lap_ms,
                    sector1_ms: self.sector_times[0],
                    sector2_ms: self.sector_times[1],
                    sector3_ms: self.sector_times[2],
                    valid: is_valid != 0,
                    session_type: rc_common::types::SessionType::Practice,
                    created_at: Utc::now(),
                };
                tracing::info!(
                    target: LOG_TARGET,
                    "AC lap completed: lap={} time={}ms sectors=[{:?}, {:?}, {:?}] valid={}",
                    completed_laps, lap_ms,
                    self.sector_times[0], self.sector_times[1], self.sector_times[2],
                    is_valid != 0
                );
                self.pending_lap = Some(lap_data);
            }
            self.sector_times = [None; 3];
        }
        self.last_lap_count = completed_laps;

        Ok(Some(TelemetryFrame {
            pod_id: self.pod_id.clone(),
            timestamp: Utc::now(),
            driver_name: self.current_driver.clone(),
            car: self.current_car.clone(),
            track: self.current_track.clone(),
            lap_number: completed_laps,
            lap_time_ms,
            sector: current_sector as u8,
            speed_kmh,
            throttle,
            brake,
            steering,
            gear,
            rpm,
            position: None,
            session_time_ms: lap_time_ms,
            drs_active: None,
            drs_available: None,
            ers_deploy_mode: None,
            ers_store_percent: None,
            best_lap_ms: if best_lap_ms > 0 { Some(best_lap_ms as u32) } else { None },
            current_lap_invalid: Some(is_valid == 0),
            sector1_ms: self.sector_times[0],
            sector2_ms: self.sector_times[1],
            sector3_ms: self.sector_times[2],
            lap_id: None,
            sim_type: Some(SimType::AssettoCorsa),
            normalized_car_position: if (0.0..=1.0).contains(&normalized_car_position) {
                Some(normalized_car_position)
            } else {
                None
            },
            is_in_pit: Some(is_in_pit_direct),
        }))
    }

    #[cfg(not(windows))]
    fn read_telemetry(&mut self) -> Result<Option<TelemetryFrame>> {
        Ok(None)
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
            sim_type: SimType::AssettoCorsa,
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
        // Ephemeral design: no handles to close. Just clear state so the next
        // connect() call picks up whatever the current AC session looks like.
        self.connected = false;
        self.using_rc_plugin = false;
        self.last_sector_index = -1;
        self.sector_times = [None; 3];
        tracing::info!(target: LOG_TARGET, "Disconnected AC adapter (ephemeral SHM — no handles held)");
    }

    fn max_rpm(&self) -> u32 {
        self.max_rpm
    }

    #[cfg(windows)]
    fn read_ac_status(&self) -> Option<AcStatus> {
        if !self.verify_shm_alive() { return None; }
        let buf = Self::open_snapshot("Local\\acpmf_graphics", GRAPHICS_SNAPSHOT_SIZE)?;
        let raw = Self::read_i32_buf(&buf, graphics::STATUS);
        Some(match raw {
            0 => AcStatus::Off,
            1 => AcStatus::Replay,
            2 => AcStatus::Live,
            3 => AcStatus::Pause,
            _ => AcStatus::Off,
        })
    }

    #[cfg(not(windows))]
    fn read_ac_status(&self) -> Option<AcStatus> {
        None
    }

    #[cfg(windows)]
    fn read_assist_state(&self) -> Option<(u8, u8, bool)> {
        if !self.verify_shm_alive() { return None; }
        let buf = Self::open_snapshot("Local\\acpmf_physics", PHYSICS_SNAPSHOT_SIZE)?;
        let abs_val = Self::read_f32_buf(&buf, physics::ABS);
        let tc_val = Self::read_f32_buf(&buf, physics::TC);
        let auto_shifter = Self::read_i32_buf(&buf, physics::AUTO_SHIFTER_ON);
        let abs = if abs_val > 0.0 { (abs_val as u8).max(1) } else { 0 };
        let tc = if tc_val > 0.0 { (tc_val as u8).max(1) } else { 0 };
        Some((abs, tc, auto_shifter != 0))
    }

    #[cfg(not(windows))]
    fn read_assist_state(&self) -> Option<(u8, u8, bool)> {
        None
    }

    #[cfg(windows)]
    fn read_session_config(&self) -> Option<super::SessionConfig> {
        if self.using_rc_plugin {
            let buf = Self::open_snapshot("rcpmf_telemetry", PLUGIN_SNAPSHOT_SIZE)
                .or_else(|| Self::open_snapshot("Local\\rcpmf_telemetry", PLUGIN_SNAPSHOT_SIZE))?;
            let session_type_raw = Self::read_i32_buf(&buf, 20);
            let track_config = Self::read_wchar_buf(&buf, 168, 33);
            let car_model = Self::read_wchar_buf(&buf, 36, 33);
            let track_name = Self::read_wchar_buf(&buf, 102, 33);
            let session_type = match session_type_raw {
                0 => "practice",
                1 => "qualify",
                2 => "race",
                3 => "hotlap",
                4 => "time_attack",
                5 => "drift",
                _ => "unknown",
            };
            // numCars lives in acpmf_static (not plugin SHM).
            let static_buf = Self::open_snapshot("Local\\acpmf_static", STATIC_SNAPSHOT_SIZE);
            let num_cars = static_buf.as_ref().map(|b| Self::read_i32_buf(b, 64));
            return Some(super::SessionConfig {
                num_cars: num_cars.filter(|&n| n > 0).map(|n| n as u32),
                session_type: Some(session_type.to_string()),
                track_config: if track_config.is_empty() { None } else { Some(track_config) },
                car_model: if car_model.is_empty() { None } else { Some(car_model) },
                track_name: if track_name.is_empty() { None } else { Some(track_name) },
                ai_level: None,
            });
        }

        if !self.verify_shm_alive() { return None; }
        let static_buf = Self::open_snapshot("Local\\acpmf_static", STATIC_SNAPSHOT_SIZE)?;
        let graphics_buf = Self::open_snapshot("Local\\acpmf_graphics", GRAPHICS_SNAPSHOT_SIZE)?;
        let num_cars = Self::read_i32_buf(&static_buf, 64);
        let car_model = Self::read_wchar_buf(&static_buf, statics::CAR_MODEL, 33);
        let track_name = Self::read_wchar_buf(&static_buf, statics::TRACK, 33);
        let session_raw = Self::read_i32_buf(&graphics_buf, 8);
        let session_type = match session_raw {
            0 => "practice",
            1 => "qualify",
            2 => "race",
            3 => "hotlap",
            4 => "time_attack",
            5 => "drift",
            _ => "unknown",
        };
        Some(super::SessionConfig {
            num_cars: if num_cars > 0 { Some(num_cars as u32) } else { None },
            session_type: Some(session_type.to_string()),
            track_config: None,
            car_model: if car_model.is_empty() { None } else { Some(car_model) },
            track_name: if track_name.is_empty() { None } else { Some(track_name) },
            ai_level: None,
        })
    }

    #[cfg(not(windows))]
    fn read_session_config(&self) -> Option<super::SessionConfig> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gear_conversion() {
        assert_eq!((0i32 - 1) as i8, -1); // R
        assert_eq!((1i32 - 1) as i8, 0);  // N
        assert_eq!((2i32 - 1) as i8, 1);  // 1st
        assert_eq!((5i32 - 1) as i8, 4);  // 4th
    }

    #[test]
    fn test_assist_state_offsets() {
        assert_eq!(super::physics::TC, 204);
        assert_eq!(super::physics::ABS, 252);
        assert_eq!(super::physics::AUTO_SHIFTER_ON, 264);
        assert!(super::physics::TC > super::physics::SPEED_KMH);
        assert!(super::physics::ABS > super::physics::TC);
        assert!(super::physics::AUTO_SHIFTER_ON > super::physics::ABS);
    }

    #[test]
    fn test_read_assist_state_no_shm() {
        let adapter = AssettoCorsaAdapter::new("pod_1".to_string(), "127.0.0.1".to_string(), 9600);
        // Without any SHM open, open_snapshot returns None → read_assist_state returns None.
        assert_eq!(adapter.read_assist_state(), None);
    }

    #[test]
    fn test_ac_status_read_no_shm() {
        let adapter = AssettoCorsaAdapter::new("pod_1".to_string(), "127.0.0.1".to_string(), 9600);
        assert_eq!(adapter.read_ac_status(), None);
    }

    #[test]
    fn test_read_f32_buf_bounds() {
        let buf = vec![0u8; 10];
        // Out-of-bounds returns 0.0, not a panic.
        assert_eq!(AssettoCorsaAdapter::read_f32_buf(&buf, 100), 0.0);
        // In-bounds zero bytes read as 0.0.
        assert_eq!(AssettoCorsaAdapter::read_f32_buf(&buf, 0), 0.0);
    }

    #[test]
    fn test_read_i32_buf_little_endian() {
        // 0x0000_0001 little-endian = [01, 00, 00, 00]
        let buf = vec![0x01, 0x00, 0x00, 0x00];
        assert_eq!(AssettoCorsaAdapter::read_i32_buf(&buf, 0), 1);
    }

    #[test]
    fn test_read_wchar_buf_utf16_nul_terminated() {
        // UTF-16LE "abc\0" = [61, 00, 62, 00, 63, 00, 00, 00]
        let buf = vec![0x61, 0x00, 0x62, 0x00, 0x63, 0x00, 0x00, 0x00, 0xff, 0xff];
        assert_eq!(AssettoCorsaAdapter::read_wchar_buf(&buf, 0, 5), "abc");
    }

    /// ADAPTER-SHM-01 regression: the adapter MUST NOT hold persistent SHM
    /// handles across poll boundaries. Cached MapViewOfFile pointers point
    /// to stale kernel sections when AC swaps car/session, causing HUD to
    /// freeze and laps to never fire. The needle strings are constructed at
    /// runtime so this test's own source doesn't self-match.
    #[test]
    fn test_no_cached_shm_handles_regression() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src").join("sims").join("assetto_corsa.rs");
        let content = std::fs::read_to_string(&path).expect("read assetto_corsa.rs");
        // Build needles at runtime — avoids the test source matching itself.
        let forbidden_prefixes = ["physics", "graphics", "static", "rc_plugin"];
        let suffix = "_handle: ".to_string() + "Option<ShmHandle>";
        for prefix in &forbidden_prefixes {
            let needle = format!("{}{}", prefix, suffix);
            assert!(
                !content.contains(&needle),
                "REGRESSION: AC adapter reintroduced a cached SHM handle field \
                 (prefix `{}`). Use per-poll Self::open_snapshot() instead. See \
                 ADAPTER-SHM-01 (2026-04-23).",
                prefix
            );
        }
        // And the ephemeral helper must exist.
        let helper_marker = "fn open_snapshot(".to_string() + "name: &str, size: usize)";
        assert!(
            content.contains(&helper_marker),
            "AC adapter must expose open_snapshot() — the only permitted SHM ingress."
        );
    }
}
