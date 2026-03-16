use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// A set of ports allocated for a single AC server session.
#[derive(Debug, Clone)]
pub struct AllocatedPorts {
    pub udp_port: u16,
    pub tcp_port: u16,
    pub http_port: u16,
}

/// Entry tracking a freed port set and when it was released (for TIME_WAIT cooldown).
struct CooldownEntry {
    ports: AllocatedPorts,
    freed_at: Instant,
}

/// Internal mutable state behind the RwLock.
struct PortAllocatorInner {
    allocated: HashMap<String, AllocatedPorts>,
    cooldown: Vec<CooldownEntry>,
}

/// Manages dynamic port allocation for AC dedicated server sessions.
///
/// Assigns unique (UDP, TCP, HTTP) port tuples from a fixed range so that
/// concurrent multiplayer sessions never collide. Freed ports enter a
/// 4-minute cooldown to respect the TCP TIME_WAIT window on Windows.
pub struct PortAllocator {
    base_udp: u16,
    base_http: u16,
    max_sessions: u16,
    cooldown_secs: u64,
    inner: RwLock<PortAllocatorInner>,
}

impl PortAllocator {
    /// Create a new allocator.
    ///
    /// * `base_udp`  – first UDP/TCP port in the range (e.g. 9600)
    /// * `base_http` – first HTTP port in the range (e.g. 8081)
    /// * `max_sessions` – number of slots (ports base..base+max_sessions-1)
    pub fn new(base_udp: u16, base_http: u16, max_sessions: u16) -> Self {
        Self {
            base_udp,
            base_http,
            max_sessions,
            cooldown_secs: 240, // 4 minutes — covers Windows TIME_WAIT
            inner: RwLock::new(PortAllocatorInner {
                allocated: HashMap::new(),
                cooldown: Vec::new(),
            }),
        }
    }

    /// Allocate a unique set of ports for `session_id`.
    ///
    /// Iterates through all slots, skipping those already allocated or still
    /// in cooldown, and verifies the port is actually free with a bind test.
    pub async fn allocate(&self, session_id: &str) -> anyhow::Result<AllocatedPorts> {
        let mut inner = self.inner.write().await;

        // Clean up expired cooldowns first
        let cutoff = Instant::now() - Duration::from_secs(self.cooldown_secs);
        inner.cooldown.retain(|entry| entry.freed_at > cutoff);

        for i in 0..self.max_sessions {
            let udp_port = self.base_udp + i;
            let tcp_port = self.base_udp + i; // AC uses same port for UDP and TCP
            let http_port = self.base_http + i;

            // Skip if already allocated to another session
            if inner.allocated.values().any(|p| p.udp_port == udp_port) {
                continue;
            }

            // Skip if in cooldown
            if inner.cooldown.iter().any(|e| e.ports.udp_port == udp_port) {
                continue;
            }

            // Verify port is actually free (bind test)
            if !is_port_free(udp_port) || !is_port_free(http_port) {
                tracing::debug!(
                    udp_port,
                    http_port,
                    "Port slot {} skipped — bind test failed",
                    i
                );
                continue;
            }

            let ports = AllocatedPorts {
                udp_port,
                tcp_port,
                http_port,
            };

            inner.allocated.insert(session_id.to_string(), ports.clone());

            tracing::info!(
                session_id,
                udp_port,
                tcp_port,
                http_port,
                "Allocated port slot {}",
                i
            );

            return Ok(ports);
        }

        anyhow::bail!(
            "No ports available — all {} slots are allocated or in cooldown",
            self.max_sessions
        );
    }

    /// Release ports for `session_id`, moving them into the cooldown window.
    pub async fn release(&self, session_id: &str) {
        let mut inner = self.inner.write().await;
        if let Some(ports) = inner.allocated.remove(session_id) {
            tracing::info!(
                session_id,
                udp_port = ports.udp_port,
                http_port = ports.http_port,
                "Released ports — entering 4-min cooldown"
            );
            inner.cooldown.push(CooldownEntry {
                ports,
                freed_at: Instant::now(),
            });
        }
    }

    /// Add ports to cooldown without them being in the allocated map.
    /// Used during orphan cleanup when we know the ports from the DB but
    /// they were never registered in memory (e.g. after a restart).
    pub async fn add_to_cooldown(&self, ports: AllocatedPorts) {
        let mut inner = self.inner.write().await;
        tracing::info!(
            udp_port = ports.udp_port,
            http_port = ports.http_port,
            "Adding orphaned ports to cooldown"
        );
        inner.cooldown.push(CooldownEntry {
            ports,
            freed_at: Instant::now(),
        });
    }

