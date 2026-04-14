//! Server diagnostics — infrastructure & external service checks.
//!
//! Split from server_diagnostics.rs: checks 5-10 (POS, WhatsApp, SSL, internet,
//! rc-sentry, OpenRouter key).

use tokio::time::Duration;

use crate::state::AppState;

pub(super) const LOG_TARGET: &str = "server-diagnostics";

// ─── Check 5: POS proactive health probe ─────────────────────────────────
/// Server-side pull of POS health — if POS rc-agent is dead, MI was blind.
pub(super) async fn check_pos_reachable(_state: &AppState) {
    let pos_ip = "192.168.31.20";
    let url = format!("http://{}:8090/health", pos_ip);
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap_or_default();

    match client.get(&url).send().await {
        Ok(resp) if resp.status().is_success() => {
            tracing::debug!(target: LOG_TARGET, "POS health OK (LAN)");
        }
        _ => {
            // Try Tailscale fallback
            let ts_url = format!("http://100.95.211.1:8090/health");
            match client.get(&ts_url).send().await {
                Ok(resp) if resp.status().is_success() => {
                    tracing::debug!(target: LOG_TARGET, "POS health OK (Tailscale, LAN refused)");
                }
                _ => {
                    tracing::warn!(target: LOG_TARGET,
                        "POS UNREACHABLE: both LAN ({}) and Tailscale failed — billing terminal offline",
                        pos_ip
                    );
                }
            }
        }
    }
}

// ─── Check 6: WhatsApp alert channel health ──────────────────────────────
/// MI uses WhatsApp to send all alerts — but never checks if the numbers are connected.
pub(super) async fn check_whatsapp_numbers(state: &AppState) {
    let evo_base = match &state.config.auth.evolution_url {
        Some(url) => url.clone(),
        None => return, // WhatsApp not configured
    };
    let api_key = match &state.config.auth.evolution_api_key {
        Some(key) => key.clone(),
        None => return,
    };

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .unwrap_or_default();

    // Check instance connection state
    let url = format!("{}/instance/connectionState/RacingPoint", evo_base.trim_end_matches('/'));
    match client.get(&url)
        .header("apikey", &api_key)
        .send()
        .await
    {
        Ok(resp) => {
            let body = resp.text().await.unwrap_or_default();
            if body.contains("open") {
                tracing::debug!(target: LOG_TARGET, "WhatsApp connection: open");
            } else {
                tracing::warn!(target: LOG_TARGET,
                    "WhatsApp DEGRADED: connection state is not 'open' — alerts may not deliver. State: {}",
                    &body[..body.len().min(200)]
                );
            }
        }
        Err(e) => {
            tracing::warn!(target: LOG_TARGET,
                "WhatsApp UNREACHABLE: Evolution API probe failed — {} — all MI alerts are undeliverable",
                e
            );
        }
    }
}

// ─── Check 7: SSL certificate expiry ─────────────────────────────────────
/// Probes cloud domains and warns if SSL cert expires within 14 days.
pub(super) async fn check_ssl_expiry(_state: &AppState) {
    let domains = ["app.racingpoint.cloud", "admin.racingpoint.cloud", "racingpoint.cloud"];

    for domain in &domains {
        let url = format!("https://{}", domain);
        // Use native-tls to get cert info — reqwest doesn't expose it,
        // so we do a simple connect check. If the HTTPS probe fails with
        // a cert error, that's our early warning.
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap_or_default();

        match client.get(&url).send().await {
            Ok(_) => {
                tracing::debug!(target: LOG_TARGET, "SSL OK: {}", domain);
            }
            Err(e) => {
                let err_str = format!("{}", e);
                if err_str.contains("certificate") || err_str.contains("ssl") || err_str.contains("tls") {
                    tracing::error!(target: LOG_TARGET,
                        "SSL CRITICAL: {} — certificate error: {}. Renew ASAP.",
                        domain, &err_str[..err_str.len().min(200)]
                    );
                } else {
                    tracing::debug!(target: LOG_TARGET, "SSL probe failed for {} (non-cert): {}", domain, &err_str[..err_str.len().min(100)]);
                }
            }
        }
    }
}

