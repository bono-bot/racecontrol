use super::*;
use std::sync::Mutex;

// SAFETY: These tests mutate environment variables which is inherently unsafe
// in multi-threaded contexts. ENV_MUTEX serializes all env-var tests within
// this process so parallel cargo test invocations don't race on set_var/remove_var.
pub(crate) static ENV_MUTEX: Mutex<()> = Mutex::new(());

macro_rules! with_env_lock {
    ($body:block) => {{
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        $body
    }};
}

#[test]
fn jwt_secret_from_env_var() {
    let _g = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    unsafe { std::env::set_var("RACECONTROL_JWT_SECRET", "env-secret-123"); }
    let result = resolve_jwt_secret("config-value");
    assert_eq!(result, "env-secret-123");
    unsafe { std::env::remove_var("RACECONTROL_JWT_SECRET"); }
}

#[test]
fn jwt_secret_from_config_when_no_env() {
    let _g = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    unsafe { std::env::remove_var("RACECONTROL_JWT_SECRET"); }
    let result = resolve_jwt_secret("my-custom-secret");
    assert_eq!(result, "my-custom-secret");
}

#[test]
fn jwt_secret_rejects_dangerous_default() {
    let _g = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    unsafe { std::env::remove_var("RACECONTROL_JWT_SECRET"); }
    let result = resolve_jwt_secret("racingpoint-jwt-change-me-in-production");
    assert_ne!(result, "racingpoint-jwt-change-me-in-production");
    assert_eq!(result.len(), 64); // 32 bytes * 2 hex chars
}

#[test]
fn jwt_secret_auto_generates_on_empty() {
    let _g = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    unsafe { std::env::remove_var("RACECONTROL_JWT_SECRET"); }
    let result = resolve_jwt_secret("");
    assert_eq!(result.len(), 64);
    // Verify it's valid hex
    assert!(result.chars().all(|c| c.is_ascii_hexdigit()));
}

#[test]
fn jwt_secret_auto_generate_is_random() {
    let _g = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    unsafe { std::env::remove_var("RACECONTROL_JWT_SECRET"); }
    let key1 = resolve_jwt_secret("");
    let key2 = resolve_jwt_secret("");
    assert_ne!(key1, key2, "Two auto-generated keys must differ");
}

#[test]
fn env_var_overrides_terminal_secret() {
    let _g = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    unsafe { std::env::set_var("RACECONTROL_TERMINAL_SECRET", "term-secret-abc"); }
    let toml_str = r#"
[venue]
name = "Test Venue"
[server]
[database]
"#;
    let mut config: Config = toml::from_str(toml_str).expect("parse");
    config.apply_env_overrides();
    assert_eq!(config.cloud.terminal_secret.as_deref(), Some("term-secret-abc"));
    unsafe { std::env::remove_var("RACECONTROL_TERMINAL_SECRET"); }
}

#[test]
fn env_var_overrides_relay_secret() {
    let _g = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    unsafe { std::env::set_var("RACECONTROL_RELAY_SECRET", "relay-secret-xyz"); }
    let toml_str = r#"
[venue]
name = "Test Venue"
[server]
[database]
"#;
    let mut config: Config = toml::from_str(toml_str).expect("parse");
    config.apply_env_overrides();
    assert_eq!(config.bono.relay_secret.as_deref(), Some("relay-secret-xyz"));
    unsafe { std::env::remove_var("RACECONTROL_RELAY_SECRET"); }
}

#[test]
fn env_var_overrides_evolution_api_key() {
    let _g = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    unsafe { std::env::set_var("RACECONTROL_EVOLUTION_API_KEY", "evo-key-123"); }
    let toml_str = r#"
[venue]
name = "Test Venue"
[server]
[database]
"#;
    let mut config: Config = toml::from_str(toml_str).expect("parse");
    config.apply_env_overrides();
    assert_eq!(config.auth.evolution_api_key.as_deref(), Some("evo-key-123"));
    unsafe { std::env::remove_var("RACECONTROL_EVOLUTION_API_KEY"); }
}

