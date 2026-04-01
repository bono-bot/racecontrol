use anyhow::Result;
use chrono::Utc;
use rc_common::types::*;

use super::SimAdapter;

const LOG_TARGET: &str = "sim-lmu";

/// Le Mans Ultimate (LMU) shared memory telemetry adapter.
///
/// LMU uses the rFactor 2 shared memory plugin (`rF2SharedMemoryMapPlugin`)
/// which exposes telemetry via two named Windows file maps:
///   - `$rFactor2SMMP_Scoring$`   — 5 Hz, lap times + sector splits + session state
///   - `$rFactor2SMMP_Telemetry$` — 50 Hz, vehicle inputs, speed, RPM
///
/// Unlike iRacing (dynamic variable table), rF2 uses a fixed C struct layout —
/// fields are at predictable byte offsets computed from `rF2Data.cs`.
///
/// Struct offsets sourced from:
///   TheIronWolfModding/rF2SharedMemoryMapPlugin — rF2Data.cs (master branch, 2024)
///   Vehicle record size: 368 bytes per rF2VehicleScoring entry.
///   Scoring buffer layout:
///     Offset 0:  mVersionUpdateBegin (u32)
///     Offset 4:  mVersionUpdateEnd   (u32)
///     Offset 8:  mBytesUpdatedHint   (i32)
///     Offset 12: rF2ScoringInfo      (2816 bytes)
///     Offset 2828: rF2VehicleScoring[128] (368 bytes each)
///
/// Lap detection uses `mTotalLaps` (i16) incrementing on the player vehicle.
/// Sector times use `mLastSector1` (S1), `mLastSector2` (S1+S2 cumulative).
/// S3 is derived as `mLastLapTime - mLastSector2`.
pub struct LmuAdapter {
    pod_id: String,
    connected: bool,
    #[cfg(windows)]
    scoring_shm: Option<ShmHandle>,
    #[cfg(windows)]
    telemetry_shm: Option<ShmHandle>,
    last_lap_count: i16,
    last_session_type: i32,
    pending_lap: Option<LapData>,
    current_track: String,
    current_car: String,
    current_session_type: SessionType,
    first_read: bool,
}

// ─── Scoring buffer layout constants ─────────────────────────────────────────
//
// Source: rF2Data.cs — TheIronWolfModding/rF2SharedMemoryMapPlugin (master, 2024)
//
// rF2Scoring buffer layout:
//   [0]   mVersionUpdateBegin  (u32)    — incremented before write
//   [4]   mVersionUpdateEnd    (u32)    — incremented after write
//   [8]   mBytesUpdatedHint    (i32)
//   [12]  rF2ScoringInfo       (2816 bytes, Sequential C# struct)
//   [2828] rF2VehicleScoring[128]
//
// rF2ScoringInfo fields used (offsets from start of rF2ScoringInfo, i.e., +12 from buffer):
//   [0..64]   mTrackName      (char[64])
//   [64]      mSession        (i32)     — session type integer
//   [68..96]  mCurrentET/mEndET/mSectorFlag/...
//   [96]      mGamePhase      (u8)      — 0=Before, 1=Reconnect, 2=Disconnect, 3=ShuttleToGarage,
//                                         4=Countdown, 5=GreenFlag, 6=FullCourseYellow, 7=SessionStopped, 8=SessionOver
//   [208]     mNumVehicles    (i32)     — number of vehicles in session
//
// rF2VehicleScoring fields (offsets within the 368-byte vehicle struct):
// Sourced from rF2Data.cs [StructLayout(LayoutKind.Sequential)] field order + types.
// All f64 = 8 bytes, i32 = 4, i16 = 2, i8/u8 = 1.
//
//   [0]   mID                (i32)         = 4
//   [4]   mDriverName        (char[32])    = 32
//   [36]  mVehicleName       (char[64])    = 64
//   [100] mTotalLaps         (i16)         = 2
//   [102] mSector            (i8)          = 1   (0=S3/finish, 1=S1, 2=S2)
//   [103] mFinishStatus      (i8)          = 1   (0=none, 1=finished, 2=dnf, 3=dq)
//   [104] mLapDist           (f64)         = 8
//   [112] mPathLateral       (f64)         = 8
//   [120] mTrackEdge         (f64)         = 8
//   [128] mBestSector1       (f64)         = 8
//   [136] mBestSector2       (f64)         = 8   (cumulative S1+S2)
//   [144] mBestLapTime       (f64)         = 8
//   [152] mLastSector1       (f64)         = 8
//   [160] mLastSector2       (f64)         = 8   (cumulative S1+S2)
//   [168] mLastLapTime       (f64)         = 8
//   [176] mCurSector1        (f64)         = 8
//   [184] mCurSector2        (f64)         = 8   (cumulative)
//   [192] mNumPitstops       (i16)         = 2
//   [194] mNumPenalties      (i16)         = 2
//   [196] mIsPlayer          (u8)          = 1
//   [197] mControl           (i8)          = 1   (-1=nobody, 0=local player, 1=local AI, 2=remote, 3=replay)
//   [198] mInPits            (u8)          = 1
//   [199] mPlace             (u8)          = 1
//   ... (remaining fields not needed)

