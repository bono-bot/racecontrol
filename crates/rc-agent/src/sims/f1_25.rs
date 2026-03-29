use std::net::UdpSocket;

use anyhow::{Context, Result};
use chrono::Utc;
use rc_common::types::*;
use tokio::sync::mpsc;

use super::SimAdapter;
use crate::driving_detector::DetectorSignal;

const LOG_TARGET: &str = "sim-f1";

/// EA Sports F1 25 UDP telemetry adapter
///
/// Passive listener on UDP port 20777. The game broadcasts telemetry packets
/// without requiring a handshake — we just bind and receive.
///
/// Protocol: Little-endian packed binary, 29-byte header, 16 packet types.
/// We parse packets 1 (Session), 2 (LapData), 4 (Participants),
/// 6 (CarTelemetry), and 7 (CarStatus) for the player car.
pub struct F125Adapter {
    pod_id: String,
    socket: Option<UdpSocket>,
    connected: bool,
    signal_tx: Option<mpsc::Sender<DetectorSignal>>,

    // Header state
    player_car_index: u8,
    session_uid: u64,

    // From Packet 4 (Participants)
    driver_name: String,
    team_id: u8,

    // From Packet 1 (Session)
    track_id: i8,
    track_name: String,
    session_type: u8,

    // From Packet 6 (CarTelemetry)
    speed_kmh: u16,
    throttle: f32,
    brake: f32,
    steer: f32,
    gear: i8,
    rpm: u16,
    drs_active: bool,

    // From Packet 2 (LapData)
    current_lap_num: u8,
    current_lap_time_ms: u32,
    last_lap_time_ms: u32,
    sector: u8,
    sector1_ms: Option<u32>,
    sector2_ms: Option<u32>,
    current_lap_invalid: bool,

    // From Packet 7 (CarStatus)
    ers_deploy_mode: u8,
    ers_store_energy: f32,
    drs_allowed: bool,

    // Tracking
    best_lap_ms: u32,
    last_completed_lap: Option<LapData>,
    prev_lap_num: u8,
    prev_sector: u8,
}

// F1 25 packet header size
const HEADER_SIZE: usize = 29;

// Packet IDs
const PACKET_SESSION: u8 = 1;
const PACKET_LAP_DATA: u8 = 2;
const PACKET_PARTICIPANTS: u8 = 4;
const PACKET_CAR_TELEMETRY: u8 = 6;
const PACKET_CAR_STATUS: u8 = 7;

// Per-car data sizes (bytes)
const CAR_TELEMETRY_SIZE: usize = 60;
const LAP_DATA_SIZE: usize = 57;
const CAR_STATUS_SIZE: usize = 55;
const PARTICIPANT_SIZE: usize = 56;

// F1 ERS max energy store (Joules)
const ERS_MAX_ENERGY: f32 = 4_000_000.0;

impl F125Adapter {
    pub fn new(pod_id: String, signal_tx: Option<mpsc::Sender<DetectorSignal>>) -> Self {
        Self {
            pod_id,
            socket: None,
            connected: false,
            signal_tx,
            player_car_index: 0,
            session_uid: 0,
            driver_name: String::new(),
            team_id: 0,
            track_id: -1,
            track_name: String::new(),
            session_type: 0,
            speed_kmh: 0,
            throttle: 0.0,
            brake: 0.0,
            steer: 0.0,
            gear: 0,
            rpm: 0,
            drs_active: false,
            current_lap_num: 0,
            current_lap_time_ms: 0,
            last_lap_time_ms: 0,
            sector: 0,
            sector1_ms: None,
            sector2_ms: None,
            current_lap_invalid: false,
            ers_deploy_mode: 0,
            ers_store_energy: 0.0,
            drs_allowed: false,
            best_lap_ms: 0,
            last_completed_lap: None,
            prev_lap_num: 0,
            prev_sector: 0,
        }
    }

    /// Parse the 29-byte packet header. Returns (packet_id, player_car_index).
    fn parse_header(&mut self, buf: &[u8]) -> Option<u8> {
        if buf.len() < HEADER_SIZE {
            return None;
        }

        let packet_format = u16::from_le_bytes([buf[0], buf[1]]);
        if packet_format != 2025 {
            return None;
        }

        let packet_id = buf[5];
        let session_uid = u64::from_le_bytes(buf[6..14].try_into().ok()?);

        self.player_car_index = buf[21];
        self.session_uid = session_uid;

        Some(packet_id)
    }

