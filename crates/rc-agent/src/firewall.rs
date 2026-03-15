//! Firewall auto-configuration — ensures ICMP echo + TCP 8090 are open on every startup.
//! Runs synchronously before the HTTP server binds.
//!
//! Uses delete-then-add for idempotency: deleting a non-existent rule exits 0 on Windows 11.
//! Non-fatal: logs a warning and returns Failed if netsh lacks admin privileges.

use std::process::Command;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

const RULE_ICMP: &str = "RacingPoint-ICMP";
const RULE_TCP: &str = "RacingPoint-RemoteOps";

/// Result of firewall configuration attempt.
#[derive(Debug, PartialEq)]
pub enum FirewallResult {
    /// Both ICMP and TCP 8090 rules were applied successfully.
    Configured,
    /// One or more rules failed (likely not running as admin).
    Failed(String),
}

/// Configure firewall rules for ICMP echo and TCP 8090.
///
/// Idempotent: deletes existing rules by name before adding fresh ones.
/// Non-fatal: logs a warning and returns `Failed` on permission error — never panics.
///
/// NOTE: Requires administrator privileges on the calling process. On pods, rc-agent
/// runs via the admin-level HKLM Run key so this succeeds in normal operation.
/// Phase 19 (rc-watchdog SYSTEM service) may affect this — see open question in RESEARCH.md.
pub fn configure() -> FirewallResult {
    tracing::info!("[firewall] Applying firewall rules (profile=any)...");

    // Delete stale rules — exit 0 even if absent on Windows 11
    run_netsh(&build_delete_args(RULE_ICMP).iter().map(String::as_str).collect::<Vec<_>>());
    run_netsh(&build_delete_args(RULE_TCP).iter().map(String::as_str).collect::<Vec<_>>());

    // Add ICMP echo-request (ping), all profiles
    let icmp_ok =
        run_netsh(&build_icmp_args().iter().map(String::as_str).collect::<Vec<_>>());

    // Add TCP 8090 (remote ops), all profiles
    let tcp_ok =
        run_netsh(&build_tcp_args().iter().map(String::as_str).collect::<Vec<_>>());

    match (icmp_ok, tcp_ok) {
        (true, true) => {
            tracing::info!(
                "[firewall] Firewall configured — ICMP + TCP 8090 open (profile=any)"
            );
            FirewallResult::Configured
        }
        _ => {
            let msg =
                "netsh failed — agent may lack admin privileges. Port 8090 may be blocked."
                    .to_string();
            tracing::warn!("[firewall] {}", msg);
            FirewallResult::Failed(msg)
        }
    }
}

/// Build args for adding the ICMP echo-request rule.
fn build_icmp_args() -> Vec<String> {
    vec![
        "advfirewall".into(),
        "firewall".into(),
        "add".into(),
        "rule".into(),
        format!("name={}", RULE_ICMP),
        "protocol=icmpv4:8,any".into(),
        "dir=in".into(),
        "action=allow".into(),
        "profile=any".into(),
        "enable=yes".into(),
    ]
}

/// Build args for adding the TCP 8090 rule.
fn build_tcp_args() -> Vec<String> {
    vec![
        "advfirewall".into(),
        "firewall".into(),
        "add".into(),
        "rule".into(),
        format!("name={}", RULE_TCP),
        "protocol=TCP".into(),
        "localport=8090".into(),
        "dir=in".into(),
        "action=allow".into(),
        "profile=any".into(),
        "enable=yes".into(),
    ]
}

/// Build args for deleting a rule by name.
fn build_delete_args(name: &str) -> Vec<String> {
    vec![
        "advfirewall".into(),
        "firewall".into(),
        "delete".into(),
        "rule".into(),
        format!("name={}", name),
    ]
}

/// Execute a netsh command. Returns true if exit code is 0.
fn run_netsh(args: &[&str]) -> bool {
    let mut cmd = Command::new("netsh");
    cmd.args(args);
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    match cmd.output() {
        Ok(out) => {
            if !out.status.success() {
                let stderr = String::from_utf8_lossy(&out.stderr);
                tracing::warn!("[firewall] netsh {:?} failed: {}", args, stderr.trim());
            }
            out.status.success()
        }
        Err(e) => {
            tracing::warn!("[firewall] failed to spawn netsh: {}", e);
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rule_icmp_is_namespaced() {
        assert!(
            RULE_ICMP.starts_with("RacingPoint-"),
            "RULE_ICMP must start with 'RacingPoint-', got: {}",
            RULE_ICMP
        );
    }

    #[test]
    fn test_rule_tcp_is_namespaced() {
        assert!(
            RULE_TCP.starts_with("RacingPoint-"),
            "RULE_TCP must start with 'RacingPoint-', got: {}",
            RULE_TCP
        );
    }

    #[test]
    fn test_rule_names_are_distinct() {
        assert_ne!(RULE_ICMP, RULE_TCP, "RULE_ICMP and RULE_TCP must be different strings");
    }

    #[test]
    fn test_firewall_result_failed_is_not_configured() {
        let r = FirewallResult::Failed("x".into());
        assert_ne!(r, FirewallResult::Configured);
    }

    #[test]
    fn test_build_icmp_args_contains_required_fields() {
        let args = build_icmp_args();
        let joined = args.join(" ");
        assert!(
            args.iter().any(|a| a == "protocol=icmpv4:8,any"),
            "icmp args must contain protocol=icmpv4:8,any, got: {}",
            joined
        );
        assert!(
            args.iter().any(|a| a == "profile=any"),
            "icmp args must contain profile=any, got: {}",
            joined
        );
        assert!(
            args.iter().any(|a| a == "dir=in"),
            "icmp args must contain dir=in, got: {}",
            joined
        );
        assert!(
            args.iter().any(|a| a == "action=allow"),
            "icmp args must contain action=allow, got: {}",
            joined
        );
    }

    #[test]
    fn test_build_tcp_args_contains_required_fields() {
        let args = build_tcp_args();
        let joined = args.join(" ");
        assert!(
            args.iter().any(|a| a == "protocol=TCP"),
            "tcp args must contain protocol=TCP, got: {}",
            joined
        );
        assert!(
            args.iter().any(|a| a == "localport=8090"),
            "tcp args must contain localport=8090, got: {}",
            joined
        );
        assert!(
            args.iter().any(|a| a == "profile=any"),
            "tcp args must contain profile=any, got: {}",
            joined
        );
        assert!(
            args.iter().any(|a| a == "dir=in"),
            "tcp args must contain dir=in, got: {}",
            joined
        );
        assert!(
            args.iter().any(|a| a == "action=allow"),
            "tcp args must contain action=allow, got: {}",
            joined
        );
    }

    #[test]
    fn test_build_delete_args_contains_required_fields() {
        let name = "RacingPoint-TestRule";
        let args = build_delete_args(name);
        let joined = args.join(" ");
        assert!(
            args.iter().any(|a| a == "delete"),
            "delete args must contain 'delete', got: {}",
            joined
        );
        assert!(
            args.iter().any(|a| a == "rule"),
            "delete args must contain 'rule', got: {}",
            joined
        );
        assert!(
            args.iter().any(|a| a.contains(name)),
            "delete args must contain the rule name '{}', got: {}",
            name,
            joined
        );
    }
}
