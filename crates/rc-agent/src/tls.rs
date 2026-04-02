//! Phase 305: TLS support for the rc-agent HTTP server (:8090).
//!
//! This module provides:
//! - `load_agent_tls_config()` — loads a `RustlsConfig` from the pod's cert/key pair
//! - `is_tailscale_ip()` / `socket_addr_is_tailscale()` — Tailscale bypass detection
//!
//! The bypass is architectural: the Tailscale relay listener in main.rs uses a separate
//! plain-HTTP bind address. This module's functions can be used to enforce the bypass
//! at the application layer if an mTLS-aware second listener is ever added.

use anyhow::{anyhow, Context as _};
use axum_server::tls_rustls::RustlsConfig;
use std::net::SocketAddr;

use crate::config::AgentTlsConfig;

/// Load a `RustlsConfig` for the rc-agent server using the pod's cert/key pair.
///
/// The pod presents this certificate to its callers (racecontrol server, POS terminal).
/// Certificate must be signed by the venue CA.
///
/// Returns an error if the cert or key files are missing or malformed.
pub async fn load_agent_tls_config(tls: &AgentTlsConfig) -> anyhow::Result<RustlsConfig> {
    let cert_path = &tls.server_cert_path;
    let key_path  = &tls.server_key_path;

    // Verify files exist before attempting to load
    if !std::path::Path::new(cert_path).exists() {
        return Err(anyhow!(
            "agent cert not found at '{}' — run scripts/generate-venue-ca.sh to create it",
            cert_path
        ));
    }
    if !std::path::Path::new(key_path).exists() {
        return Err(anyhow!(
            "agent key not found at '{}' — run scripts/generate-venue-ca.sh to create it",
            key_path
        ));
    }

    RustlsConfig::from_pem_file(cert_path, key_path)
        .await
        .with_context(|| {
            format!(
                "failed to load agent TLS config from cert='{}' key='{}'",
                cert_path, key_path
            )
        })
}

/// Returns `true` if the given IP string is in the Tailscale address range 100.64.0.0/10.
///
/// Tailscale uses CGNAT space (RFC 6598): 100.64.0.0 – 100.127.255.255.
/// Connections from these IPs are already protected by WireGuard encryption, so
/// an additional mTLS layer is redundant (and would require distributing the venue CA
/// to the Tailscale control plane). The bypass is architectural — this function is
/// provided for guard checks should a future listener need to enforce it.
pub fn is_tailscale_ip(ip: &str) -> bool {
    let parts: Vec<&str> = ip.split('.').collect();
    if parts.len() != 4 {
        return false;
    }
    let Ok(first)  = parts[0].parse::<u8>() else { return false; };
    let Ok(second) = parts[1].parse::<u8>() else { return false; };
    first == 100 && (64..=127).contains(&second)
}

/// Returns `true` if the `SocketAddr` remote IP is a Tailscale address.
pub fn socket_addr_is_tailscale(addr: &SocketAddr) -> bool {
    is_tailscale_ip(&addr.ip().to_string())
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AgentTlsConfig;

    // All 8 venue pod Tailscale IPs (from CLAUDE.md network map)
    #[test]
    fn tailscale_ip_all_venue_nodes() {
        let venue_ts_ips = [
            "100.92.122.89",   // Pod 1 sim1-1
            "100.105.93.108",  // Pod 2 sim2
            "100.69.231.26",   // Pod 3 sim3
            "100.75.45.10",    // Pod 4 sim4
            "100.110.133.87",  // Pod 5 sim5
            "100.127.149.17",  // Pod 6 sim6
            "100.82.196.28",   // Pod 7 sim7
            "100.98.67.67",    // Pod 8 sim8
            "100.125.108.37",  // Server (racing-point-server-1)
            "100.70.177.44",   // Bono VPS
            "100.95.211.1",    // POS terminal (pos1)
        ];
        for ip in &venue_ts_ips {
            assert!(is_tailscale_ip(ip), "expected Tailscale for {}", ip);
        }
    }

    #[test]
    fn tailscale_ip_range_boundaries() {
        assert!(is_tailscale_ip("100.64.0.1"),      "lower boundary");
        assert!(is_tailscale_ip("100.127.255.254"), "upper boundary");
    }

    #[test]
    fn tailscale_ip_false_cases() {
        assert!(!is_tailscale_ip("192.168.31.89"),   "pod 1 LAN");
        assert!(!is_tailscale_ip("192.168.31.23"),   "server LAN");
        assert!(!is_tailscale_ip("127.0.0.1"),       "localhost");
        assert!(!is_tailscale_ip("10.0.0.1"),        "private 10.x");
        assert!(!is_tailscale_ip("100.63.255.255"),  "below range");
        assert!(!is_tailscale_ip("100.128.0.1"),     "above range");
        assert!(!is_tailscale_ip(""),                "empty");
        assert!(!is_tailscale_ip("not-an-ip"),       "invalid");
    }

    #[test]
    fn tailscale_socket_addr_detection() {
        use std::net::{IpAddr, Ipv4Addr, SocketAddr};
        let ts  = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(100, 92, 122, 89)), 8090);
        let lan = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 31, 89)), 8090);
        assert!( socket_addr_is_tailscale(&ts));
        assert!(!socket_addr_is_tailscale(&lan));
    }

    #[test]
    fn agent_tls_config_defaults_are_safe() {
        let cfg = AgentTlsConfig::default();
        assert!(!cfg.enabled,         "disabled by default");
        assert!(cfg.tailscale_bypass, "tailscale bypass on by default");
    }

    #[tokio::test]
    async fn load_agent_tls_config_missing_files_returns_error() {
        let cfg = AgentTlsConfig {
            enabled: true,
            server_cert_path: "/nonexistent/pod.pem".to_string(),
            server_key_path:  "/nonexistent/pod-key.pem".to_string(),
            tailscale_bypass: true,
        };
        let result = load_agent_tls_config(&cfg).await;
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("not found") || msg.contains("agent cert"),
            "error should mention missing cert: {}",
            msg
        );
    }
}