    /// Process a single UDP packet — updates internal state.
    fn process_packet(&mut self, buf: &[u8]) {
        let packet_id = match self.parse_header(buf) {
            Some(id) => id,
            None => return,
        };

        let data = &buf[HEADER_SIZE..];
        let idx = self.player_car_index as usize;

        match packet_id {
            PACKET_CAR_TELEMETRY => self.parse_car_telemetry(data, idx),
            PACKET_LAP_DATA => self.parse_lap_data(data, idx),
            PACKET_CAR_STATUS => self.parse_car_status(data, idx),
            PACKET_PARTICIPANTS => self.parse_participants(data, idx),
            PACKET_SESSION => self.parse_session(data),
            _ => {}
        }
    }

    /// Packet 6 — CarTelemetry (60 bytes per car)
    /// Layout per car:
    ///   0: u16 speed (KPH)
    ///   2: f32 throttle (0.0–1.0)
    ///   6: f32 steer (-1.0 to 1.0)
    ///  10: f32 brake (0.0–1.0)
    ///  14: u8 clutch
    ///  15: i8 gear (-1=R, 0=N, 1-8)
    ///  16: u16 engineRPM
    ///  18: u8 drs (0=off, 1=on)
    ///  19: u8 revLightsPercent
    ///  20: u16 revLightsBitValue
    ///  22: u16[4] brakesTemperature (8 bytes)
    ///  30: u8[4] tyresSurfaceTemperature
    ///  34: u8[4] tyresInnerTemperature
    ///  38: u16 engineTemperature
    ///  40: f32[4] tyresPressure (16 bytes)
    ///  56: u8[4] surfaceType
    /// = 60 bytes total per car
    fn parse_car_telemetry(&mut self, data: &[u8], idx: usize) {
        let offset = idx * CAR_TELEMETRY_SIZE;
        if data.len() < offset + CAR_TELEMETRY_SIZE {
            return;
        }
        let car = &data[offset..];

        self.speed_kmh = u16::from_le_bytes([car[0], car[1]]);
        self.throttle = f32::from_le_bytes(car[2..6].try_into().unwrap_or_default());
        self.steer = f32::from_le_bytes(car[6..10].try_into().unwrap_or_default());
        self.brake = f32::from_le_bytes(car[10..14].try_into().unwrap_or_default());
        self.gear = car[15] as i8;
        self.rpm = u16::from_le_bytes([car[16], car[17]]);
        self.drs_active = car[18] == 1;
    }