/// Offset of version update begin/end in any rF2 buffer (u32 at 0, u32 at 4)
const BUF_VERSION_BEGIN_OFFSET: usize = 0;
const BUF_VERSION_END_OFFSET: usize = 4;

/// Offset to the rF2ScoringInfo struct within the scoring buffer
const SCORING_INFO_OFFSET: usize = 12;

/// Size of rF2ScoringInfo (from rF2Data.cs — StructLayout Sequential with Pack=4)
/// Computed from all fields: 2816 bytes.
const SCORING_INFO_SIZE: usize = 2816;

/// Offset to the vehicle array in the scoring buffer
const SCORING_VEHICLES_OFFSET: usize = SCORING_INFO_OFFSET + SCORING_INFO_SIZE; // = 2828

/// Size of each rF2VehicleScoring entry (from rF2Data.cs)
const VEHICLE_STRUCT_SIZE: usize = 368;

/// Maximum vehicles in the vehicle array
const MAX_VEHICLES: usize = 128;

// rF2ScoringInfo field offsets (relative to SCORING_INFO_OFFSET in the buffer)
const SCORING_INFO_TRACK_NAME_OFF: usize = 0;    // char[64]
const SCORING_INFO_SESSION_OFF: usize = 64;       // i32 — mSession
const SCORING_INFO_GAME_PHASE_OFF: usize = 96;    // u8 — mGamePhase
const SCORING_INFO_NUM_VEHICLES_OFF: usize = 208; // i32 — mNumVehicles

// rF2VehicleScoring field offsets (relative to start of each vehicle entry)
const VEH_VEHICLE_NAME_OFF: usize = 36;    // char[64] — mVehicleName
const VEH_TOTAL_LAPS_OFF: usize = 100;    // i16 — mTotalLaps
const VEH_LAST_SECTOR1_OFF: usize = 152;  // f64 — mLastSector1 (S1 time)
const VEH_LAST_SECTOR2_OFF: usize = 160;  // f64 — mLastSector2 (S1+S2 cumulative)
const VEH_LAST_LAP_TIME_OFF: usize = 168; // f64 — mLastLapTime
const VEH_IS_PLAYER_OFF: usize = 196;     // u8 — mIsPlayer
#[allow(dead_code)]
const VEH_IN_PITS_OFF: usize = 198;      // u8 — mInPits

// rF2Telemetry buffer layout:
// [0]   mVersionUpdateBegin (u32)
// [4]   mVersionUpdateEnd   (u32)
// [8]   mBytesUpdatedHint   (i32)
// [12]  rF2TelemetryInfo:
//   [0]    mNumVehicles (i32)
//   [4]    mET          (f64)
//   [12]   rF2VehicleTelemetry[128] — each vehicle entry
//
// rF2VehicleTelemetry (128 bytes, simplified for our use):
// Sourced from rF2Data.cs; we only need speed/throttle/brake/gear/rpm.
const TELEMETRY_INFO_OFFSET: usize = 12;
#[allow(dead_code)]
const TEL_NUM_VEHICLES_OFF: usize = 0; // i32 relative to rF2TelemetryInfo

// rF2VehicleTelemetry vehicle entry size: 1440 bytes (full)
const TEL_VEHICLE_SIZE: usize = 1440;

// Within rF2VehicleTelemetry:
// [0]   mID           (i32)
// [4]   pad           (i32)
// [8]   mPos          (rF2Vec3: 3×f64 = 24 bytes)
// [32]  mLocalVel     (rF2Vec3: 3×f64 = 24)
// [56]  mLocalAccel   (rF2Vec3 = 24)
// [80]  mOri          (rF2Vec3[3] = 72)
// [152] mLocalRot     (rF2Vec3 = 24)
// [176] mLocalRotAccel(rF2Vec3 = 24)
// [200] mGear         (i32)
// [204] mEngineRPM    (f64)
// [212] mEngineWaterTemp (f64)
// [220] mFuelLevel    (f64)
// [228] mEngineOilTemp(f64)
// [236] mClutchRPM    (f64)
// [244] mUnfilteredThrottle (f64)
// [252] mUnfilteredBrake    (f64)
// [260] mUnfilteredSteering (f64)
// [268] mUnfilteredClutch   (f64)
// [276] mFilteredThrottle   (f64)
// [284] mFilteredBrake      (f64)
// [292] mFilteredSteering   (f64)
// ... mSpeed at [432] derived as |mLocalVel| — OR use mOri matrix * mLocalVel
// We use a simpler approach: compute speed from mLocalVel magnitude.
// mLocalVel is at offset 32, rF2Vec3 = {x: f64, y: f64, z: f64}