#[test]
fn env_var_overrides_gmail_secrets() {
    let _g = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    unsafe {
        std::env::set_var("RACECONTROL_GMAIL_CLIENT_SECRET", "gmail-cs");
        std::env::set_var("RACECONTROL_GMAIL_REFRESH_TOKEN", "gmail-rt");
    }
    let toml_str = r#"
[venue]
name = "Test Venue"
[server]
[database]
"#;
    let mut config: Config = toml::from_str(toml_str).expect("parse");
    config.apply_env_overrides();
    assert_eq!(config.gmail.client_secret.as_deref(), Some("gmail-cs"));
    assert_eq!(config.gmail.refresh_token.as_deref(), Some("gmail-rt"));
    unsafe {
        std::env::remove_var("RACECONTROL_GMAIL_CLIENT_SECRET");
        std::env::remove_var("RACECONTROL_GMAIL_REFRESH_TOKEN");
    }
}

#[test]
fn config_fallback_preserved_when_no_env_vars() {
    let _g = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    // Clear all secret env vars
    unsafe {
        std::env::remove_var("RACECONTROL_JWT_SECRET");
        std::env::remove_var("RACECONTROL_TERMINAL_SECRET");
        std::env::remove_var("RACECONTROL_RELAY_SECRET");
        std::env::remove_var("RACECONTROL_EVOLUTION_API_KEY");
        std::env::remove_var("RACECONTROL_GMAIL_CLIENT_SECRET");
        std::env::remove_var("RACECONTROL_GMAIL_REFRESH_TOKEN");
    }
    let toml_str = r#"
[venue]
name = "Test Venue"
[server]
[database]
[cloud]
terminal_secret = "from-config"
[bono]
relay_secret = "from-config-relay"
[auth]
jwt_secret = "custom-jwt-from-config"
evolution_api_key = "evo-from-config"
[gmail]
client_secret = "gmail-from-config"
refresh_token = "gmail-rt-from-config"
"#;
    let mut config: Config = toml::from_str(toml_str).expect("parse");
    config.apply_env_overrides();
    assert_eq!(config.auth.jwt_secret, "custom-jwt-from-config");
    assert_eq!(config.cloud.terminal_secret.as_deref(), Some("from-config"));
    assert_eq!(config.bono.relay_secret.as_deref(), Some("from-config-relay"));
    assert_eq!(config.auth.evolution_api_key.as_deref(), Some("evo-from-config"));
    assert_eq!(config.gmail.client_secret.as_deref(), Some("gmail-from-config"));
    assert_eq!(config.gmail.refresh_token.as_deref(), Some("gmail-rt-from-config"));
}

#[test]
fn server_config_tls_port_deserializes() {
    let toml_str = r#"
[venue]
name = "Test Venue"
[server]
tls_port = 8443
cert_path = "/tmp/cert.pem"
key_path = "/tmp/key.pem"
[database]
"#;
    let config: Config = toml::from_str(toml_str).expect("parse with tls_port");
    assert_eq!(config.server.tls_port, Some(8443));
    assert_eq!(config.server.cert_path.as_deref(), Some("/tmp/cert.pem"));
    assert_eq!(config.server.key_path.as_deref(), Some("/tmp/key.pem"));
}

#[test]
fn server_config_tls_port_defaults_to_none() {
    let toml_str = r#"
[venue]
name = "Test Venue"
[server]
[database]
"#;
    let config: Config = toml::from_str(toml_str).expect("parse without tls_port");
    assert!(config.server.tls_port.is_none());
    assert!(config.server.cert_path.is_none());
    assert!(config.server.key_path.is_none());
}

#[test]
fn watchdog_config_deserializes_with_defaults() {
    let toml_str = r#"
[venue]
name = "Test Venue"

[server]

[database]
"#;
    let config: Config = toml::from_str(toml_str).expect("should parse with defaults");
    assert!(config.watchdog.enabled);
    assert!(!config.watchdog.email_enabled);
    assert_eq!(config.watchdog.email_recipient, "usingh@racingpoint.in");
    assert_eq!(config.watchdog.email_script_path, "send_email.js");
    assert_eq!(config.watchdog.email_pod_cooldown_secs, 1800);
    assert_eq!(config.watchdog.email_venue_cooldown_secs, 300);
    assert!(config.watchdog.escalation_steps_secs.is_empty());
}

