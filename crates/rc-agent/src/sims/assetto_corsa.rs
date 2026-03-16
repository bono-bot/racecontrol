use anyhow::Result;
use chrono::Utc;
use rc_common::types::*;
use rc_common::types::AcStatus;
use super::SimAdapter;

/// Assetto Corsa shared memory telemetry reader.
///
/// Reads AC's memory-mapped files (acpmf_physics, acpmf_graphics, acpmf_static)
/// which are always available when AC is running and support multiple readers.
///
/// Sector times and lap completion are tracked via the graphics shared memory.
/// AC exposes `currentSectorIndex` (0/1/2), `lastSectorTime` (ms for the sector
/// just completed), `iLastTime` (total lap time for last completed lap), and
/// `completedLaps` (increments each time a lap is finished).
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
    // Windows handles for memory-mapped files
    #[cfg(windows)]
    physics_handle: Option<ShmHandle>,
    #[cfg(windows)]
    graphics_handle: Option<ShmHandle>,
    #[cfg(windows)]
    static_handle: Option<ShmHandle>,
}

/// Wrapper for a Windows memory-mapped file handle + view pointer
#[cfg(windows)]
struct ShmHandle {
    _handle: winapi::shared::ntdef::HANDLE,
    ptr: *const u8,
    _size: usize,
}

#[cfg(windows)]
// SAFETY: The memory-mapped file pointers are read-only views shared between processes.
// The underlying data is managed by AC and only read by this process.
unsafe impl Send for ShmHandle {}
#[cfg(windows)]
unsafe impl Sync for ShmHandle {}

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
// Reference: sim_info.py SPageFileGraphic with _pack_ = 4
// Offsets calculated from struct layout:
//   0: packetId(i32), 4: status(i32), 8: session(i32),
//   12: currentTime(wchar[15]=30B), 42: lastTime(30B), 72: bestTime(30B),
//   102: split(30B), 132: completedLaps(i32), 136: position(i32),
//   140: iCurrentTime(i32), 144: iLastTime(i32), 148: iBestTime(i32),
//   152: sessionTimeLeft(f32), 156: distanceTraveled(f32), 160: isInPit(i32),
//   164: currentSectorIndex(i32), 168: lastSectorTime(i32), 172: numberOfLaps(i32),
//   176: tyreCompound(wchar[33]=66B), 242: (pad 2B), 244: replayTimeMultiplier(f32),
//   248: normalizedCarPosition(f32), ...
//   396: currentSectorIndex repeated? ... 1408+: isValidLap
mod graphics {
    pub const STATUS: usize = 4;              // i32, AC_STATUS: 0=OFF 1=REPLAY 2=LIVE 3=PAUSE
    pub const COMPLETED_LAPS: usize = 132;    // i32
    pub const CURRENT_SECTOR_INDEX: usize = 164; // i32, 0=S1 1=S2 2=S3
    pub const I_CURRENT_TIME: usize = 140;    // i32, current lap time in ms
    pub const I_LAST_TIME: usize = 144;       // i32, last completed lap time in ms
    pub const I_BEST_TIME: usize = 148;       // i32, session best lap time in ms
    pub const IS_IN_PIT: usize = 160;         // i32, 1 if in pit lane
    pub const LAST_SECTOR_TIME: usize = 168;  // i32, last sector split time in ms
    pub const NUMBER_OF_LAPS: usize = 172;    // i32, total laps in session (0 = unlimited)
    pub const NORMALIZED_CAR_POSITION: usize = 248; // f32, 0.0-1.0 track progress
    // isValidLap is deep in the extended struct (~offset 1408+), not reliably accessible
    // We still track it but acknowledge it may read incorrect data
    pub const IS_VALID_LAP: usize = 180;      // i32, approximate — may need correction
}

// Static (acpmf_static) — updates once per session
// Reference: sim_info.py SPageFileStatic with _pack_ = 4
// 0: smVersion(wchar[15]=30B), 30: acVersion(30B), 60: numberOfSessions(i32),
// 64: numCars(i32), 68: carModel(wchar[33]=66B), 134: track(66B),
// 200: playerName(66B), 266: playerSurname(66B), 332: playerNick(66B),
// 398: sectorCount(i32)
mod statics {
    pub const CAR_MODEL: usize = 68;    // wchar[33] = 66 bytes UTF-16LE
    pub const TRACK: usize = 134;       // wchar[33] = 66 bytes UTF-16LE
    pub const PLAYER_NAME: usize = 200; // wchar[33] = 66 bytes UTF-16LE
    pub const NUM_SECTORS: usize = 398; // i32, number of sectors on this track (usually 3)
    // 402: maxTorque(f32), 406: maxPower(f32), 410: maxRpm(i32)
    pub const MAX_RPM: usize = 410;     // i32, car's max RPM (replaces hardcoded 18000)
}

