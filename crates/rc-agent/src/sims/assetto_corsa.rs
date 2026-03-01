use std::net::UdpSocket;
use std::time::Duration;

use anyhow::{Context, Result};
use chrono::Utc;
use rc_common::types::*;
use super::SimAdapter;

/// Assetto Corsa UDP telemetry protocol client
///
/// Connects to acServer's UDP telemetry port (default 9996) to receive
/// real-time race data including lap times, positions, and car telemetry.
///
/// Protocol reference: AC Remote Telemetry documentation by Kunos
pub struct AssettoCorsaAdapter {
    server_ip: String,
    server_port: u16,
    socket: Option<UdpSocket>,
    connected: bool,
    pod_id: String,
    last_lap_count: u32,
    current_driver: String,
    current_car: String,
    current_track: String,
}

// AC UDP protocol operation types
const AC_HANDSHAKE: i32 = 0;
const AC_SUBSCRIBE_UPDATE: i32 = 1;
const AC_SUBSCRIBE_SPOT: i32 = 2;
const AC_DISMISS: i32 = 3;

impl AssettoCorsaAdapter {
    pub fn new(pod_id: String, server_ip: String, server_port: u16) -> Self {
        Self {
            server_ip,
            server_port,
            socket: None,
            connected: false,
            pod_id,
            last_lap_count: 0,
            current_driver: String::new(),
            current_car: String::new(),
            current_track: String::new(),
        }
    }

    /// Send handshake to AC server
    fn send_handshake(&self) -> Result<()> {
        let socket = self.socket.as_ref().context("Not connected")?;

        // AC handshake packet: operation_id (i32 LE) + identifier (i32 LE) + version (i32 LE)
        let mut buf = Vec::with_capacity(12);
        buf.extend_from_slice(&AC_HANDSHAKE.to_le_bytes());
        buf.extend_from_slice(&1i32.to_le_bytes()); // identifier
        buf.extend_from_slice(&1i32.to_le_bytes()); // version

        let addr = format!("{}:{}", self.server_ip, self.server_port);
        socket.send_to(&buf, &addr)?;
        tracing::info!("Sent handshake to AC server at {}", addr);
        Ok(())
    }

    /// Subscribe to real-time updates
    fn subscribe_updates(&self) -> Result<()> {
        let socket = self.socket.as_ref().context("Not connected")?;

        // Subscribe packet: operation_id (i32 LE) + identifier (i32 LE) + update_interval (i32 LE)
        let mut buf = Vec::with_capacity(12);
        buf.extend_from_slice(&AC_SUBSCRIBE_UPDATE.to_le_bytes());
        buf.extend_from_slice(&1i32.to_le_bytes()); // identifier
        buf.extend_from_slice(&100i32.to_le_bytes()); // update interval ms

        let addr = format!("{}:{}", self.server_ip, self.server_port);
        socket.send_to(&buf, &addr)?;
        tracing::info!("Subscribed to AC telemetry updates");
        Ok(())
    }

    /// Parse a handshake response from AC server
    fn parse_handshake_response(&mut self, buf: &[u8]) -> Result<()> {
        if buf.len() < 408 {
            anyhow::bail!("Handshake response too short: {} bytes", buf.len());
        }

        // Parse car name (UTF-32LE, 100 bytes from offset 0)
        self.current_car = parse_utf32_string(&buf[0..200]);
        // Parse driver name (UTF-32LE, 100 bytes from offset 200)
        self.current_driver = parse_utf32_string(&buf[200..400]);
        // Parse track name (UTF-32LE, from offset 400)
        if buf.len() >= 608 {
            self.current_track = parse_utf32_string(&buf[400..600]);
        }

        tracing::info!(
            "AC handshake: driver={}, car={}, track={}",
            self.current_driver, self.current_car, self.current_track
        );
        Ok(())
    }