#[test]
fn watchdog_config_deserializes_with_explicit_email_values() {
    let toml_str = r#"
[venue]
name = "Test Venue"

[server]

[database]

[watchdog]
enabled = true
email_enabled = true
email_recipient = "ops@example.com"
email_script_path = "/opt/send.js"
email_pod_cooldown_secs = 3600
email_venue_cooldown_secs = 600
escalation_steps_secs = [10, 30, 60, 120]
"#;
    let config: Config = toml::from_str(toml_str).expect("should parse explicit values");
    assert!(config.watchdog.email_enabled);
    assert_eq!(config.watchdog.email_recipient, "ops@example.com");
    assert_eq!(config.watchdog.email_script_path, "/opt/send.js");
    assert_eq!(config.watchdog.email_pod_cooldown_secs, 3600);
    assert_eq!(config.watchdog.email_venue_cooldown_secs, 600);
    assert_eq!(config.watchdog.escalation_steps_secs, vec![10, 30, 60, 120]);
}

#[test]
fn bono_config_defaults() {
    let toml_str = r#"
[venue]
name = "Test Venue"

[server]

[database]
"#;
    let config: Config = toml::from_str(toml_str).expect("should parse with defaults");
    assert!(!config.bono.enabled);
    assert_eq!(config.bono.relay_port, 8099);
    assert!(config.bono.webhook_url.is_none());
    assert!(config.bono.tailscale_bind_ip.is_none());
    assert!(config.bono.relay_secret.is_none());
}

#[test]
fn bono_config_explicit() {
    let toml_str = r#"
[venue]
name = "Test Venue"

[server]

[database]

[bono]
enabled = true
webhook_url = "http://100.64.0.1/webhooks/racecontrol"
tailscale_bind_ip = "100.64.0.2"
relay_port = 8099
relay_secret = "super-secret"
"#;
    let config: Config = toml::from_str(toml_str).expect("should parse explicit bono values");
    assert!(config.bono.enabled);
    assert_eq!(config.bono.webhook_url.as_deref(), Some("http://100.64.0.1/webhooks/racecontrol"));
    assert_eq!(config.bono.tailscale_bind_ip.as_deref(), Some("100.64.0.2"));
    assert_eq!(config.bono.relay_port, 8099);
    assert_eq!(config.bono.relay_secret.as_deref(), Some("super-secret"));
}

// ─── ProcessGuardConfig Tests ─────────────────────────────────────────────

#[test]
fn process_guard_config_default_values() {
    let guard = ProcessGuardConfig::default();
    assert!(!guard.enabled);
    assert_eq!(guard.violation_action, "report_only");
    assert_eq!(guard.poll_interval_secs, 60);
    assert!(guard.warn_before_kill);
    assert!(guard.allowed.is_empty());
    assert!(guard.overrides.is_empty());
}

#[test]
fn process_guard_config_deserializes_from_toml() {
    let toml_str = r#"
[venue]
name = "Test Venue"
[server]
[database]

[process_guard]
enabled = true
violation_action = "report_only"
poll_interval_secs = 30

[[process_guard.allowed]]
name = "explorer.exe"
category = "system"
machines = ["all"]
"#;
    let config: Config = toml::from_str(toml_str).expect("should parse process_guard");
    assert!(config.process_guard.enabled);
    assert_eq!(config.process_guard.violation_action, "report_only");
    assert_eq!(config.process_guard.poll_interval_secs, 30);
    assert_eq!(config.process_guard.allowed.len(), 1);
    assert_eq!(config.process_guard.allowed[0].name, "explorer.exe");
}

#[test]
fn allowed_process_roundtrips() {
    let toml_str = r#"
[venue]
name = "Test Venue"
[server]
[database]

[[process_guard.allowed]]
name = "rc-agent.exe"
category = "racecontrol"
machines = ["pod"]
"#;
    let config: Config = toml::from_str(toml_str).expect("should parse allowed entry");
    assert_eq!(config.process_guard.allowed.len(), 1);
    let entry = &config.process_guard.allowed[0];
    assert_eq!(entry.name, "rc-agent.exe");
    assert_eq!(entry.category, "racecontrol");
    assert_eq!(entry.machines, vec!["pod"]);
}