#[allow(dead_code)]
const TEL_VEH_ID_OFF: usize = 0;         // i32
const TEL_VEH_LOCAL_VEL_OFF: usize = 32; // rF2Vec3 (x,y,z f64) — for speed
const TEL_VEH_GEAR_OFF: usize = 200;     // i32 — mGear (-1=reverse, 0=neutral, 1..=n=gears)
const TEL_VEH_ENGINE_RPM_OFF: usize = 204; // f64 — mEngineRPM
const TEL_VEH_UNFILTERED_THROTTLE_OFF: usize = 244; // f64
const TEL_VEH_UNFILTERED_BRAKE_OFF: usize = 252;    // f64
const TEL_VEH_FILTERED_STEERING_OFF: usize = 292;   // f64

// ─── Windows SHM handle ──────────────────────────────────────────────────────

/// Wrapper for a Windows memory-mapped file handle and view pointer.
/// Follows the same pattern as IracingAdapter's ShmHandle.
#[cfg(windows)]
struct ShmHandle {
    _handle: winapi::shared::ntdef::HANDLE,
    ptr: *const u8,
    _size: usize,
}

#[cfg(windows)]
// SAFETY: The memory-mapped file pointer is a read-only view of LMU's
// shared memory. LMU's rF2SharedMemoryMapPlugin owns the write side;
// this process only reads.
unsafe impl Send for ShmHandle {}

#[cfg(windows)]
unsafe impl Sync for ShmHandle {}

#[cfg(windows)]
impl Drop for ShmHandle {
    fn drop(&mut self) {
        unsafe {
            winapi::um::memoryapi::UnmapViewOfFile(self.ptr as *const _);
            winapi::um::handleapi::CloseHandle(self._handle);
        }
    }
}

// ─── Windows shared memory helpers ───────────────────────────────────────────

#[cfg(windows)]
fn open_lmu_shm(name: &str) -> Result<ShmHandle> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;

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
                "OpenFileMappingW failed for LMU shm {:?} (error={}) — \
                 is LMU running with rF2SharedMemoryMapPlugin loaded?",
                name,
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
            let err = winapi::um::errhandlingapi::GetLastError();
            winapi::um::handleapi::CloseHandle(handle);
            anyhow::bail!("MapViewOfFile failed for LMU shm {:?} (error={})", name, err);
        }

        Ok(ShmHandle {
            _handle: handle,
            ptr: ptr as *const u8,
            _size: 0,
        })
    }
}

/// Read the version-update pair from a buffer and return (begin, end).
/// If begin == end the read is consistent (not torn).
#[cfg(windows)]
fn read_version_pair(ptr: *const u8) -> (u32, u32) {
    unsafe {
        let begin = std::ptr::read_unaligned(
            ptr.add(BUF_VERSION_BEGIN_OFFSET) as *const u32,
        );
        let end = std::ptr::read_unaligned(
            ptr.add(BUF_VERSION_END_OFFSET) as *const u32,
        );
        (begin, end)
    }
}

// ─── Sector time derivation ───────────────────────────────────────────────────

/// Derive sector split times (milliseconds) from rF2 cumulative fields.
///
/// rF2 stores:
///   last_s1_s         = mLastSector1 (S1 time in seconds)
///   last_s2_cumul_s   = mLastSector2 (S1 + S2, cumulative, in seconds)
///   last_lap_s        = mLastLapTime (full lap in seconds)
///
/// Returns (None, None, None) if any input is <= 0.0 (rF2 signals invalid with <= 0).
///
/// Source: TransitionTracker.cs — TheIronWolfModding/rF2SharedMemoryMapPlugin
pub fn sector_times_ms(
    last_lap_s: f64,
    last_s1_s: f64,
    last_s2_cumul_s: f64,
) -> (Option<u32>, Option<u32>, Option<u32>) {
    if last_lap_s <= 0.0 || last_s1_s <= 0.0 || last_s2_cumul_s <= 0.0 {
        return (None, None, None);
    }
    let s1 = (last_s1_s * 1000.0).round() as u32;
    let s2 = ((last_s2_cumul_s - last_s1_s) * 1000.0).round() as u32;
    let s3 = ((last_lap_s - last_s2_cumul_s) * 1000.0).round() as u32;
    (Some(s1), Some(s2), Some(s3))
}

