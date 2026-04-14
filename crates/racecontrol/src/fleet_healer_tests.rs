use super::*;

// ── Tailscale IP map ─────────────────────────────────────────────────

#[test]
fn tailscale_ip_returns_correct_ips_for_all_8_pods() {
    assert_eq!(tailscale_ip(1), Some("100.92.122.89"));
    assert_eq!(tailscale_ip(2), Some("100.105.93.108"));
    assert_eq!(tailscale_ip(3), Some("100.69.231.26"));
    assert_eq!(tailscale_ip(4), Some("100.75.45.10"));
    assert_eq!(tailscale_ip(5), Some("100.110.133.87"));
    assert_eq!(tailscale_ip(6), Some("100.127.149.17"));
    assert_eq!(tailscale_ip(7), Some("100.82.196.28"));
    assert_eq!(tailscale_ip(8), Some("100.98.67.67"));
}

#[test]
fn tailscale_ip_returns_none_for_invalid_pod() {
    assert_eq!(tailscale_ip(0), None);
    assert_eq!(tailscale_ip(9), None);
    assert_eq!(tailscale_ip(100), None);
}

// ── Diagnostic Fingerprinting ────────────────────────────────────────

#[test]
fn fingerprint_detects_missing_rcagent_in_tasklist() {
    let result = SshCommandResult {
        pod_id: "pod_1".into(),
        command: "tasklist /FO CSV /NH".into(),
        exit_code: Some(0),
        stdout: r#""svchost.exe","1234","Services","0","12,345 K"
"msedge.exe","5678","Console","1","98,765 K""#
            .into(),
        stderr: String::new(),
        duration_ms: 100,
        timestamp: Utc::now(),
    };

    let symptoms = DiagnosticFingerprinter::fingerprint(&[result]);
    assert!(
        symptoms.iter().any(|s| s.category == "process_missing" && s.detail == "rc-agent.exe"),
        "Should detect missing rc-agent.exe"
    );
}

#[test]
fn fingerprint_detects_missing_edge() {
    let result = SshCommandResult {
        pod_id: "pod_2".into(),
        command: "tasklist /FO CSV /NH".into(),
        exit_code: Some(0),
        stdout: r#""rc-agent.exe","1234","Console","1","50,000 K""#.into(),
        stderr: String::new(),
        duration_ms: 100,
        timestamp: Utc::now(),
    };

    let symptoms = DiagnosticFingerprinter::fingerprint(&[result]);
    assert!(
        symptoms.iter().any(|s| s.category == "process_missing" && s.detail == "msedge.exe"),
        "Should detect missing msedge.exe"
    );
}

#[test]
fn fingerprint_detects_port_not_listening() {
    let result = SshCommandResult {
        pod_id: "pod_3".into(),
        command: "netstat -an | findstr LISTEN".into(),
        exit_code: Some(0),
        stdout: "  TCP    0.0.0.0:445    0.0.0.0:0    LISTENING\n".into(),
        stderr: String::new(),
        duration_ms: 100,
        timestamp: Utc::now(),
    };

    let symptoms = DiagnosticFingerprinter::fingerprint(&[result]);
    assert!(
        symptoms.iter().any(|s| s.category == "port_not_listening" && s.detail.contains("8090")),
        "Should detect port 8090 not listening"
    );
}

#[test]
fn fingerprint_detects_maintenance_mode() {
    let result = SshCommandResult {
        pod_id: "pod_4".into(),
        command: "if exist ...".into(),
        exit_code: Some(0),
        stdout: "MAINTENANCE_MODE_PRESENT\n".into(),
        stderr: String::new(),
        duration_ms: 50,
        timestamp: Utc::now(),
    };

    let symptoms = DiagnosticFingerprinter::fingerprint(&[result]);
    assert!(
        symptoms.iter().any(|s| s.category == "sentinel"),
        "Should detect MAINTENANCE_MODE sentinel"
    );
}

#[test]
fn fingerprint_detects_session_zero() {
    let result = SshCommandResult {
        pod_id: "pod_5".into(),
        command: "powershell ... Get-Process rc-agent ...".into(),
        exit_code: Some(0),
        stdout: r#"{"Id":1234,"SessionId":0,"CPU":5.2,"WorkingSet64":52428800}"#.into(),
        stderr: String::new(),
        duration_ms: 200,
        timestamp: Utc::now(),
    };

    let symptoms = DiagnosticFingerprinter::fingerprint(&[result]);
    assert!(
        symptoms.iter().any(|s| s.category == "wrong_session"),
        "Should detect rc-agent in Session 0"
    );
}