    /// Packet 2 — LapData (57 bytes per car)
    /// Layout per car:
    ///   0: u32 lastLapTimeInMS
    ///   4: u32 currentLapTimeInMS
    ///   8: u16 sector1TimeMSPart
    ///  10: u8  sector1TimeMinutesPart
    ///  11: u16 sector2TimeMSPart
    ///  13: u8  sector2TimeMinutesPart
    ///  14: u16 deltaToCarInFrontMSPart
    ///  16: u8  deltaToCarInFrontMinutesPart
    ///  17: u16 deltaToRaceLeaderMSPart
    ///  19: u8  deltaToRaceLeaderMinutesPart
    ///  20: f32 lapDistance
    ///  24: f32 totalDistance
    ///  28: f32 safetyCarDelta
    ///  32: u8  carPosition
    ///  33: u8  currentLapNum
    ///  34: u8  pitStatus
    ///  35: u8  numPitStops
    ///  36: u8  sector (0=S1, 1=S2, 2=S3)
    ///  37: u8  currentLapInvalid (0=valid, 1=invalid)
    ///  38: u8  penalties
    ///  39: u8  totalWarnings
    ///  40: u8  cornerCuttingWarnings
    ///  41: u8  numUnservedDriveThrough
    ///  42: u8  numUnservedStopGo
    ///  43: u8  gridPosition
    ///  44: u8  driverStatus
    ///  45: u8  resultStatus
    ///  46: u8  pitLaneTimerActive
    ///  47: u16 pitLaneTimeInLaneInMS
    ///  49: u16 pitStopTimerInMS
    ///  51: u8  pitStopShouldServePen
    ///  52: f32 speedTrapFastestSpeed
    ///  56: u8  speedTrapFastestLap
    /// = 57 bytes total per car
    fn parse_lap_data(&mut self, data: &[u8], idx: usize) {
        let offset = idx * LAP_DATA_SIZE;
        if data.len() < offset + LAP_DATA_SIZE {
            return;
        }
        let car = &data[offset..];

        let last_lap_time_ms =
            u32::from_le_bytes(car[0..4].try_into().unwrap_or_default());
        let current_lap_time_ms =
            u32::from_le_bytes(car[4..8].try_into().unwrap_or_default());

        // Sector 1 time = minutes * 60000 + ms_part
        let s1_ms_part = u16::from_le_bytes([car[8], car[9]]) as u32;
        let s1_min_part = car[10] as u32;
        let s1_total = s1_min_part * 60_000 + s1_ms_part;

        // Sector 2 time
        let s2_ms_part = u16::from_le_bytes([car[11], car[12]]) as u32;
        let s2_min_part = car[13] as u32;
        let s2_total = s2_min_part * 60_000 + s2_ms_part;

        let current_lap_num = car[33];
        let sector = car[36];
        let current_lap_invalid = car[37] == 1;

        // Track sector transitions to capture split times
        if sector > self.prev_sector || (sector == 0 && self.prev_sector == 2) {
            match self.prev_sector {
                0 => {
                    // Completed S1
                    if s1_total > 0 {
                        self.sector1_ms = Some(s1_total);
                    }
                }
                1 => {
                    // Completed S2
                    if s2_total > 0 {
                        self.sector2_ms = Some(s2_total);
                    }
                }
                _ => {}
            }
        }
        self.prev_sector = sector;

        // Detect lap completion
        if current_lap_num > self.prev_lap_num && self.prev_lap_num > 0 && last_lap_time_ms > 0 {
            // Calculate S3 from total - S1 - S2
            let s3_ms = match (self.sector1_ms, self.sector2_ms) {
                (Some(s1), Some(s2)) if last_lap_time_ms > s1 + s2 => {
                    Some(last_lap_time_ms - s1 - s2)
                }
                _ => None,
            };

            let lap_session_type = match self.session_type {
                1 | 2 | 3 | 4 => SessionType::Practice,
                5 | 6 | 7 | 8 => SessionType::Qualifying,
                9 | 10 | 11 => SessionType::Race,
                12 => SessionType::Hotlap,
                _ => SessionType::Practice,
            };

            let lap = LapData {
                id: uuid::Uuid::new_v4().to_string(),
                session_id: String::new(),
                driver_id: String::new(),
                pod_id: self.pod_id.clone(),
                sim_type: SimType::F125,
                track: self.track_name.clone(),
                car: team_name(self.team_id).to_string(),
                lap_number: self.prev_lap_num as u32,
                lap_time_ms: last_lap_time_ms,
                sector1_ms: self.sector1_ms,
                sector2_ms: self.sector2_ms,
                sector3_ms: s3_ms,
                valid: !self.current_lap_invalid,
                session_type: lap_session_type,
                created_at: Utc::now(),
            };

            // Update best lap
            if self.best_lap_ms == 0 || last_lap_time_ms < self.best_lap_ms {
                if !self.current_lap_invalid {
                    self.best_lap_ms = last_lap_time_ms;
                }
            }

            self.last_completed_lap = Some(lap);

            // Reset sector tracking for the new lap
            self.sector1_ms = None;
            self.sector2_ms = None;
            self.current_lap_invalid = false;
        }

        self.prev_lap_num = current_lap_num;
        self.current_lap_num = current_lap_num;
        self.current_lap_time_ms = current_lap_time_ms;
        self.last_lap_time_ms = last_lap_time_ms;
        self.sector = sector;

        // Carry invalidity across the lap (once invalid, stays invalid)
        if current_lap_invalid {
            self.current_lap_invalid = true;
        }
    }