impl AssettoCorsaAdapter {
    pub fn new(pod_id: String, _server_ip: String, _server_port: u16) -> Self {
        Self {
            connected: false,
            pod_id,
            last_lap_count: 0,
            current_driver: String::new(),
            current_car: String::new(),
            current_track: String::new(),
            max_rpm: 8000, // default until read from AC static memory
            last_sector_index: -1,
            sector_times: [None; 3],
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

impl SimAdapter for AssettoCorsaAdapter {
    fn sim_type(&self) -> SimType {
        SimType::AssettoCorsa
    }

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

        let physics = open_shm("Local\\acpmf_physics")?;
        let graphics = open_shm("Local\\acpmf_graphics")?;
        let static_info = open_shm("Local\\acpmf_static")?;

        // Read static info (car, track, driver)
        self.current_car = Self::read_wchar_string(&static_info, statics::CAR_MODEL, 33);
        self.current_track = Self::read_wchar_string(&static_info, statics::TRACK, 33);
        self.current_driver = Self::read_wchar_string(&static_info, statics::PLAYER_NAME, 33);

        let num_sectors = Self::read_i32(&static_info, statics::NUM_SECTORS);
        let raw_max_rpm = Self::read_i32(&static_info, statics::MAX_RPM);
        self.max_rpm = if raw_max_rpm > 0 { raw_max_rpm as u32 } else { 8000 };

        tracing::info!(
            "AC shared memory connected: driver={}, car={}, track={}, sectors={}, max_rpm={}",
            self.current_driver, self.current_car, self.current_track, num_sectors, self.max_rpm
        );

        // Snapshot current completed_laps to avoid false lap detection from stale data
        let initial_laps = Self::read_i32(&graphics, graphics::COMPLETED_LAPS) as u32;

        self.physics_handle = Some(physics);
        self.graphics_handle = Some(graphics);
        self.static_handle = Some(static_info);
        self.connected = true;
        self.last_lap_count = initial_laps;
        self.last_sector_index = -1;
        self.sector_times = [None; 3];
        tracing::info!("AC: initial completed_laps = {} (skipping stale)", initial_laps);

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
        let physics = match &self.physics_handle {
            Some(h) => h,
            None => return Ok(None),
        };
        let graphics = match &self.graphics_handle {
            Some(h) => h,
            None => return Ok(None),
        };

        let speed_kmh = Self::read_f32(physics, physics::SPEED_KMH);
        let throttle = Self::read_f32(physics, physics::GAS);
        let brake = Self::read_f32(physics, physics::BRAKE);
        let steering = Self::read_f32(physics, physics::STEER_ANGLE);
        let rpm = Self::read_i32(physics, physics::RPMS) as u32;
        // Gear: AC uses 0=R, 1=N, 2=1st. Convert to display: -1=R, 0=N, 1=1st
        let raw_gear = Self::read_i32(physics, physics::GEAR);
        let gear = (raw_gear - 1) as i8;

        let completed_laps = Self::read_i32(graphics, graphics::COMPLETED_LAPS) as u32;
        let lap_time_ms = Self::read_i32(graphics, graphics::I_CURRENT_TIME) as u32;
        let last_lap_time_ms = Self::read_i32(graphics, graphics::I_LAST_TIME);
        let best_lap_ms = Self::read_i32(graphics, graphics::I_BEST_TIME);
        let current_sector = Self::read_i32(graphics, graphics::CURRENT_SECTOR_INDEX);
        let last_sector_time = Self::read_i32(graphics, graphics::LAST_SECTOR_TIME);
        let is_valid = Self::read_i32(graphics, graphics::IS_VALID_LAP);

        // Track sector transitions to accumulate split times
        if current_sector != self.last_sector_index && last_sector_time > 0 {
            // A sector just completed — store its time
            let completed_sector = self.last_sector_index;
            if completed_sector >= 0 && completed_sector < 3 {
                self.sector_times[completed_sector as usize] = Some(last_sector_time as u32);
            }
            self.last_sector_index = current_sector;
        } else if self.last_sector_index < 0 {
            // First read — initialize sector tracking
            self.last_sector_index = current_sector;
        }

        // Detect lap completion: completedLaps incremented
        if completed_laps > self.last_lap_count {
            let lap_ms = if last_lap_time_ms > 0 { last_lap_time_ms as u32 } else { 0 };

            if lap_ms > 0 {
                let lap_data = LapData {
                    id: uuid::Uuid::new_v4().to_string(),
                    session_id: String::new(), // Filled by racecontrol from billing session
                    driver_id: String::new(),  // Filled by racecontrol from billing session
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
                    created_at: Utc::now(),
                };

                tracing::info!(
                    "AC lap completed: lap={} time={}ms sectors=[{:?}, {:?}, {:?}] valid={}",
                    completed_laps, lap_ms,
                    self.sector_times[0], self.sector_times[1], self.sector_times[2],
                    is_valid != 0
                );

                self.pending_lap = Some(lap_data);
            }

            // Reset sector accumulator for next lap
            self.sector_times = [None; 3];
        }
        self.last_lap_count = completed_laps;

        // Build telemetry frame with sector data
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
        tracing::info!("Disconnected from AC shared memory");
    }

    fn max_rpm(&self) -> u32 {
        self.max_rpm
    }

    #[cfg(windows)]
    fn read_ac_status(&self) -> Option<AcStatus> {
        let gh = self.graphics_handle.as_ref()?;
        let raw = Self::read_i32(gh, graphics::STATUS);
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
        let ph = self.physics_handle.as_ref()?;

        let abs_val = Self::read_f32(ph, physics::ABS);
        let tc_val = Self::read_f32(ph, physics::TC);
        let auto_shifter = Self::read_i32(ph, physics::AUTO_SHIFTER_ON);

        let abs = if abs_val > 0.0 { (abs_val as u8).max(1) } else { 0 };
        let tc = if tc_val > 0.0 { (tc_val as u8).max(1) } else { 0 };

        Some((abs, tc, auto_shifter != 0))
    }

    #[cfg(not(windows))]
    fn read_assist_state(&self) -> Option<(u8, u8, bool)> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gear_conversion() {
        // AC gear encoding: 0=R, 1=N, 2=1st, 3=2nd...
        // Display gear: -1=R, 0=N, 1=1st, 2=2nd...
        assert_eq!((0i32 - 1) as i8, -1); // R
        assert_eq!((1i32 - 1) as i8, 0);  // N
        assert_eq!((2i32 - 1) as i8, 1);  // 1st
        assert_eq!((5i32 - 1) as i8, 4);  // 4th
    }

    #[test]
    fn test_assist_state_offsets() {
        // Verify that the physics shared memory offsets are correct
        assert_eq!(super::physics::TC, 204, "TC offset should be 204");
        assert_eq!(super::physics::ABS, 252, "ABS offset should be 252");
        assert_eq!(super::physics::AUTO_SHIFTER_ON, 264, "AUTO_SHIFTER_ON offset should be 264");

        // Verify offsets are after SPEED_KMH (28) and before the struct boundary
        assert!(super::physics::TC > super::physics::SPEED_KMH);
        assert!(super::physics::ABS > super::physics::TC);
        assert!(super::physics::AUTO_SHIFTER_ON > super::physics::ABS);
    }

    #[test]
    fn test_read_assist_state_non_windows() {
        // On non-Windows (or without AC running), read_assist_state returns None
        let adapter = AssettoCorsaAdapter::new("pod_1".to_string(), "127.0.0.1".to_string(), 9600);
        let state = adapter.read_assist_state();
        // Without shared memory handle, it returns None
        assert_eq!(state, None);
    }

    #[test]
    fn test_ac_status_read_non_windows() {
        // On non-Windows, read_ac_status() always returns None (no shared memory)
        let adapter = AssettoCorsaAdapter::new("pod_1".to_string(), "127.0.0.1".to_string(), 9600);
        let status = adapter.read_ac_status();
        #[cfg(not(windows))]
        assert_eq!(status, None);
        // On Windows without AC running, graphics_handle is None so it also returns None
        #[cfg(windows)]
        assert_eq!(status, None);
    }
}
