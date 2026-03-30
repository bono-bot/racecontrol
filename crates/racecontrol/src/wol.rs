//! Wake-on-LAN and remote shutdown utilities for pod management.

use anyhow::{anyhow, Result};
use tokio::net::UdpSocket;

const WOL_PORT: u16 = 9;
const BROADCAST_ADDR: &str = "192.168.31.255";
const POD_AGENT_PORT: u16 = 8090;

/// Send a Wake-on-LAN magic packet to the given MAC address.
///
/// MAC can be in format "AA:BB:CC:DD:EE:FF" or "AA-BB-CC-DD-EE-FF".
///
/// SF-05: WoL runs on the server — it cannot read pod sentinel files directly.
/// The caller (pod_healer) checks `state.lease_manager.get_lease(&pod_id)` before
/// calling this function to avoid waking pods under active heal control (v31.0 Phase 267).
pub async fn send_wol(mac: &str) -> Result<()> {
    let mac_bytes = parse_mac(mac)?;

    // Magic packet: 6 bytes of 0xFF followed by MAC repeated 16 times
    let mut packet = [0u8; 102];
    packet[..6].fill(0xFF);
    for i in 0..16 {
        let offset = 6 + i * 6;
        packet[offset..offset + 6].copy_from_slice(&mac_bytes);
    }

    let socket = UdpSocket::bind("0.0.0.0:0").await?;
    socket.set_broadcast(true)?;
    socket
        .send_to(&packet, (BROADCAST_ADDR, WOL_PORT))
        .await?;

    tracing::info!("[wol] Magic packet sent to {} (broadcast {}:{})", mac, BROADCAST_ADDR, WOL_PORT);
    Ok(())
}

/// Send a shutdown command to a pod via its pod-agent HTTP endpoint.
pub async fn shutdown_pod(http_client: &reqwest::Client, ip: &str) -> Result<String> {
    let url = format!("http://{}:{}/exec", ip, POD_AGENT_PORT);
    let resp = http_client
        .post(&url)
        .json(&serde_json::json!({
            "cmd": "shutdown /s /f /t 0",
            "timeout_ms": 5000
        }))
        .timeout(std::time::Duration::from_secs(8))
        .send()
        .await?;

    let body: serde_json::Value = resp.json().await?;
    let stdout = body["stdout"].as_str().unwrap_or("");
    let stderr = body["stderr"].as_str().unwrap_or("");

    tracing::info!("[wol] Shutdown command sent to {}", ip);
    Ok(format!("{}{}", stdout, stderr))
}

/// Send a restart command to a pod via its pod-agent HTTP endpoint.
pub async fn restart_pod(http_client: &reqwest::Client, ip: &str) -> Result<String> {
    let url = format!("http://{}:{}/exec", ip, POD_AGENT_PORT);
    let resp = http_client
        .post(&url)
        .json(&serde_json::json!({
            "cmd": "shutdown /r /f /t 0",
            "timeout_ms": 5000
        }))
        .timeout(std::time::Duration::from_secs(8))
        .send()
        .await?;

    let body: serde_json::Value = resp.json().await?;
    let stdout = body["stdout"].as_str().unwrap_or("");
    let stderr = body["stderr"].as_str().unwrap_or("");

    tracing::info!("[wol] Restart command sent to {}", ip);
    Ok(format!("{}{}", stdout, stderr))
}

pub(crate) fn parse_mac(mac: &str) -> Result<[u8; 6]> {
    let parts: Vec<&str> = mac.split(|c| c == ':' || c == '-').collect();
    if parts.len() != 6 {
        return Err(anyhow!("Invalid MAC address: {}", mac));
    }

    let mut bytes = [0u8; 6];
    for (i, part) in parts.iter().enumerate() {
        bytes[i] = u8::from_str_radix(part, 16)
            .map_err(|_| anyhow!("Invalid hex in MAC: {}", part))?;
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_mac_colon_separated_returns_correct_bytes() {
        let result = parse_mac("AA:BB:CC:DD:EE:FF").unwrap();
        assert_eq!(result, [0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF]);
    }

    #[test]
    fn parse_mac_dash_separated_returns_correct_bytes() {
        let result = parse_mac("AA-BB-CC-DD-EE-FF").unwrap();
        assert_eq!(result, [0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF]);
    }

    #[test]
    fn parse_mac_too_few_parts_returns_err() {
        let result = parse_mac("AA:BB:CC");
        assert!(result.is_err(), "Expected error for too-short MAC");
    }

    #[test]
    fn parse_mac_invalid_hex_returns_err() {
        let result = parse_mac("GG:HH:II:JJ:KK:LL");
        assert!(result.is_err(), "Expected error for invalid hex digits");
    }

    #[test]
    fn parse_mac_empty_string_returns_err() {
        let result = parse_mac("");
        assert!(result.is_err(), "Expected error for empty MAC string");
    }

    #[test]
    fn parse_mac_lowercase_returns_correct_bytes() {
        let result = parse_mac("aa:bb:cc:dd:ee:ff").unwrap();
        assert_eq!(result, [0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF]);
    }
}