    /// Remove cooldown entries older than the cooldown duration.
    pub async fn cleanup_expired_cooldowns(&self) {
        let mut inner = self.inner.write().await;
        let cutoff = Instant::now() - Duration::from_secs(self.cooldown_secs);
        let before = inner.cooldown.len();
        inner.cooldown.retain(|entry| entry.freed_at > cutoff);
        let removed = before - inner.cooldown.len();
        if removed > 0 {
            tracing::debug!("Cleaned up {} expired port cooldown entries", removed);
        }
    }
}

/// Check if a TCP port is free by attempting to bind to it.
fn is_port_free(port: u16) -> bool {
    std::net::TcpListener::bind(("0.0.0.0", port)).is_ok()
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_allocate_unique_ports() {
        let alloc = PortAllocator::new(19600, 18081, 16);

        let p1 = alloc.allocate("session-1").await.unwrap();
        let p2 = alloc.allocate("session-2").await.unwrap();
        let p3 = alloc.allocate("session-3").await.unwrap();
        let p4 = alloc.allocate("session-4").await.unwrap();

        // All UDP ports must be unique
        let udp_ports = vec![p1.udp_port, p2.udp_port, p3.udp_port, p4.udp_port];
        let mut deduped = udp_ports.clone();
        deduped.sort();
        deduped.dedup();
        assert_eq!(udp_ports.len(), deduped.len(), "UDP ports must be unique");

        // All HTTP ports must be unique
        let http_ports = vec![p1.http_port, p2.http_port, p3.http_port, p4.http_port];
        let mut deduped = http_ports.clone();
        deduped.sort();
        deduped.dedup();
        assert_eq!(http_ports.len(), deduped.len(), "HTTP ports must be unique");

        // Ports must be in range
        for p in &udp_ports {
            assert!(*p >= 19600 && *p < 19616, "UDP port out of range: {}", p);
        }
        for p in &http_ports {
            assert!(*p >= 18081 && *p < 18097, "HTTP port out of range: {}", p);
        }
    }

    #[tokio::test]
    async fn test_release_enters_cooldown() {
        let alloc = PortAllocator::new(19700, 18181, 16);

        let p1 = alloc.allocate("session-1").await.unwrap();
        let freed_udp = p1.udp_port;

        alloc.release("session-1").await;

        // Allocate again — should get a DIFFERENT port because the freed one is in cooldown
        let p2 = alloc.allocate("session-2").await.unwrap();
        assert_ne!(
            p2.udp_port, freed_udp,
            "Freed port should be in cooldown and not immediately reusable"
        );
    }

    #[tokio::test]
    async fn test_exhaust_all_slots() {
        // Only 2 slots available
        let alloc = PortAllocator::new(19800, 18281, 2);

        let _p1 = alloc.allocate("s1").await.unwrap();
        let _p2 = alloc.allocate("s2").await.unwrap();

        // Third allocation should fail
        let result = alloc.allocate("s3").await;
        assert!(result.is_err(), "Should fail when all slots exhausted");
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("No ports available"),
            "Error should mention no ports: {}",
            err_msg
        );
    }

    #[tokio::test]
    async fn test_cooldown_prevents_reuse_after_release() {
        // Only 1 slot — after release, cooldown should block reallocation
        let alloc = PortAllocator::new(19900, 18381, 1);

        let _p1 = alloc.allocate("s1").await.unwrap();
        alloc.release("s1").await;

        // Should fail — only slot is in cooldown
        let result = alloc.allocate("s2").await;
        assert!(
            result.is_err(),
            "Should fail when only slot is in cooldown"
        );
    }

    #[tokio::test]
    async fn test_add_to_cooldown_blocks_allocation() {
        let alloc = PortAllocator::new(20000, 18481, 2);

        // Simulate orphaned ports being added to cooldown
        alloc
            .add_to_cooldown(AllocatedPorts {
                udp_port: 20000,
                tcp_port: 20000,
                http_port: 18481,
            })
            .await;

        // First allocation should skip slot 0 (in cooldown) and use slot 1
        let p1 = alloc.allocate("s1").await.unwrap();
        assert_eq!(p1.udp_port, 20001, "Should skip cooldown slot and use next");
        assert_eq!(p1.http_port, 18482);
    }
}
