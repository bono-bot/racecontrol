//! mDNS service advertiser for racecontrol server.
//!
//! Registers `_racecontrol._tcp.local.` on the LAN so rc-agent instances
//! can discover the server without hardcoded IPs in their TOML config.
//! TXT records carry `build_id` and `venue_id` for identification.

use std::collections::HashMap;
use mdns_sd::{ServiceDaemon, ServiceInfo};

const LOG_TARGET: &str = "mdns";
const SERVICE_TYPE: &str = "_racecontrol._tcp.local.";
const INSTANCE_NAME: &str = "RaceControl Server";

/// Start the mDNS advertiser as a background task.
/// Registers the service on the given port with build_id and venue_id TXT records.
/// Returns a handle that keeps the daemon alive — drop it to deregister.
pub fn start_advertiser(port: u16, build_id: &str, venue_id: &str) -> Option<ServiceDaemon> {
    let daemon = match ServiceDaemon::new() {
        Ok(d) => d,
        Err(e) => {
            tracing::warn!(target: LOG_TARGET, "Failed to create mDNS daemon: {}. Pods will use TOML config fallback.", e);
            return None;
        }
    };

    let mut properties = HashMap::new();
    properties.insert("build_id".to_string(), build_id.to_string());
    properties.insert("venue_id".to_string(), venue_id.to_string());
    properties.insert("version".to_string(), "1".to_string());

    // Get the machine hostname for mDNS registration.
    // Windows: COMPUTERNAME env var. Fallback: "racecontrol".
    let hostname = std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "racecontrol".to_string());
    let host_fqdn = format!("{}.local.", hostname);

    let service_info = match ServiceInfo::new(
        SERVICE_TYPE,
        INSTANCE_NAME,
        &host_fqdn,
        "",  // empty = auto-detect IPs
        port,
        properties,
    ) {
        Ok(info) => info,
        Err(e) => {
            tracing::warn!(target: LOG_TARGET, "Failed to create mDNS service info: {}. Pods will use TOML config fallback.", e);
            return None;
        }
    };

    match daemon.register(service_info) {
        Ok(_) => {
            tracing::info!(
                target: LOG_TARGET,
                "mDNS advertiser started: {} on port {} (venue={}, build={})",
                SERVICE_TYPE, port, venue_id, build_id
            );
        }
        Err(e) => {
            tracing::warn!(target: LOG_TARGET, "Failed to register mDNS service: {}. Pods will use TOML config fallback.", e);
            return None;
        }
    }

    Some(daemon)
}
