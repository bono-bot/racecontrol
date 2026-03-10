//! UDP Heartbeat Protocol — shared packet format between rc-agent and rc-core.
//!
//! Runs alongside WebSocket for fast liveness detection (6s vs 30-60s).
//! Fixed-size binary packets with zero allocations.
//!
//! Port: 9999 (does not conflict with game telemetry ports)
//! Interval: Agent sends ping every 2s, core responds with pong.
//! Timeout: 3 missed pings/pongs (6s) = dead.

/// Default UDP heartbeat port
pub const HEARTBEAT_PORT: u16 = 9999;

/// Ping interval in seconds
pub const PING_INTERVAL_SECS: u64 = 2;

/// Number of missed pings/pongs before declaring dead
pub const MISS_THRESHOLD: u32 = 3;

/// Dead timeout = PING_INTERVAL_SECS * MISS_THRESHOLD
pub const DEAD_TIMEOUT_SECS: u64 = PING_INTERVAL_SECS * MISS_THRESHOLD as u64;

/// Magic bytes: "RP" (0x52, 0x50) — reject stray packets
const MAGIC: [u8; 2] = [0x52, 0x50];

/// Packet type identifiers
const TYPE_PING: u8 = 0x01;
const TYPE_PONG: u8 = 0x02;

/// Ping packet: Agent → Core (12 bytes)
///
/// ```text
/// ┌──────────┬──────────┬──────────┬──────────────┬──────────────┐
/// │ magic(2) │ pod#(1)  │ type(1)  │ sequence(4)  │ status(4)    │
/// └──────────┴──────────┴──────────┴──────────────┴──────────────┘
/// ```
#[derive(Debug, Clone, Copy)]
pub struct HeartbeatPing {
    pub pod_number: u8,
    pub sequence: u32,
    pub status: PodStatusBits,
}

/// Pong packet: Core → Agent (16 bytes)
///
/// ```text
/// ┌──────────┬──────────┬──────────┬──────────────┬──────────────┬──────────────┐
/// │ magic(2) │ pod#(1)  │ type(1)  │ sequence(4)  │ server_ts(4) │ flags(4)     │
/// └──────────┴──────────┴──────────┴──────────────┴──────────────┴──────────────┘
/// ```
#[derive(Debug, Clone, Copy)]
pub struct HeartbeatPong {
    pub pod_number: u8,
    pub sequence: u32,
    pub server_timestamp: u32,
    pub flags: ServerFlags,
}

/// Pod status bitfield packed into 4 bytes (in ping)
///
/// ```text
/// bit 0:     ws_connected
/// bit 1:     game_running
/// bit 2:     driving_active
/// bit 3:     billing_active
/// bit 4-7:   game_id (0=none, 1=AC, 2=F1, 3=iRacing, 4=LMU, 5=Forza, 6=ACEvo)
/// bit 8-15:  cpu_percent (0-100)
/// bit 16-23: gpu_percent (0-100)
/// bit 24-31: reserved
/// ```
#[derive(Debug, Clone, Copy, Default)]
pub struct PodStatusBits(pub u32);

impl PodStatusBits {
    pub fn new() -> Self {
        Self(0)
    }

    pub fn ws_connected(&self) -> bool {
        self.0 & 1 != 0
    }
    pub fn set_ws_connected(&mut self, v: bool) {
        if v { self.0 |= 1; } else { self.0 &= !1; }
    }

    pub fn game_running(&self) -> bool {
        self.0 & (1 << 1) != 0
    }
    pub fn set_game_running(&mut self, v: bool) {
        if v { self.0 |= 1 << 1; } else { self.0 &= !(1 << 1); }
    }

    pub fn driving_active(&self) -> bool {
        self.0 & (1 << 2) != 0
    }
    pub fn set_driving_active(&mut self, v: bool) {
        if v { self.0 |= 1 << 2; } else { self.0 &= !(1 << 2); }
    }

    pub fn billing_active(&self) -> bool {
        self.0 & (1 << 3) != 0
    }
    pub fn set_billing_active(&mut self, v: bool) {
        if v { self.0 |= 1 << 3; } else { self.0 &= !(1 << 3); }
    }