#[test]
fn process_guard_override_deserializes() {
    let toml_str = r#"
[venue]
name = "Test Venue"
[server]
[database]

[process_guard.overrides.test_machine]
allow_extra_processes = ["cargo.exe", "rustc.exe"]
allow_extra_ports = [8080, 9999]
allow_extra_autostart = ["MyService"]
deny_processes = ["steam.exe"]
"#;
    let config: Config = toml::from_str(toml_str).expect("should parse override");
    let ovr = config.process_guard.overrides.get("test_machine")
        .expect("test_machine override should exist");
    assert_eq!(ovr.allow_extra_processes, vec!["cargo.exe", "rustc.exe"]);
    assert_eq!(ovr.allow_extra_ports, vec![8080, 9999]);
    assert_eq!(ovr.allow_extra_autostart, vec!["MyService"]);
    assert_eq!(ovr.deny_processes, vec!["steam.exe"]);
}

#[test]
fn process_guard_override_james_key() {
    let toml_str = r#"
[venue]
name = "Test Venue"
[server]
[database]

[process_guard.overrides.james]
allow_extra_processes = ["ollama.exe"]
"#;
    let config: Config = toml::from_str(toml_str).expect("should parse james override");
    let james = config.process_guard.overrides.get("james")
        .expect("james override should exist");
    assert!(james.allow_extra_processes.contains(&"ollama.exe".to_string()));
}

#[test]
fn config_without_process_guard_section_defaults() {
    let toml_str = r#"
[venue]
name = "Test Venue"
[server]
[database]
"#;
    let config: Config = toml::from_str(toml_str).expect("should parse without process_guard");
    assert!(!config.process_guard.enabled);
    assert_eq!(config.process_guard.violation_action, "report_only");
    assert_eq!(config.process_guard.poll_interval_secs, 60);
    assert!(config.process_guard.allowed.is_empty());
    assert!(config.process_guard.overrides.is_empty());
}

/// Validates the repo's racecontrol.toml parses without error and has a
/// non-empty process guard allowlist. Catches SSH banner corruption, BOM,
/// or missing required fields before they reach production.
#[test]
fn repo_toml_parses_and_has_allowlist() {
    let toml_path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../racecontrol.toml");
    let content = std::fs::read_to_string(toml_path)
        .expect("racecontrol.toml must exist at repo root");

    // Detect common corruption: SSH banners, BOM
    assert!(
        !content.starts_with("**"),
        "racecontrol.toml starts with '**' — likely SSH banner corruption"
    );
    assert!(
        !content.as_bytes().starts_with(&[0xEF, 0xBB, 0xBF]),
        "racecontrol.toml has UTF-8 BOM — TOML parsers reject this"
    );
    assert!(
        content.starts_with('['),
        "racecontrol.toml must start with a TOML section header, got: {:?}",
        &content[..content.len().min(40)]
    );

    let config: Config = toml::from_str(&content)
        .expect("racecontrol.toml must be valid TOML matching Config struct");

    assert!(
        !config.process_guard.allowed.is_empty(),
        "process_guard.allowed must not be empty — got 0 entries"
    );
    assert!(
        config.process_guard.allowed.len() >= 100,
        "process_guard.allowed has only {} entries — expected 100+, possible data loss",
        config.process_guard.allowed.len()
    );
}

/// GAP-2 FIX (MMA 5/5 consensus): Cross-crate TOML section compatibility.
/// Extracts all top-level [section] headers from racecontrol.toml and verifies
/// each one is a known field in the Config struct.
#[test]
fn repo_toml_sections_match_config_fields() {
    let toml_path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../racecontrol.toml");
    let content = std::fs::read_to_string(toml_path)
        .expect("racecontrol.toml must exist at repo root");

    let top_level_sections: Vec<&str> = content
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            trimmed.starts_with('[')
                && !trimmed.starts_with("[[")
                && !trimmed.contains('.')
        })
        .map(|line| line.trim().trim_start_matches('[').trim_end_matches(']').trim())
        .collect();

    // IMPORTANT: When adding a new field to Config, add it here too.
    let known_fields = [
        "venue", "server", "database", "cloud", "pods", "branding",
        "integrations", "ai_debugger", "ac_server", "auth", "watchdog",
        "bono", "gmail", "monitoring", "alerting", "process_guard",
        "cafe", "billing", "mma",
    ];

    for section in &top_level_sections {
        assert!(
            known_fields.contains(section),
            "racecontrol.toml has section [{}] which is NOT in Config struct. \
             Add `#[serde(default)] pub {}: {}Config` to Config.",
            section, section, section
        );
    }
}
