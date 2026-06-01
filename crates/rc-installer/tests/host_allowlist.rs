//! INC-2H — pure server_address host-allowlist tests (MMA G5).

use rc_installer::host_allowlist::{extract_host, validate_server_address, HostPolicy};
use rc_installer::InstallError;

#[test]
fn rfc1918_allowed_under_both_policies() {
    for addr in ["192.168.31.23", "10.0.0.5", "172.16.4.9", "172.31.255.1"] {
        assert!(validate_server_address(addr, HostPolicy::LanOnly).is_ok(), "{addr}");
        assert!(
            validate_server_address(addr, HostPolicy::LanOrRacingpoint).is_ok(),
            "{addr}"
        );
    }
}

#[test]
fn loopback_allowed() {
    assert!(validate_server_address("127.0.0.1", HostPolicy::LanOnly).is_ok());
}

#[test]
fn public_ip_rejected_under_both_policies() {
    for addr in ["8.8.8.8", "203.0.113.5", "1.1.1.1"] {
        assert!(matches!(
            validate_server_address(addr, HostPolicy::LanOnly),
            Err(InstallError::HostNotAllowed(_))
        ));
        assert!(matches!(
            validate_server_address(addr, HostPolicy::LanOrRacingpoint),
            Err(InstallError::HostNotAllowed(_))
        ));
    }
}

#[test]
fn racingpoint_suffix_allowed_only_under_lanorrp() {
    assert!(validate_server_address("server.racingpoint.in", HostPolicy::LanOrRacingpoint).is_ok());
    assert!(validate_server_address("x.racecontrol.in", HostPolicy::LanOrRacingpoint).is_ok());
    // strict LanOnly rejects DNS names
    assert!(matches!(
        validate_server_address("server.racingpoint.in", HostPolicy::LanOnly),
        Err(InstallError::HostNotAllowed(_))
    ));
}

#[test]
fn lookalike_suffix_rejected() {
    // suffix-boundary check: these must NOT be treated as racingpoint.in
    for addr in ["racingpoint.in.evil.com", "evilracingpoint.in", "notracingpoint.cloud"] {
        assert!(
            matches!(
                validate_server_address(addr, HostPolicy::LanOrRacingpoint),
                Err(InstallError::HostNotAllowed(_))
            ),
            "{addr} should be rejected"
        );
    }
}

#[test]
fn apex_suffix_allowed() {
    assert!(validate_server_address("racingpoint.in", HostPolicy::LanOrRacingpoint).is_ok());
}

#[test]
fn strips_scheme_and_port() {
    assert_eq!(extract_host("https://192.168.31.23:8080/install").unwrap(), "192.168.31.23");
    assert!(validate_server_address("https://192.168.31.23:8080", HostPolicy::LanOnly).is_ok());
}

#[test]
fn non_ascii_host_rejected() {
    assert!(matches!(
        validate_server_address("café.racingpoint.in", HostPolicy::LanOrRacingpoint),
        Err(InstallError::HostNotAllowed(_))
    ));
}

#[test]
fn empty_or_garbage_rejected() {
    for addr in ["", "   ", ":::"] {
        assert!(matches!(
            validate_server_address(addr, HostPolicy::LanOrRacingpoint),
            Err(InstallError::HostNotAllowed(_))
        ));
    }
}

#[test]
fn pseudo_port_colon_bypass_rejected() {
    // regression (WF2 CRITICAL): a non-numeric pseudo-port must NOT smuggle an allowed
    // suffix past the digit-only port guard. "evil.com:.racingpoint.in" must be rejected.
    for addr in [
        "evil.com:.racingpoint.in",
        "attacker.net:.racecontrol.in",
        "8.8.8.8:.racingpoint.cloud",
    ] {
        assert!(
            matches!(
                validate_server_address(addr, HostPolicy::LanOrRacingpoint),
                Err(InstallError::HostNotAllowed(_))
            ),
            "{addr} must be rejected (pseudo-port colon bypass)"
        );
    }
}

#[test]
fn bono_vps_host_rejected_under_current_policies() {
    // DOCUMENTED KNOWN STATE (WF2): the real bono cloud-VPS host (`*.hstgr.cloud`) is NOT
    // in the server-address allowlist. These policies target `server_address` (pod ->
    // venue heart on the LAN); `bono_address` (cloud control-plane) needs its own
    // policy/suffix at INC-5 wire-time. Until then it is fail-closed here.
    assert!(matches!(
        validate_server_address("srv1422716.hstgr.cloud", HostPolicy::LanOrRacingpoint),
        Err(InstallError::HostNotAllowed(_))
    ));
}