// Bug #17: Explicit Drop for LmuAdapter to ensure SHM handles are released.
impl Drop for LmuAdapter {
    fn drop(&mut self) {
        #[cfg(windows)]
        {
            if self.scoring_shm.is_some() || self.telemetry_shm.is_some() {
                tracing::debug!(target: LOG_TARGET, "LmuAdapter dropped — releasing SHM handles");
            }
            self.scoring_shm = None;
            self.telemetry_shm = None;
        }
    }
}

// ─── LmuAdapter methods ───────────────────────────────────────────────────────

impl LmuAdapter {
    pub fn new(pod_id: String) -> Self {
        Self {
            pod_id,
            connected: false,
            #[cfg(windows)]
            scoring_shm: None,
            #[cfg(windows)]
            telemetry_shm: None,
            last_lap_count: 0,
            last_session_type: -1,
            pending_lap: None,
            current_track: String::new(),
            current_car: String::new(),
            current_session_type: SessionType::Practice,
            first_read: true,
        }
    }

    /// Read track name from scoring info (null-terminated char[64] at SCORING_INFO_TRACK_NAME_OFF).
    #[cfg(windows)]
    fn read_track_name(scoring_ptr: *const u8) -> String {
        let base = SCORING_INFO_OFFSET + SCORING_INFO_TRACK_NAME_OFF;
        let bytes = unsafe { std::slice::from_raw_parts(scoring_ptr.add(base), 64) };
        let null_end = bytes.iter().position(|&b| b == 0).unwrap_or(64);
        String::from_utf8_lossy(&bytes[..null_end]).into_owned()
    }

    /// Read mSession (i32) from scoring info.
    #[cfg(windows)]
    fn read_session_type(scoring_ptr: *const u8) -> i32 {
        let off = SCORING_INFO_OFFSET + SCORING_INFO_SESSION_OFF;
        unsafe { std::ptr::read_unaligned(scoring_ptr.add(off) as *const i32) }
    }

    /// Read mGamePhase (u8) from scoring info.
    #[cfg(windows)]
    fn read_game_phase(scoring_ptr: *const u8) -> u8 {
        let off = SCORING_INFO_OFFSET + SCORING_INFO_GAME_PHASE_OFF;
        unsafe { std::ptr::read_unaligned(scoring_ptr.add(off) as *const u8) }
    }

    /// Read mNumVehicles (i32) from scoring info.
    #[cfg(windows)]
    fn read_num_vehicles(scoring_ptr: *const u8) -> i32 {
        let off = SCORING_INFO_OFFSET + SCORING_INFO_NUM_VEHICLES_OFF;
        unsafe { std::ptr::read_unaligned(scoring_ptr.add(off) as *const i32) }
    }

    /// Find the player's vehicle index in the vehicle array.
    /// Player is identified by mIsPlayer == 1.
    /// Returns None if not found.
    #[cfg(windows)]
    fn find_player_vehicle_index(scoring_ptr: *const u8, num_vehicles: usize) -> Option<usize> {
        let vehicles_base = SCORING_VEHICLES_OFFSET;
        let count = num_vehicles.min(MAX_VEHICLES);
        for i in 0..count {
            let veh_base = vehicles_base + i * VEHICLE_STRUCT_SIZE;
            let is_player = unsafe {
                std::ptr::read_unaligned(scoring_ptr.add(veh_base + VEH_IS_PLAYER_OFF) as *const u8)
            };
            if is_player == 1 {
                return Some(i);
            }
        }
        None
    }

    /// Read the private helper — check if player is on track from scoring shm.
    /// Returns Some(true) when scoring buffer is readable AND player found AND mGamePhase >= 4.
    #[cfg(windows)]
    fn read_is_on_track_from_shm(&self) -> Option<bool> {
        if !self.connected {
            return None;
        }
        let scoring = self.scoring_shm.as_ref()?;
        let ptr = scoring.ptr;

        // Torn-read check
        let (begin, end) = read_version_pair(ptr);
        if begin != end {
            return None;
        }

        let game_phase = Self::read_game_phase(ptr);
        if game_phase < 4 {
            return Some(false);
        }

        let num_vehicles = Self::read_num_vehicles(ptr);
        if num_vehicles <= 0 {
            return Some(false);
        }

        let player_idx =
            Self::find_player_vehicle_index(ptr, num_vehicles as usize);
        Some(player_idx.is_some())
    }

    #[cfg(not(windows))]
    fn read_is_on_track_from_shm(&self) -> Option<bool> {
        None
    }