    /// Packet 7 — CarStatus (55 bytes per car)
    /// Layout per car:
    ///   0: u8  tractionControl
    ///   1: u8  antiLockBrakes
    ///   2: u8  fuelMix
    ///   3: u8  frontBrakeBias
    ///   4: u8  pitLimiterStatus
    ///   5: f32 fuelInTank
    ///   9: f32 fuelCapacity
    ///  13: f32 fuelRemainingLaps
    ///  17: u16 maxRPM
    ///  19: u16 idleRPM
    ///  21: u8  maxGears
    ///  22: u8  drsAllowed
    ///  23: u16 drsActivationDistance
    ///  25: u8  actualTyreCompound
    ///  26: u8  visualTyreCompound
    ///  27: u8  tyresAgeLaps
    ///  28: i8  vehicleFIAFlags
    ///  29: f32 enginePowerICE
    ///  33: f32 enginePowerMGUK
    ///  37: f32 ersStoreEnergy
    ///  41: u8  ersDeployMode
    ///  42: f32 ersHarvestedThisLapMGUK
    ///  46: f32 ersHarvestedThisLapMGUH
    ///  50: f32 ersDeployedThisLap
    ///  54: u8  networkPaused
    /// = 55 bytes total per car
    fn parse_car_status(&mut self, data: &[u8], idx: usize) {
        let offset = idx * CAR_STATUS_SIZE;
        if data.len() < offset + CAR_STATUS_SIZE {
            return;
        }
        let car = &data[offset..];

        self.drs_allowed = car[22] == 1;
        self.ers_store_energy =
            f32::from_le_bytes(car[37..41].try_into().unwrap_or_default());
        self.ers_deploy_mode = car[41];
    }

    /// Packet 4 — Participants
    /// Header byte 0: u8 numActiveCars
    /// Then array of participants, each 56 bytes:
    ///   0: u8  aiControlled
    ///   1: u8  driverId
    ///   2: u8  networkId
    ///   3: u8  teamId
    ///   4: u8  myTeam
    ///   5: u8  raceNumber
    ///   6: u8  nationality
    ///   7: char[32] name (UTF-8 null-terminated)
    ///  39: u8  yourTelemetry
    ///  40: u8  showOnlineNames
    ///  41: u16 techLevel
    ///  43: u8  platform
    ///  44: u8  numColours
    ///  45: LiveryColour[4] (3 bytes each = 12 bytes, but only numColours used)
    ///     ... totalling 56 bytes per participant (approximate — may have padding)
    fn parse_participants(&mut self, data: &[u8], idx: usize) {
        if data.is_empty() {
            return;
        }

        // Each participant entry is after the numActiveCars byte
        let entry_offset = 1 + idx * PARTICIPANT_SIZE;
        if data.len() < entry_offset + PARTICIPANT_SIZE {
            return;
        }
        let entry = &data[entry_offset..];

        self.team_id = entry[3];

        // Name starts at byte 7, 32 bytes UTF-8 null-terminated
        let name_bytes = &entry[7..39];
        self.driver_name = parse_f1_string(name_bytes);
    }

    /// Packet 1 — Session
    /// Relevant fields at the start of the data section:
    ///   0: u8  weather
    ///   1: i8  trackTemperature
    ///   2: i8  airTemperature
    ///   3: u8  totalLaps
    ///   4: u16 trackLength
    ///   6: u8  sessionType
    ///   7: i8  trackId
    fn parse_session(&mut self, data: &[u8]) {
        if data.len() < 8 {
            return;
        }

        self.session_type = data[6];
        self.track_id = data[7] as i8;
        self.track_name = track_name(self.track_id).to_string();
    }
}

impl SimAdapter for F125Adapter {
    fn sim_type(&self) -> SimType {
        SimType::F125
    }

    fn connect(&mut self) -> Result<()> {
        let socket = UdpSocket::bind("0.0.0.0:20777")
            .context("Failed to bind F1 25 telemetry port 20777")?;
        socket.set_nonblocking(true)?;
        self.socket = Some(socket);
        self.connected = true;
        tracing::info!(target: LOG_TARGET, "F1 25 adapter listening on UDP port 20777");
        Ok(())
    }

    fn is_connected(&self) -> bool {
        self.connected
    }