    pub fn game_id(&self) -> u8 {
        ((self.0 >> 4) & 0x0F) as u8
    }
    pub fn set_game_id(&mut self, id: u8) {
        self.0 = (self.0 & !(0x0F << 4)) | ((id as u32 & 0x0F) << 4);
    }

    pub fn cpu_percent(&self) -> u8 {
        ((self.0 >> 8) & 0xFF) as u8
    }
    pub fn set_cpu_percent(&mut self, pct: u8) {
        self.0 = (self.0 & !(0xFF << 8)) | ((pct as u32) << 8);
    }

    pub fn gpu_percent(&self) -> u8 {
        ((self.0 >> 16) & 0xFF) as u8
    }
    pub fn set_gpu_percent(&mut self, pct: u8) {
        self.0 = (self.0 & !(0xFF << 16)) | ((pct as u32) << 16);
    }
}

/// Server flags bitfield packed into 4 bytes (in pong)
///
/// ```text
/// bit 0: ws_expected     — core expects WebSocket to be connected
/// bit 1: force_reconnect — agent should drop + reconnect WebSocket
/// bit 2: force_restart   — agent should restart itself
/// ```
#[derive(Debug, Clone, Copy, Default)]
pub struct ServerFlags(pub u32);

impl ServerFlags {
    pub fn new() -> Self {
        Self(0)
    }

    pub fn ws_expected(&self) -> bool {
        self.0 & 1 != 0
    }
    pub fn set_ws_expected(&mut self, v: bool) {
        if v { self.0 |= 1; } else { self.0 &= !1; }
    }

    pub fn force_reconnect(&self) -> bool {
        self.0 & (1 << 1) != 0
    }
    pub fn set_force_reconnect(&mut self, v: bool) {
        if v { self.0 |= 1 << 1; } else { self.0 &= !(1 << 1); }
    }

    pub fn force_restart(&self) -> bool {
        self.0 & (1 << 2) != 0
    }
    pub fn set_force_restart(&mut self, v: bool) {
        if v { self.0 |= 1 << 2; } else { self.0 &= !(1 << 2); }
    }
}

// ─── Serialization ──────────────────────────────────────────────────────────

impl HeartbeatPing {
    pub const SIZE: usize = 12;

    /// Serialize to fixed 12-byte buffer
    pub fn to_bytes(&self) -> [u8; Self::SIZE] {
        let mut buf = [0u8; Self::SIZE];
        buf[0..2].copy_from_slice(&MAGIC);
        buf[2] = self.pod_number;
        buf[3] = TYPE_PING;
        buf[4..8].copy_from_slice(&self.sequence.to_le_bytes());
        buf[8..12].copy_from_slice(&self.status.0.to_le_bytes());
        buf
    }

    /// Deserialize from bytes. Returns None if magic/type mismatch or wrong size.
    pub fn from_bytes(buf: &[u8]) -> Option<Self> {
        if buf.len() < Self::SIZE {
            return None;
        }
        if buf[0..2] != MAGIC || buf[3] != TYPE_PING {
            return None;
        }
        Some(Self {
            pod_number: buf[2],
            sequence: u32::from_le_bytes([buf[4], buf[5], buf[6], buf[7]]),
            status: PodStatusBits(u32::from_le_bytes([buf[8], buf[9], buf[10], buf[11]])),
        })
    }
}

impl HeartbeatPong {
    pub const SIZE: usize = 16;

    /// Serialize to fixed 16-byte buffer
    pub fn to_bytes(&self) -> [u8; Self::SIZE] {
        let mut buf = [0u8; Self::SIZE];
        buf[0..2].copy_from_slice(&MAGIC);
        buf[2] = self.pod_number;
        buf[3] = TYPE_PONG;
        buf[4..8].copy_from_slice(&self.sequence.to_le_bytes());
        buf[8..12].copy_from_slice(&self.server_timestamp.to_le_bytes());
        buf[12..16].copy_from_slice(&self.flags.0.to_le_bytes());
        buf
    }