    /// Parse an update packet from AC server
    fn parse_update(&mut self, buf: &[u8]) -> Result<Option<TelemetryFrame>> {
        if buf.len() < 328 {
            return Ok(None);
        }

        // AC update packet structure (all little-endian):
        // Offset 0: identifier (char[4]) - skip
        // Offset 4: size (i32)
        // Offset 8: speed_kmh (f32)
        // ... many more fields

        let speed_kmh = f32::from_le_bytes(buf[8..12].try_into()?);
        let lap_time_ms = i32::from_le_bytes(buf[12..16].try_into()?) as u32;
        let lap_count = i32::from_le_bytes(buf[16..20].try_into()?) as u32;

        // More telemetry fields
        let throttle = if buf.len() > 24 {
            f32::from_le_bytes(buf[20..24].try_into().unwrap_or_default())
        } else {
            0.0
        };
        let brake = if buf.len() > 28 {
            f32::from_le_bytes(buf[24..28].try_into().unwrap_or_default())
        } else {
            0.0
        };
        let gear = if buf.len() > 32 {
            i32::from_le_bytes(buf[28..32].try_into().unwrap_or_default()) as i8
        } else {
            0
        };
        let rpm = if buf.len() > 36 {
            f32::from_le_bytes(buf[32..36].try_into().unwrap_or_default()) as u32
        } else {
            0
        };
        let steering = if buf.len() > 40 {
            f32::from_le_bytes(buf[36..40].try_into().unwrap_or_default())
        } else {
            0.0
        };

        let frame = TelemetryFrame {
            pod_id: self.pod_id.clone(),
            timestamp: Utc::now(),
            driver_name: self.current_driver.clone(),
            car: self.current_car.clone(),
            track: self.current_track.clone(),
            lap_number: lap_count,
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
        };

        // Track lap count for lap completion detection
        self.last_lap_count = lap_count;

        Ok(Some(frame))
    }
}

impl SimAdapter for AssettoCorsaAdapter {
    fn sim_type(&self) -> SimType {
        SimType::AssettocCorsa
    }

    fn connect(&mut self) -> Result<()> {
        let socket = UdpSocket::bind("0.0.0.0:0")?;
        socket.set_read_timeout(Some(Duration::from_millis(100)))?;
        socket.set_nonblocking(false)?;

        self.socket = Some(socket);
        self.send_handshake()?;

        // Wait for handshake response
        let mut buf = [0u8; 1024];
        let socket = self.socket.as_ref().unwrap();
        match socket.recv_from(&mut buf) {
            Ok((len, _)) => {
                self.parse_handshake_response(&buf[..len])?;
                self.connected = true;
                self.subscribe_updates()?;
                Ok(())
            }
            Err(e) => {
                anyhow::bail!("AC handshake timeout: {}", e);
            }
        }
    }

    fn is_connected(&self) -> bool {
        self.connected
    }

    fn read_telemetry(&mut self) -> Result<Option<TelemetryFrame>> {
        let socket = self.socket.as_ref().context("Not connected")?;
        let mut buf = [0u8; 1024];

        match socket.recv_from(&mut buf) {
            Ok((len, _)) => self.parse_update(&buf[..len]),
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => Ok(None),
            Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    fn poll_lap_completed(&mut self) -> Result<Option<LapData>> {
        // Lap completion is detected by lap_count incrementing
        // This is handled in the main agent loop by comparing frames
        Ok(None)
    }

    fn session_info(&self) -> Result<Option<SessionInfo>> {
        if !self.connected {
            return Ok(None);
        }
        Ok(Some(SessionInfo {
            id: String::new(),
            session_type: SessionType::Practice,
            sim_type: SimType::AssettocCorsa,
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
        if let Some(ref socket) = self.socket {
            // Send dismiss packet
            let mut buf = Vec::with_capacity(12);
            buf.extend_from_slice(&AC_DISMISS.to_le_bytes());
            buf.extend_from_slice(&1i32.to_le_bytes());
            buf.extend_from_slice(&0i32.to_le_bytes());
            let addr = format!("{}:{}", self.server_ip, self.server_port);
            let _ = socket.send_to(&buf, &addr);
        }
        self.socket = None;
        self.connected = false;
        tracing::info!("Disconnected from AC server");
    }
}

/// Parse a UTF-32LE encoded string from AC protocol
fn parse_utf32_string(buf: &[u8]) -> String {
    let mut chars = Vec::new();
    for chunk in buf.chunks(4) {
        if chunk.len() == 4 {
            let code = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
            if code == 0 {
                break;
            }
            if let Some(c) = char::from_u32(code) {
                chars.push(c);
            }
        }
    }
    chars.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_utf32_string() {
        // "AC" in UTF-32LE
        let buf = [0x41, 0x00, 0x00, 0x00, 0x43, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
        assert_eq!(parse_utf32_string(&buf), "AC");
    }

    #[test]
    fn test_parse_empty_string() {
        let buf = [0x00, 0x00, 0x00, 0x00];
        assert_eq!(parse_utf32_string(&buf), "");
    }
}