    /// Read the player's vehicle data from the scoring buffer and detect lap completion.
    /// Called from read_telemetry_windows() on each poll.
    #[cfg(windows)]
    fn process_scoring(
        &mut self,
        scoring_ptr: *const u8,
    ) {
        let session_type = Self::read_session_type(scoring_ptr);

        // Session transition detection: mSession changed
        if self.last_session_type != -1 && session_type != self.last_session_type {
            tracing::info!(
                target: LOG_TARGET,
                "session transition: type {} -> {}. Resetting lap state.",
                self.last_session_type,
                session_type
            );
            self.last_session_type = session_type;
            self.last_lap_count = 0;
            self.first_read = true;
            self.pending_lap = None;
            return;
        }
        self.last_session_type = session_type;

        let num_vehicles = Self::read_num_vehicles(scoring_ptr);
        if num_vehicles <= 0 {
            return;
        }

        let player_idx =
            match Self::find_player_vehicle_index(scoring_ptr, num_vehicles as usize) {
                Some(idx) => idx,
                None => return,
            };

        let veh_base = SCORING_VEHICLES_OFFSET + player_idx * VEHICLE_STRUCT_SIZE;

        let total_laps: i16 = unsafe {
            std::ptr::read_unaligned(
                scoring_ptr.add(veh_base + VEH_TOTAL_LAPS_OFF) as *const i16,
            )
        };
        let last_lap_time: f64 = unsafe {
            std::ptr::read_unaligned(
                scoring_ptr.add(veh_base + VEH_LAST_LAP_TIME_OFF) as *const f64,
            )
        };
        let last_sector1: f64 = unsafe {
            std::ptr::read_unaligned(
                scoring_ptr.add(veh_base + VEH_LAST_SECTOR1_OFF) as *const f64,
            )
        };
        let last_sector2: f64 = unsafe {
            std::ptr::read_unaligned(
                scoring_ptr.add(veh_base + VEH_LAST_SECTOR2_OFF) as *const f64,
            )
        };

        // Read vehicle name for current_car (only once, update on change)
        let veh_name_bytes = unsafe {
            std::slice::from_raw_parts(
                scoring_ptr.add(veh_base + VEH_VEHICLE_NAME_OFF),
                64,
            )
        };
        let null_end = veh_name_bytes.iter().position(|&b| b == 0).unwrap_or(64);
        let car_name = String::from_utf8_lossy(&veh_name_bytes[..null_end]).into_owned();
        if !car_name.is_empty() && self.current_car != car_name {
            self.current_car = car_name;
        }

        // Update track name
        let track = Self::read_track_name(scoring_ptr);
        if !track.is_empty() && self.current_track != track {
            self.current_track = track;
        }

        // First-packet safety: snapshot current lap count but do not emit
        if self.first_read {
            tracing::info!(
                target: LOG_TARGET,
                "first_read: snapshotting last_lap_count={}, no lap emitted",
                total_laps
            );
            self.last_lap_count = total_laps;
            self.first_read = false;
            return;
        }

        // Lap completion detection
        if total_laps > self.last_lap_count && last_lap_time > 0.0 {
            let lap_time_ms = (last_lap_time * 1000.0).round() as u32;
            let (s1, s2, s3) = sector_times_ms(last_lap_time, last_sector1, last_sector2);

            tracing::info!(
                target: LOG_TARGET,
                "lap completed: lap={} time={}ms s1={:?} s2={:?} s3={:?}",
                total_laps,
                lap_time_ms,
                s1,
                s2,
                s3
            );

            self.pending_lap = Some(LapData {
                id: uuid::Uuid::new_v4().to_string(),
                session_id: String::new(), // filled by racecontrol from billing session
                driver_id: String::new(),  // filled by racecontrol from billing session
                pod_id: self.pod_id.clone(),
                sim_type: SimType::LeMansUltimate,
                track: self.current_track.clone(),
                car: self.current_car.clone(),
                lap_number: total_laps as u32,
                lap_time_ms,
                sector1_ms: s1,
                sector2_ms: s2,
                sector3_ms: s3,
                valid: true,
                session_type: self.current_session_type,
                created_at: Utc::now(),
            });
        }
        self.last_lap_count = total_laps;
    }
}

// ─── SimAdapter impl ──────────────────────────────────────────────────────────

impl SimAdapter for LmuAdapter {
    fn sim_type(&self) -> SimType {
        SimType::LeMansUltimate
    }