    fn read_telemetry(&mut self) -> Result<Option<TelemetryFrame>> {
        if self.socket.is_none() {
            anyhow::bail!("Not bound");
        }

        // Collect packets first to avoid borrow conflict
        let mut packets: Vec<Vec<u8>> = Vec::new();
        {
            let socket = match self.socket.as_ref() {
                Some(s) => s,
                None => anyhow::bail!("Socket not bound despite prior check"),
            };
            let mut buf = [0u8; 2048];
            loop {
                match socket.recv_from(&mut buf) {
                    Ok((len, _)) if len > HEADER_SIZE => {
                        packets.push(buf[..len].to_vec());
                    }
                    Ok(_) => break,
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                    Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => break,
                    Err(e) => return Err(e.into()),
                }
            }
        }

        let got_data = !packets.is_empty();
        for packet in &packets {
            self.process_packet(packet);
        }

        if !got_data {
            return Ok(None);
        }

        // Signal driving detector that we're receiving telemetry
        if let Some(ref tx) = self.signal_tx {
            let _ = tx.try_send(DetectorSignal::UdpActive);
        }

        // ERS percentage
        let ers_percent = if ERS_MAX_ENERGY > 0.0 {
            (self.ers_store_energy / ERS_MAX_ENERGY * 100.0).clamp(0.0, 100.0)
        } else {
            0.0
        };

        Ok(Some(TelemetryFrame {
            pod_id: self.pod_id.clone(),
            timestamp: Utc::now(),
            driver_name: self.driver_name.clone(),
            car: team_name(self.team_id).to_string(),
            track: self.track_name.clone(),
            lap_number: self.current_lap_num as u32,
            lap_time_ms: self.current_lap_time_ms,
            sector: self.sector,
            speed_kmh: self.speed_kmh as f32,
            throttle: self.throttle,
            brake: self.brake,
            steering: self.steer,
            gear: self.gear,
            rpm: self.rpm as u32,
            position: None,
            session_time_ms: self.current_lap_time_ms,
            // F1-specific
            drs_active: Some(self.drs_active),
            drs_available: Some(self.drs_allowed),
            ers_deploy_mode: Some(self.ers_deploy_mode),
            ers_store_percent: Some(ers_percent),
            best_lap_ms: if self.best_lap_ms > 0 {
                Some(self.best_lap_ms)
            } else {
                None
            },
            current_lap_invalid: Some(self.current_lap_invalid),
            sector1_ms: self.sector1_ms,
            sector2_ms: self.sector2_ms,
            sector3_ms: None, // S3 only known at lap completion
            lap_id: None, // Phase 251: stamped by event_loop before WS send
        }))
    }

    fn poll_lap_completed(&mut self) -> Result<Option<LapData>> {
        Ok(self.last_completed_lap.take())
    }

    fn session_info(&self) -> Result<Option<SessionInfo>> {
        if !self.connected || self.track_name.is_empty() {
            return Ok(None);
        }
        let session_type = match self.session_type {
            1 | 2 | 3 | 4 => SessionType::Practice,
            5 | 6 | 7 | 8 => SessionType::Qualifying,
            9 | 10 | 11 => SessionType::Race,
            12 => SessionType::Hotlap,
            _ => SessionType::Practice,
        };
        Ok(Some(SessionInfo {
            id: String::new(),
            session_type,
            sim_type: SimType::F125,
            track: self.track_name.clone(),
            car_class: None,
            status: SessionStatus::Active,
            max_drivers: None,
            laps_or_minutes: None,
            started_at: None,
            ended_at: None,
        }))
    }

    fn disconnect(&mut self) {
        self.socket = None;
        self.connected = false;
        tracing::info!(target: LOG_TARGET, "F1 25 UDP socket closed (port 20777) — game exit cleanup");
    }
}

// ─── Helpers ────────────────────────────────────────────────────────────────

/// Parse a UTF-8 null-terminated string from F1 participant data
fn parse_f1_string(buf: &[u8]) -> String {
    let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
    String::from_utf8_lossy(&buf[..end]).to_string()
}

/// F1 25 Track ID → Track Name
fn track_name(id: i8) -> &'static str {
    match id {
        0 => "Melbourne",
        1 => "Paul Ricard",
        2 => "Shanghai",
        3 => "Sakhir",
        4 => "Catalunya",
        5 => "Monaco",
        6 => "Montreal",
        7 => "Silverstone",
        8 => "Hockenheim",
        9 => "Hungaroring",
        10 => "Spa",
        11 => "Monza",
        12 => "Singapore",
        13 => "Suzuka",
        14 => "Abu Dhabi",
        15 => "Austin",
        16 => "Interlagos",
        17 => "Red Bull Ring",
        18 => "Sochi",
        19 => "Mexico City",
        20 => "Baku",
        21 => "Sakhir Short",
        22 => "Silverstone Short",
        23 => "Austin Short",
        24 => "Suzuka Short",
        25 => "Hanoi",
        26 => "Zandvoort",
        27 => "Imola",
        28 => "Portimao",
        29 => "Jeddah",
        30 => "Miami",
        31 => "Las Vegas",
        32 => "Losail",
        _ => "Unknown Track",
    }
}