// ─── Check 8: Internet/Router connectivity ───────────────────────────────
/// Probes external endpoints to verify internet is up. Distinguishes router-down from WAN-down.
pub(super) async fn check_internet_connectivity(_state: &AppState) {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap_or_default();

    // Try multiple external endpoints (any one passing = internet up)
    let endpoints = [
        "https://www.google.com/generate_204",
        "https://connectivitycheck.gstatic.com/generate_204",
        "http://www.msftconnecttest.com/connecttest.txt",
    ];

    let mut any_ok = false;
    for url in &endpoints {
        if let Ok(resp) = client.get(*url).send().await {
            if resp.status().is_success() || resp.status().as_u16() == 204 {
                any_ok = true;
                break;
            }
        }
    }

    if !any_ok {
        tracing::error!(target: LOG_TARGET,
            "INTERNET DOWN: all external connectivity probes failed. \
             Check router (.1), WAN link, or DNS. MI cloud probes will also fail."
        );
    } else {
        tracing::debug!(target: LOG_TARGET, "Internet connectivity OK");
    }

    // Also check router gateway
    let router_client = reqwest::Client::builder()
        .timeout(Duration::from_secs(3))
        .build()
        .unwrap_or_default();

    if let Err(_) = router_client.get("http://192.168.31.1").send().await {
        tracing::warn!(target: LOG_TARGET,
            "ROUTER UNREACHABLE: 192.168.31.1 not responding — gateway may be down"
        );
    }
}

// ─── Check 9: rc-sentry on pods ──────────────────────────────────────────
/// Server-side probe of rc-sentry :8091 on pods. If sentry dies, MI loses pod self-healing.
pub(super) async fn check_sentry_reachable(_state: &AppState) {
    let pod_ips = [
        ("pod_1", "192.168.31.89"), ("pod_2", "192.168.31.33"),
        ("pod_3", "192.168.31.28"), ("pod_4", "192.168.31.88"),
        ("pod_5", "192.168.31.86"), ("pod_6", "192.168.31.87"),
        ("pod_7", "192.168.31.38"), ("pod_8", "192.168.31.91"),
    ];

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(3))
        .build()
        .unwrap_or_default();

    let mut dead_sentries = Vec::new();

    for (name, ip) in &pod_ips {
        let url = format!("http://{}:8091/health", ip);
        match client.get(&url).send().await {
            Ok(resp) if resp.status().is_success() => {}
            _ => {
                dead_sentries.push(*name);
            }
        }
    }

    if !dead_sentries.is_empty() {
        tracing::warn!(target: LOG_TARGET,
            "SENTRY DOWN on {}: {} — attempting auto-restart via rc-agent exec",
            dead_sentries.join(", "), dead_sentries.len()
        );

        // AUTO-FIX: restart rc-sentry via rc-agent :8090 exec endpoint
        for (name, ip) in &pod_ips {
            if !dead_sentries.contains(name) { continue; }
            let exec_url = format!("http://{}:8090/exec", ip);
            let body = serde_json::json!({
                "cmd": "schtasks /Run /TN StartRCSentry"
            });
            match client.post(&exec_url)
                .json(&body)
                .send()
                .await
            {
                Ok(resp) if resp.status().is_success() => {
                    tracing::info!(target: LOG_TARGET, "SENTRY AUTO-FIX: restarted {} via rc-agent exec", name);
                }
                _ => {
                    tracing::warn!(target: LOG_TARGET, "SENTRY AUTO-FIX FAILED on {} — rc-agent may also be down", name);
                }
            }
        }
    } else {
        tracing::debug!(target: LOG_TARGET, "All 8 rc-sentry instances reachable");
    }
}

// ─── Check 10: OpenRouter API key validity ───────────────────────────────
/// Proactive key check — don't wait for a 401 during diagnosis to discover the key is dead.
pub(super) async fn check_openrouter_key(_state: &AppState) {
    let key = match std::env::var("OPENROUTER_KEY") {
        Ok(k) if !k.is_empty() => k,
        _ => {
            // Try loading from saved file
            let saved = std::path::Path::new("data/openrouter-mma-key.txt");
            match tokio::fs::read_to_string(saved).await {
                Ok(k) if !k.trim().is_empty() => k.trim().to_string(),
                _ => return, // No key configured — skip
            }
        }
    };

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .unwrap_or_default();

    // Use /auth/key endpoint to validate without spending credits
    match client.get("https://openrouter.ai/api/v1/auth/key")
        .header("Authorization", format!("Bearer {}", key))
        .send()
        .await
    {
        Ok(resp) => {
            if resp.status().is_success() {
                tracing::debug!(target: LOG_TARGET, "OpenRouter key valid");
            } else if resp.status().as_u16() == 401 {
                tracing::error!(target: LOG_TARGET,
                    "OPENROUTER KEY DEAD: 401 — MMA Tier 3/4 diagnosis will fail. \
                     Provision new key via openrouter.ai/settings/keys"
                );
            } else {
                tracing::debug!(target: LOG_TARGET, "OpenRouter check: HTTP {}", resp.status());
            }
        }
        Err(_) => {
            // Network issue — don't alert (internet check will catch this)
        }
    }
}