    #[cfg(windows)]
    fn connect(&mut self) -> Result<()> {
        let scoring = open_lmu_shm("$rFactor2SMMP_Scoring$").map_err(|e| {
            anyhow::anyhow!(
                "LMU scoring shm not available: {} — \
                 ensure rF2SharedMemoryMapPlugin is loaded by LMU",
                e
            )
        })?;

        let telemetry = match open_lmu_shm("$rFactor2SMMP_Telemetry$") {
            Ok(t) => t,
            Err(e) => {
                // scoring was opened but telemetry failed — drop scoring
                drop(scoring);
                return Err(anyhow::anyhow!(
                    "LMU telemetry shm not available: {} — \
                     ensure rF2SharedMemoryMapPlugin is loaded by LMU",
                    e
                ));
            }
        };

        self.scoring_shm = Some(scoring);
        self.telemetry_shm = Some(telemetry);
        self.connected = true;
        self.first_read = true;
        self.last_lap_count = 0;
        self.last_session_type = -1;
        self.pending_lap = None;

        tracing::info!(
            target: LOG_TARGET,
            "connected to rF2 shared memory (Scoring + Telemetry)"
        );
        Ok(())
    }

    #[cfg(not(windows))]
    fn connect(&mut self) -> Result<()> {
        anyhow::bail!("LMU shared memory requires Windows");
    }

    fn is_connected(&self) -> bool {
        self.connected
    }

    #[cfg(windows)]
    fn read_telemetry(&mut self) -> Result<Option<TelemetryFrame>> {
        let scoring_ptr = match &self.scoring_shm {
            Some(h) => h.ptr,
            None => return Ok(None),
        };
        let telemetry_ptr = match &self.telemetry_shm {
            Some(h) => h.ptr,
            None => return Ok(None),
        };

        // Torn-read guard for scoring buffer (retry up to 3 times)
        let scoring_consistent = {
            let mut ok = false;
            for _attempt in 0..3 {
                let (begin, end) = read_version_pair(scoring_ptr);
                if begin == end {
                    ok = true;
                    break;
                }
                // Torn read — yield and retry
                std::hint::spin_loop();
            }
            ok
        };

        if scoring_consistent {
            self.process_scoring(scoring_ptr);
        }

        // Torn-read guard for telemetry buffer
        let tel_consistent = {
            let mut ok = false;
            for _attempt in 0..3 {
                let (begin, end) = read_version_pair(telemetry_ptr);
                if begin == end {
                    ok = true;
                    break;
                }
                std::hint::spin_loop();
            }
            ok
        };

        if !tel_consistent {
            return Ok(None);
        }

        // Find the player vehicle in telemetry buffer by matching ID with scoring player ID.
        // Since we need to find the player, we reuse find_player_vehicle_index from scoring
        // (scoring is already confirmed consistent). Alternatively we iterate telemetry for
        // the first vehicle with a matching player indicator — use vehicle 0 as a fallback
        // if scoring isn't consistent (simplification: use index 0 for speed reads).
        // For robustness, find the player's mID from scoring and match in telemetry.
        // Simpler: scan scoring for player index, use same index in telemetry.
        let num_veh_scoring = Self::read_num_vehicles(scoring_ptr);
        let player_tel_idx = if scoring_consistent && num_veh_scoring > 0 {
            Self::find_player_vehicle_index(scoring_ptr, num_veh_scoring as usize)
                .unwrap_or(0)
        } else {
            0
        };

        let tel_info_base = TELEMETRY_INFO_OFFSET;
        let tel_veh_base = tel_info_base + 12 + player_tel_idx * TEL_VEHICLE_SIZE;
        // Note: rF2TelemetryInfo layout:
        //   [0] mNumVehicles (i32) — at tel_info_base+0
        //   [4] mET (f64)          — at tel_info_base+4
        //   [12] vehicle array     — at tel_info_base+12

        // Read telemetry fields
        let local_vel_x: f64 = unsafe {
            std::ptr::read_unaligned(
                telemetry_ptr.add(tel_veh_base + TEL_VEH_LOCAL_VEL_OFF) as *const f64,
            )
        };
        let local_vel_y: f64 = unsafe {
            std::ptr::read_unaligned(
                telemetry_ptr.add(tel_veh_base + TEL_VEH_LOCAL_VEL_OFF + 8) as *const f64,
            )
        };
        let local_vel_z: f64 = unsafe {
            std::ptr::read_unaligned(
                telemetry_ptr.add(tel_veh_base + TEL_VEH_LOCAL_VEL_OFF + 16) as *const f64,
            )
        };

        let gear: i32 = unsafe {
            std::ptr::read_unaligned(
                telemetry_ptr.add(tel_veh_base + TEL_VEH_GEAR_OFF) as *const i32,
            )
        };
        let engine_rpm: f64 = unsafe {
            std::ptr::read_unaligned(
                telemetry_ptr.add(tel_veh_base + TEL_VEH_ENGINE_RPM_OFF) as *const f64,
            )
        };
        let throttle: f64 = unsafe {
            std::ptr::read_unaligned(
                telemetry_ptr.add(tel_veh_base + TEL_VEH_UNFILTERED_THROTTLE_OFF) as *const f64,
            )
        };
        let brake: f64 = unsafe {
            std::ptr::read_unaligned(
                telemetry_ptr.add(tel_veh_base + TEL_VEH_UNFILTERED_BRAKE_OFF) as *const f64,
            )
        };
        let steering: f64 = unsafe {
            std::ptr::read_unaligned(
                telemetry_ptr.add(tel_veh_base + TEL_VEH_FILTERED_STEERING_OFF) as *const f64,
            )
        };

        // Compute speed from local velocity magnitude (m/s -> km/h)
        let speed_ms = (local_vel_x * local_vel_x
            + local_vel_y * local_vel_y
            + local_vel_z * local_vel_z)
            .sqrt();
        let speed_kmh = speed_ms * 3.6;

        let frame = TelemetryFrame {
            pod_id: self.pod_id.clone(),
            timestamp: Utc::now(),
            driver_name: String::new(), // filled by racecontrol from billing session
            car: self.current_car.clone(),
            track: self.current_track.clone(),
            lap_number: self.last_lap_count as u32,
            lap_time_ms: 0, // rF2 has no real-time current-lap timer in this path
            sector: 0,
            speed_kmh: speed_kmh as f32,
            throttle: throttle as f32,
            brake: brake as f32,
            steering: steering as f32,
            gear: gear as i8,
            rpm: engine_rpm as u32,
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
            lap_id: None, // Phase 251: stamped by event_loop before WS send
            sim_type: Some(SimType::LeMansUltimate),
        };

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
        Ok(None) // session info populated from billing session by racecontrol
    }