/// F1 25 Team ID → Team Name
fn team_name(id: u8) -> &'static str {
    match id {
        0 => "Mercedes",
        1 => "Ferrari",
        2 => "Red Bull Racing",
        3 => "Williams",
        4 => "Aston Martin",
        5 => "Alpine",
        6 => "RB",
        7 => "Haas",
        8 => "McLaren",
        9 => "Sauber",
        85 => "Mercedes 2020",
        86 => "Ferrari 2020",
        87 => "Red Bull 2020",
        88 => "Williams 2020",
        89 => "Racing Point 2020",
        90 => "Renault 2020",
        91 => "AlphaTauri 2020",
        92 => "Haas 2020",
        93 => "McLaren 2020",
        94 => "Alfa Romeo 2020",
        104 => "Audi",
        143 => "Art GP",
        144 => "Campos",
        145 => "Carlin",
        146 => "Charouz",
        147 => "Dams",
        148 => "Uni-Virtuosi",
        149 => "MP Motorsport",
        150 => "Prema",
        151 => "Trident",
        152 => "BWT",
        153 => "Hitech",
        154 => "PHM",
        _ => "Unknown Team",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal F1 25 UDP packet with the 2025 header.
    /// Returns a Vec<u8> = 29-byte header + data bytes.
    /// player_car_index is always 0 (player is car 0).
    fn build_test_packet(packet_id: u8, data: &[u8]) -> Vec<u8> {
        let mut buf = vec![0u8; HEADER_SIZE + data.len()];
        // packet_format = 2025 little-endian
        buf[0] = 0xE9; // 0x07E9 = 2025
        buf[1] = 0x07;
        // packet_id at byte 5
        buf[5] = packet_id;
        // session_uid bytes 6..14 — leave as zeros (valid)
        // player_car_index at byte 21 = 0 (player is car index 0)
        buf[21] = 0;
        // copy data after header
        buf[HEADER_SIZE..].copy_from_slice(data);
        buf
    }

    /// Build a 57-byte LapData car buffer for player car (index 0).
    fn build_lap_data_car(
        last_lap_ms: u32,
        current_lap_ms: u32,
        s1_ms_part: u16,
        s1_min: u8,
        s2_ms_part: u16,
        s2_min: u8,
        lap_num: u8,
        sector: u8,
        invalid: u8,
    ) -> Vec<u8> {
        let mut car = vec![0u8; LAP_DATA_SIZE];
        car[0..4].copy_from_slice(&last_lap_ms.to_le_bytes());
        car[4..8].copy_from_slice(&current_lap_ms.to_le_bytes());
        car[8..10].copy_from_slice(&s1_ms_part.to_le_bytes());
        car[10] = s1_min;
        car[11..13].copy_from_slice(&s2_ms_part.to_le_bytes());
        car[13] = s2_min;
        car[33] = lap_num;
        car[36] = sector;
        car[37] = invalid;
        car
    }

    /// Build a Session data buffer (8 bytes minimum).
    fn build_session_data(session_type: u8, track_id: u8) -> Vec<u8> {
        let mut data = vec![0u8; 8];
        data[6] = session_type;
        data[7] = track_id; // track_id 11 = Monza
        data
    }

    #[test]
    fn test_parse_header_valid() {
        let mut adapter = F125Adapter::new("test".to_string(), None);
        let mut buf = [0u8; 29];
        // packet_format = 2025 (little-endian)
        buf[0] = 0xE9;
        buf[1] = 0x07;
        // packet_id = 6 (CarTelemetry)
        buf[5] = 6;
        // player_car_index = 0
        buf[21] = 0;

        let result = adapter.parse_header(&buf);
        assert_eq!(result, Some(6));
        assert_eq!(adapter.player_car_index, 0);
    }

    #[test]
    fn test_parse_header_wrong_format() {
        let mut adapter = F125Adapter::new("test".to_string(), None);
        let mut buf = [0u8; 29];
        // packet_format = 2024 (wrong year)
        buf[0] = 0xE8;
        buf[1] = 0x07;

        let result = adapter.parse_header(&buf);
        assert_eq!(result, None);
    }

    #[test]
    fn test_parse_f1_string() {
        let mut buf = [0u8; 32];
        buf[0] = b'M';
        buf[1] = b'a';
        buf[2] = b'x';
        buf[3] = 0;

        assert_eq!(parse_f1_string(&buf), "Max");
    }

    #[test]
    fn test_track_name_lookup() {
        assert_eq!(track_name(11), "Monza");
        assert_eq!(track_name(10), "Spa");
        assert_eq!(track_name(-1), "Unknown Track");
    }

    #[test]
    fn test_team_name_lookup() {
        assert_eq!(team_name(1), "Ferrari");
        assert_eq!(team_name(8), "McLaren");
        assert_eq!(team_name(255), "Unknown Team");
    }

    // ─── New tests for TEL-F1-01, TEL-F1-02, TEL-F1-03 ─────────────────────

    /// TEL-F1-02: Lap completion produces LapData with correct lap_time_ms and sim_type F125.
    /// Feed lap 1 packet (prev_lap_num stays 0), then lap 2 packet (triggers completion).
    #[test]
    fn test_lap_completion_on_lap_transition() {
        let mut adapter = F125Adapter::new("pod-test".to_string(), None);

        // Packet 1: lap_num=1, sector=0, no last lap time (first packet)
        let car1 = build_lap_data_car(0, 5000, 0, 0, 0, 0, 1, 0, 0);
        let pkt1 = build_test_packet(PACKET_LAP_DATA, &car1);
        adapter.process_packet(&pkt1);

        // No lap yet — first packet establishes prev_lap_num=1
        let result = adapter.poll_lap_completed().unwrap();
        assert!(result.is_none(), "No lap should be produced on first packet");

        // Packet 2: lap_num=2, last_lap_time_ms=90000 — triggers lap 1 completion
        let car2 = build_lap_data_car(90_000, 1000, 0, 0, 0, 0, 2, 0, 0);
        let pkt2 = build_test_packet(PACKET_LAP_DATA, &car2);
        adapter.process_packet(&pkt2);

        let lap = adapter.poll_lap_completed().unwrap();
        assert!(lap.is_some(), "Lap should be produced on lap number transition");
        let lap = lap.unwrap();
        assert_eq!(lap.lap_time_ms, 90_000, "lap_time_ms must match last_lap_time_ms");
        assert_eq!(lap.sim_type, SimType::F125, "sim_type must be F125");
        assert!(lap.valid, "Lap should be valid when no invalid flag set");
    }

    /// TEL-F1-02: Sector split times are captured and sector3_ms = total - S1 - S2.
    #[test]
    fn test_sector_splits_captured() {
        let mut adapter = F125Adapter::new("pod-test".to_string(), None);

        // Lap 1, sector 0 — establish initial state
        let car0 = build_lap_data_car(0, 1000, 0, 0, 0, 0, 1, 0, 0);
        let pkt0 = build_test_packet(PACKET_LAP_DATA, &car0);
        adapter.process_packet(&pkt0);

        // Transition to sector 1 (S1 complete): s1=30000ms (ms_part=30000, min=0)
        // sector field goes from 0 -> 1, s1_ms_part=30000
        let car1 = build_lap_data_car(0, 31_000, 30_000, 0, 0, 0, 1, 1, 0);
        let pkt1 = build_test_packet(PACKET_LAP_DATA, &car1);
        adapter.process_packet(&pkt1);

        // Transition to sector 2 (S2 complete): s2=28000ms (ms_part=28000, min=0)
        let car2 = build_lap_data_car(0, 59_000, 30_000, 0, 28_000, 0, 1, 2, 0);
        let pkt2 = build_test_packet(PACKET_LAP_DATA, &car2);
        adapter.process_packet(&pkt2);

        // Lap 2 start (lap completion): total=88000ms, S1=30000, S2=28000 -> S3=30000
        let car3 = build_lap_data_car(88_000, 1_000, 30_000, 0, 28_000, 0, 2, 0, 0);
        let pkt3 = build_test_packet(PACKET_LAP_DATA, &car3);
        adapter.process_packet(&pkt3);

        let lap = adapter.poll_lap_completed().unwrap().expect("Lap must complete");
        assert_eq!(lap.lap_time_ms, 88_000);
        assert_eq!(lap.sector1_ms, Some(30_000), "S1 must be 30000ms");
        assert_eq!(lap.sector2_ms, Some(28_000), "S2 must be 28000ms");
        assert_eq!(lap.sector3_ms, Some(30_000), "S3 = total - S1 - S2 = 30000ms");
    }

    /// TEL-F1-02: When current_lap_invalid=1 is set during a lap, LapData.valid == false.
    #[test]
    fn test_invalid_lap_flagged() {
        let mut adapter = F125Adapter::new("pod-test".to_string(), None);

        // Lap 1 established
        let car0 = build_lap_data_car(0, 5_000, 0, 0, 0, 0, 1, 0, 0);
        let pkt0 = build_test_packet(PACKET_LAP_DATA, &car0);
        adapter.process_packet(&pkt0);

        // Mid-lap: invalid flag set (track limits, etc.)
        let car_inv = build_lap_data_car(0, 50_000, 0, 0, 0, 0, 1, 0, 1);
        let pkt_inv = build_test_packet(PACKET_LAP_DATA, &car_inv);
        adapter.process_packet(&pkt_inv);

        // Lap 2 — triggers completion of lap 1 which was flagged invalid
        let car2 = build_lap_data_car(95_000, 1_000, 0, 0, 0, 0, 2, 0, 0);
        let pkt2 = build_test_packet(PACKET_LAP_DATA, &car2);
        adapter.process_packet(&pkt2);

        let lap = adapter.poll_lap_completed().unwrap().expect("Lap must complete");
        assert!(!lap.valid, "Lap must be marked invalid when current_lap_invalid was set");
    }

    /// TEL-F1-01 + TEL-F1-03: Session type values map to correct SessionType variants.
    /// Values: 1=Practice, 5=Qualifying, 10=Race, 12=Hotlap
    #[test]
    fn test_session_type_mapping() {
        let cases = [
            (1u8, SessionType::Practice),
            (5u8, SessionType::Qualifying),
            (10u8, SessionType::Race),
            (12u8, SessionType::Hotlap),
        ];

        for (session_val, expected) in cases {
            let mut adapter = F125Adapter::new("pod-test".to_string(), None);
            adapter.connected = true; // session_info() requires connected

            // Feed a Session packet (packet id 1)
            let session_data = build_session_data(session_val, 11); // track_id 11 = Monza
            let pkt = build_test_packet(PACKET_SESSION, &session_data);
            adapter.process_packet(&pkt);

            let info = adapter.session_info().unwrap();
            assert!(info.is_some(), "session_info should return Some after Session packet");
            let info = info.unwrap();
            assert_eq!(
                info.session_type, expected,
                "session_type {} should map to {:?}",
                session_val, expected
            );
        }
    }

    /// TEL-F1-02: First LapData packet does not produce a spurious lap completion.
    /// (prev_lap_num starts at 0, condition requires prev_lap_num > 0)
    #[test]
    fn test_no_lap_on_first_packet() {
        let mut adapter = F125Adapter::new("pod-test".to_string(), None);

        // First packet ever: lap_num=1, last_lap_ms=0 (no previous lap recorded)
        let car = build_lap_data_car(0, 12_000, 0, 0, 0, 0, 1, 0, 0);
        let pkt = build_test_packet(PACKET_LAP_DATA, &car);
        adapter.process_packet(&pkt);

        let result = adapter.poll_lap_completed().unwrap();
        assert!(result.is_none(), "No lap should be produced on the very first packet");
    }

    /// TEL-F1-02: poll_lap_completed() uses take() semantics — first call returns Some,
    /// second call returns None (data is cleared after first poll).
    #[test]
    fn test_poll_lap_completed_clears() {
        let mut adapter = F125Adapter::new("pod-test".to_string(), None);

        // Establish lap 1
        let car0 = build_lap_data_car(0, 5_000, 0, 0, 0, 0, 1, 0, 0);
        adapter.process_packet(&build_test_packet(PACKET_LAP_DATA, &car0));

        // Trigger lap completion (lap 2 arrives)
        let car1 = build_lap_data_car(80_000, 1_000, 0, 0, 0, 0, 2, 0, 0);
        adapter.process_packet(&build_test_packet(PACKET_LAP_DATA, &car1));

        // First poll: should return the completed lap
        let first = adapter.poll_lap_completed().unwrap();
        assert!(first.is_some(), "First poll must return the completed lap");

        // Second poll: must return None (cleared)
        let second = adapter.poll_lap_completed().unwrap();
        assert!(second.is_none(), "Second poll must return None — take() semantics");
    }
}