    /// Deserialize from bytes. Returns None if magic/type mismatch or wrong size.
    pub fn from_bytes(buf: &[u8]) -> Option<Self> {
        if buf.len() < Self::SIZE {
            return None;
        }
        if buf[0..2] != MAGIC || buf[3] != TYPE_PONG {
            return None;
        }
        Some(Self {
            pod_number: buf[2],
            sequence: u32::from_le_bytes([buf[4], buf[5], buf[6], buf[7]]),
            server_timestamp: u32::from_le_bytes([buf[8], buf[9], buf[10], buf[11]]),
            flags: ServerFlags(u32::from_le_bytes([buf[12], buf[13], buf[14], buf[15]])),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ping_roundtrip() {
        let mut status = PodStatusBits::new();
        status.set_ws_connected(true);
        status.set_game_running(true);
        status.set_game_id(1); // AC
        status.set_cpu_percent(45);
        status.set_gpu_percent(80);

        let ping = HeartbeatPing {
            pod_number: 8,
            sequence: 42,
            status,
        };

        let bytes = ping.to_bytes();
        assert_eq!(bytes.len(), 12);
        assert_eq!(&bytes[0..2], &MAGIC);

        let parsed = HeartbeatPing::from_bytes(&bytes).unwrap();
        assert_eq!(parsed.pod_number, 8);
        assert_eq!(parsed.sequence, 42);
        assert!(parsed.status.ws_connected());
        assert!(parsed.status.game_running());
        assert!(!parsed.status.driving_active());
        assert_eq!(parsed.status.game_id(), 1);
        assert_eq!(parsed.status.cpu_percent(), 45);
        assert_eq!(parsed.status.gpu_percent(), 80);
    }

    #[test]
    fn pong_roundtrip() {
        let mut flags = ServerFlags::new();
        flags.set_ws_expected(true);
        flags.set_force_reconnect(true);

        let pong = HeartbeatPong {
            pod_number: 3,
            sequence: 100,
            server_timestamp: 1709856000,
            flags,
        };

        let bytes = pong.to_bytes();
        assert_eq!(bytes.len(), 16);

        let parsed = HeartbeatPong::from_bytes(&bytes).unwrap();
        assert_eq!(parsed.pod_number, 3);
        assert_eq!(parsed.sequence, 100);
        assert_eq!(parsed.server_timestamp, 1709856000);
        assert!(parsed.flags.ws_expected());
        assert!(parsed.flags.force_reconnect());
        assert!(!parsed.flags.force_restart());
    }

    #[test]
    fn rejects_bad_magic() {
        let mut bytes = HeartbeatPing {
            pod_number: 1,
            sequence: 0,
            status: PodStatusBits::new(),
        }.to_bytes();
        bytes[0] = 0xFF; // corrupt magic
        assert!(HeartbeatPing::from_bytes(&bytes).is_none());
    }

    #[test]
    fn rejects_wrong_type() {
        let bytes = HeartbeatPing {
            pod_number: 1,
            sequence: 0,
            status: PodStatusBits::new(),
        }.to_bytes();
        // Try parsing ping bytes as pong — should fail (type mismatch)
        assert!(HeartbeatPong::from_bytes(&bytes).is_none());
    }

    #[test]
    fn rejects_short_buffer() {
        assert!(HeartbeatPing::from_bytes(&[0x52, 0x50]).is_none());
        assert!(HeartbeatPong::from_bytes(&[]).is_none());
    }

    #[test]
    fn status_bits_all_fields() {
        let mut s = PodStatusBits::new();
        assert_eq!(s.0, 0);

        s.set_ws_connected(true);
        s.set_driving_active(true);
        s.set_billing_active(true);
        s.set_game_id(5); // Forza
        s.set_cpu_percent(99);
        s.set_gpu_percent(100);

        assert!(s.ws_connected());
        assert!(!s.game_running());
        assert!(s.driving_active());
        assert!(s.billing_active());
        assert_eq!(s.game_id(), 5);
        assert_eq!(s.cpu_percent(), 99);
        assert_eq!(s.gpu_percent(), 100);

        // Toggle off
        s.set_ws_connected(false);
        assert!(!s.ws_connected());
        assert!(s.driving_active()); // unchanged
    }
}