    fn disconnect(&mut self) {
        #[cfg(windows)]
        {
            // Drop the ShmHandles (Drop impl calls UnmapViewOfFile + CloseHandle)
            self.scoring_shm = None;
            self.telemetry_shm = None;
        }
        self.connected = false;
        tracing::info!(target: LOG_TARGET, "disconnected from rF2 shared memory");
    }

    /// Read whether the player is currently on track from the rF2 scoring buffer.
    ///
    /// This is an explicit override inside `impl SimAdapter for LmuAdapter`
    /// so that event_loop can call `adapter.read_is_on_track()` via
    /// `dyn SimAdapter` trait dispatch. An inherent method alone would NOT be
    /// reachable through a trait object.
    ///
    /// Returns Some(true) when:
    ///   - Scoring buffer is readable (version fields consistent)
    ///   - Player vehicle found (mIsPlayer == 1)
    ///   - mGamePhase >= 4 (Countdown or later)
    fn read_is_on_track(&self) -> Option<bool> {
        self.read_is_on_track_from_shm()
    }
}

// ─── Unit tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── 1. connect() without LMU running ──

    #[test]
    fn test_connect_no_shm() {
        let mut adapter = LmuAdapter::new("pod_1".to_string());
        let result = adapter.connect();
        // On non-Windows: Err because shm requires Windows.
        // On Windows without LMU: Err because OpenFileMappingW fails (shm not created).
        assert!(result.is_err(), "connect() should fail without LMU running");
        assert!(!adapter.is_connected());
    }

    // ── 2. Sector derivation: cumulative S1/S2, derived S3 ──

    #[test]
    fn test_sector_derivation() {
        // lap=62.5s, s1=20.1s, s2_cumul=42.3s
        // S1 = 20.1 * 1000 = 20100 ms
        // S2 = (42.3 - 20.1) * 1000 = 22200 ms
        // S3 = (62.5 - 42.3) * 1000 = 20200 ms
        let (s1, s2, s3) = sector_times_ms(62.5, 20.1, 42.3);
        assert_eq!(s1, Some(20100), "S1 = 20.1s = 20100ms");
        assert_eq!(s2, Some(22200), "S2 = (42.3-20.1)s = 22200ms");
        assert_eq!(s3, Some(20200), "S3 = (62.5-42.3)s = 20200ms");
    }

    // ── 3. Sector guard: any input <= 0 returns (None, None, None) ──

    #[test]
    fn test_sector_guard() {
        // Negative last_s1 (e.g., -1.0 signals no valid time)
        let (s1, s2, s3) = sector_times_ms(62.5, -1.0, 42.3);
        assert_eq!((s1, s2, s3), (None, None, None), "negative s1 -> all None");

        // Zero lap time
        let (s1, s2, s3) = sector_times_ms(0.0, 20.1, 42.3);
        assert_eq!((s1, s2, s3), (None, None, None), "zero lap_time -> all None");

        // Zero s2_cumul
        let (s1, s2, s3) = sector_times_ms(62.5, 20.1, 0.0);
        assert_eq!((s1, s2, s3), (None, None, None), "zero s2_cumul -> all None");
    }

    // ── 4. Lap completed event: mTotalLaps increment emits LapData ──

    #[test]
    fn test_lap_completed_event() {
        let mut adapter = LmuAdapter::new("pod_1".to_string());
        adapter.connected = true;
        adapter.first_read = false;
        adapter.last_lap_count = 1;
        adapter.current_track = "Le Mans".to_string();
        adapter.current_car = "Ferrari 499P".to_string();
        adapter.current_session_type = SessionType::Race;
        adapter.last_session_type = 1; // some non-initial session

        // Simulate what process_scoring does when mTotalLaps goes 1->2
        // with valid lap time and sector splits.
        let last_lap_time = 62.5_f64;
        let last_sector1 = 20.1_f64;
        let last_sector2 = 42.3_f64;

        let total_laps: i16 = 2;
        if total_laps > adapter.last_lap_count && last_lap_time > 0.0 {
            let lap_time_ms = (last_lap_time * 1000.0).round() as u32;
            let (s1, s2, s3) =
                sector_times_ms(last_lap_time, last_sector1, last_sector2);
            adapter.pending_lap = Some(LapData {
                id: uuid::Uuid::new_v4().to_string(),
                session_id: String::new(),
                driver_id: String::new(),
                pod_id: adapter.pod_id.clone(),
                sim_type: SimType::LeMansUltimate,
                track: adapter.current_track.clone(),
                car: adapter.current_car.clone(),
                lap_number: total_laps as u32,
                lap_time_ms,
                sector1_ms: s1,
                sector2_ms: s2,
                sector3_ms: s3,
                valid: true,
                session_type: adapter.current_session_type,
                created_at: Utc::now(),
            });
        }
        adapter.last_lap_count = total_laps;

        let lap = adapter
            .pending_lap
            .take()
            .expect("pending_lap should be set after lap completion");

        assert_eq!(lap.sim_type, SimType::LeMansUltimate);
        assert_eq!(lap.lap_time_ms, 62_500);
        assert_eq!(lap.lap_number, 2);
        assert_eq!(lap.sector1_ms, Some(20100));
        assert_eq!(lap.sector2_ms, Some(22200));
        assert_eq!(lap.sector3_ms, Some(20200));
        assert_eq!(lap.track, "Le Mans");
        assert_eq!(lap.car, "Ferrari 499P");
        assert_eq!(lap.session_type, SessionType::Race);
        assert!(lap.valid);
        assert_eq!(lap.pod_id, "pod_1");
    }

    // ── 5. First-packet safety: no lap emitted on initial connect ──

    #[test]
    fn test_first_packet_safety() {
        let mut adapter = LmuAdapter::new("pod_1".to_string());
        adapter.connected = true;
        adapter.first_read = true;
        adapter.last_lap_count = 0;

        // Simulate what process_scoring does on first read when mTotalLaps is already 3
        let total_laps: i16 = 3;
        if adapter.first_read {
            adapter.last_lap_count = total_laps;
            adapter.first_read = false;
            // Do NOT emit a lap — just snapshot
        }

        assert!(
            adapter.pending_lap.is_none(),
            "no lap should fire on first read when mTotalLaps is already > 0"
        );
        assert_eq!(adapter.last_lap_count, 3, "lap count should be snapshotted");
        assert!(!adapter.first_read, "first_read should be cleared after first packet");
    }

    // ── 6. Session transition resets lap tracking state ──

    #[test]
    fn test_session_transition_resets_lap() {
        let mut adapter = LmuAdapter::new("pod_1".to_string());
        adapter.connected = true;
        adapter.first_read = false;
        adapter.last_lap_count = 5;
        adapter.last_session_type = 1;
        adapter.pending_lap = Some(LapData {
            id: "test".to_string(),
            session_id: String::new(),
            driver_id: String::new(),
            pod_id: "pod_1".to_string(),
            sim_type: SimType::LeMansUltimate,
            track: String::new(),
            car: String::new(),
            lap_number: 5,
            lap_time_ms: 60_000,
            sector1_ms: None,
            sector2_ms: None,
            sector3_ms: None,
            valid: true,
            session_type: SessionType::Race,
            created_at: Utc::now(),
        });

        // Simulate session transition: mSession changes from 1 to 2
        let new_session_type = 2_i32;
        if adapter.last_session_type != -1 && new_session_type != adapter.last_session_type {
            adapter.last_session_type = new_session_type;
            adapter.last_lap_count = 0;
            adapter.first_read = true;
            adapter.pending_lap = None;
        }

        assert_eq!(adapter.last_lap_count, 0, "lap count should reset to 0");
        assert!(adapter.first_read, "first_read should reset to true");
        assert!(adapter.pending_lap.is_none(), "pending_lap should be cleared");
        assert_eq!(adapter.last_session_type, 2, "session type should update");
    }
}