#[test]
fn fingerprint_detects_event_log_crash() {
    let result = SshCommandResult {
        pod_id: "pod_6".into(),
        command: "wevtutil qe Application /c:20".into(),
        exit_code: Some(0),
        stdout: "  Faulting application name: rc-agent.exe, version: 0.0.0.0\n".into(),
        stderr: String::new(),
        duration_ms: 300,
        timestamp: Utc::now(),
    };

    let symptoms = DiagnosticFingerprinter::fingerprint(&[result]);
    assert!(
        symptoms.iter().any(|s| s.category == "app_crash"),
        "Should detect application crash in event log"
    );
}

// ── Fleet Pattern Detection ──────────────────────────────────────────

#[test]
fn fleet_pattern_triggers_on_three_pods() {
    let mut detector = FleetPatternDetector::new();
    let symptom = Symptom {
        category: "process_missing".into(),
        detail: "rc-agent.exe".into(),
        severity: "critical".into(),
    };

    assert!(detector.record_failure("pod_1", &symptom).is_none());
    assert!(detector.record_failure("pod_2", &symptom).is_none());
    let pattern = detector.record_failure("pod_3", &symptom);
    assert!(pattern.is_some(), "Should trigger pattern on 3rd pod");
    let p = pattern.unwrap();
    assert_eq!(p.affected_pods.len(), 3);
}

#[test]
fn fleet_pattern_deduplicates_same_pod() {
    let mut detector = FleetPatternDetector::new();
    let symptom = Symptom {
        category: "process_missing".into(),
        detail: "rc-agent.exe".into(),
        severity: "critical".into(),
    };

    assert!(detector.record_failure("pod_1", &symptom).is_none());
    assert!(detector.record_failure("pod_1", &symptom).is_none());
    assert!(detector.record_failure("pod_1", &symptom).is_none());
    // Same pod 3 times should NOT trigger — needs 3 different pods.
    assert!(detector.record_failure("pod_2", &symptom).is_none());
}

// ── Repair Confidence Gate ───────────────────────────────────────────

#[test]
fn confidence_gate_allows_high_confidence_deterministic() {
    assert!(RepairDispatcher::should_dispatch(0.9, "Deterministic"));
    assert!(RepairDispatcher::should_dispatch(0.8, "Config"));
    assert!(RepairDispatcher::should_dispatch(1.0, "Deterministic"));
}

#[test]
fn confidence_gate_blocks_low_confidence() {
    assert!(!RepairDispatcher::should_dispatch(0.79, "Deterministic"));
    assert!(!RepairDispatcher::should_dispatch(0.5, "Config"));
    assert!(!RepairDispatcher::should_dispatch(0.0, "Deterministic"));
}

#[test]
fn confidence_gate_blocks_non_deterministic_fix_types() {
    assert!(!RepairDispatcher::should_dispatch(0.95, "Restart"));
    assert!(!RepairDispatcher::should_dispatch(0.99, "CodeChange"));
    assert!(!RepairDispatcher::should_dispatch(1.0, "Manual"));
}

// ── Canary Rollout ───────────────────────────────────────────────────

#[test]
fn canary_rollout_order() {
    let waves = CanaryRollout::waves(&[1, 2, 3, 4, 5, 6, 7, 8]);
    assert_eq!(waves.len(), 3);
    assert_eq!(waves[0], vec![8]); // canary
    assert_eq!(waves[1], vec![1, 2, 3]); // wave 1
    assert_eq!(waves[2], vec![4, 5, 6, 7]); // wave 2
}

#[test]
fn canary_rollout_filters_targets() {
    let waves = CanaryRollout::waves(&[1, 8]);
    assert_eq!(waves.len(), 2);
    assert_eq!(waves[0], vec![8]);
    assert_eq!(waves[1], vec![1]);
}

#[test]
fn canary_rollout_no_canary_pod() {
    let waves = CanaryRollout::waves(&[1, 2, 3]);
    assert_eq!(waves.len(), 1);
    assert_eq!(waves[0], vec![1, 2, 3]);
}

// ── Survival Report Ingester ─────────────────────────────────────────

#[test]
fn ingester_detects_trouble() {
    let mut ingester = SurvivalReportIngester::new();
    let report = SurvivalReport {
        pod_id: "pod_1".into(),
        source_layer: "watchdog".into(),
        status: "degraded".into(),
        timestamp: Utc::now(),
        diagnostics: None,
        uptime_secs: Some(120),
        build_id: Some("abc123".into()),
    };
    assert!(ingester.ingest(report));
}

#[test]
fn ingester_healthy_is_not_trouble() {
    let mut ingester = SurvivalReportIngester::new();
    let report = SurvivalReport {
        pod_id: "pod_2".into(),
        source_layer: "watchdog".into(),
        status: "healthy".into(),
        timestamp: Utc::now(),
        diagnostics: None,
        uptime_secs: Some(3600),
        build_id: Some("def456".into()),
    };
    assert!(!ingester.ingest(report));
}
