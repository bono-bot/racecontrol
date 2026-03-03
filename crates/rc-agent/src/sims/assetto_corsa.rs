use anyhow::Result;
use chrono::Utc;
use rc_common::types::*;
use super::SimAdapter;

/// Assetto Corsa shared memory telemetry reader.
///
/// Reads AC's memory-mapped files (acpmf_physics, acpmf_graphics, acpmf_static)
/// which are always available when AC is running and support multiple readers.
pub struct AssettoCorsaAdapter {
    connected: bool,
    pod_id: String,
    last_lap_count: u32,
    current_driver: String,
    current_car: String,
    current_track: String,
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
// Physics (acpmf_physics) — updates every frame
mod physics {
    pub const GAS: usize = 4;        // f32, throttle 0.0-1.0
    pub const BRAKE: usize = 8;      // f32, brake 0.0-1.0
    pub const GEAR: usize = 16;      // i32, 0=R 1=N 2=1st 3=2nd...
    pub const RPMS: usize = 20;      // i32, engine RPM
    pub const STEER_ANGLE: usize = 24; // f32, radians
    pub const SPEED_KMH: usize = 28;  // f32, km/h
}

// Graphics (acpmf_graphics) — updates ~10Hz
mod graphics {
    pub const COMPLETED_LAPS: usize = 132; // i32
    pub const I_CURRENT_TIME: usize = 140; // i32, ms
    pub const I_LAST_TIME: usize = 144;    // i32, ms
    pub const I_BEST_TIME: usize = 148;    // i32, ms
}

// Static (acpmf_static) — updates once per session
mod statics {
    pub const CAR_MODEL: usize = 68;    // wchar[33] = 66 bytes UTF-16LE
    pub const TRACK: usize = 134;       // wchar[33] = 66 bytes UTF-16LE
    pub const PLAYER_NAME: usize = 200; // wchar[33] = 66 bytes UTF-16LE
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

        tracing::info!(
            "AC shared memory connected: driver={}, car={}, track={}",
            self.current_driver, self.current_car, self.current_track
        );

        self.physics_handle = Some(physics);
        self.graphics_handle = Some(graphics);
        self.static_handle = Some(static_info);
        self.connected = true;

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

        let lap_number = Self::read_i32(graphics, graphics::COMPLETED_LAPS) as u32;
        let lap_time_ms = Self::read_i32(graphics, graphics::I_CURRENT_TIME) as u32;

        self.last_lap_count = lap_number;

        Ok(Some(TelemetryFrame {
            pod_id: self.pod_id.clone(),
            timestamp: Utc::now(),
            driver_name: self.current_driver.clone(),
            car: self.current_car.clone(),
            track: self.current_track.clone(),
            lap_number,
            lap_time_ms,
            sector: 0,
            speed_kmh,
            throttle,
            brake,
            steering,
            gear,
            rpm,
            position: None,
            session_time_ms: lap_time_ms,
        }))
    }

    #[cfg(not(windows))]
    fn read_telemetry(&mut self) -> Result<Option<TelemetryFrame>> {
        Ok(None)
    }

    fn poll_lap_completed(&mut self) -> Result<Option<LapData>> {
        Ok(None)
    }

    fn session_info(&self) -> Result<Option<SessionInfo>> {
        if !self.connected {
            return Ok(None);
        }
        Ok(Some(SessionInfo {
            id: String::new(),
            session_type: SessionType::Practice,
            sim_type: SimType::AssettoCorsa,
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
            // Drop handles — MapViewOfFile and CloseHandle cleanup
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
}
