//! mDNS discovery for rc-agent.
//!
//! On startup, browses for `_racecontrol._tcp.local.` to find the racecontrol
//! server without needing a hardcoded IP in rc-agent.toml.
//! Falls back to the TOML `core.url` if mDNS discovery fails or times out.

use std::time::Duration;
use mdns_sd::{ServiceDaemon, ServiceEvent};

const LOG_TARGET: &str = "mdns-discovery";
const SERVICE_TYPE: &str = "_racecontrol._tcp.local.";
const BROWSE_TIMEOUT: Duration = Duration::from_secs(5);

/// Discover the racecontrol server via mDNS.
/// Returns a WebSocket URL like `ws://192.168.31.23:8080/ws/agent` if found.
/// Returns `None` after BROWSE_TIMEOUT if no server responds.
pub fn discover_server() -> Option<String> {
    tracing::info!(target: LOG_TARGET, "Browsing for {} (timeout: {}s)...", SERVICE_TYPE, BROWSE_TIMEOUT.as_secs());

    let daemon = match ServiceDaemon::new() {
        Ok(d) => d,
        Err(e) => {
            tracing::warn!(target: LOG_TARGET, "Failed to create mDNS browser: {}. Using TOML config.", e);
            return None;
        }
    };

    let receiver = match daemon.browse(SERVICE_TYPE) {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(target: LOG_TARGET, "Failed to browse for {}: {}. Using TOML config.", SERVICE_TYPE, e);
            let _ = daemon.shutdown();
            return None;
        }
    };

    let deadline = std::time::Instant::now() + BROWSE_TIMEOUT;

    loop {
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        if remaining.is_zero() {
            tracing::info!(target: LOG_TARGET, "mDNS browse timed out after {}s — no server found. Using TOML config.", BROWSE_TIMEOUT.as_secs());
            let _ = daemon.shutdown();
            return None;
        }

        match receiver.recv_timeout(remaining) {
            Ok(ServiceEvent::ServiceResolved(info)) => {
                let port = info.get_port();
                let addresses = info.get_addresses();

                // Prefer IPv4 addresses
                let ip = addresses.iter()
                    .find(|addr| addr.is_ipv4())
                    .or_else(|| addresses.iter().next());

                if let Some(addr) = ip {
                    let url = format!("ws://{}:{}/ws/agent", addr, port);

                    // Log TXT records for diagnostics
                    let build_id = info.get_property_val_str("build_id").unwrap_or("unknown");
                    let venue_id = info.get_property_val_str("venue_id").unwrap_or("unknown");

                    tracing::info!(
                        target: LOG_TARGET,
                        "Discovered racecontrol server via mDNS: {} (venue={}, build={})",
                        url, venue_id, build_id
                    );

                    let _ = daemon.shutdown();
                    return Some(url);
                }
            }
            Ok(_) => {
                // Other events (SearchStarted, ServiceFound, etc.) — continue browsing
                continue;
            }
            Err(_) => {
                // Timeout or channel disconnected — no server found
                tracing::info!(target: LOG_TARGET, "mDNS browse timed out — no server found. Using TOML config.");
                let _ = daemon.shutdown();
                return None;
            }
        }
    }
}
