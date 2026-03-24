//! Self-test module for rc-agent fleet health diagnostics.
//!
//! Runs 18 deterministic probes covering WebSocket connectivity, TCP/UDP ports,
//! HID wheelbase detection, Ollama availability, CLOSE_WAIT sockets, running
//! processes, disk/memory resources, GPU temperature, and Steam.
//!
//! Probes are run concurrently with a 10-second per-probe timeout. Results are
//! aggregated into a SelfTestReport and optionally enriched with an LLM verdict
//! (HEALTHY/DEGRADED/CRITICAL) from local Ollama. Falls back to deterministic
//! verdict if Ollama is unavailable.

use std::sync::atomic::Ordering;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::task::spawn_blocking;
use tokio::time::timeout;

use crate::udp_heartbeat::HeartbeatStatus;

// ─── Public Types ────────────────────────────────────────────────────────────

/// Result of a single diagnostic probe.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProbeResult {
    pub name: String,
    pub status: ProbeStatus,
    pub detail: String,
}

/// Pass/Fail/Skip status for a probe.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ProbeStatus {
    Pass,
    Fail,
    Skip,
}

/// Aggregated report from all probes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelfTestReport {
    pub probes: Vec<ProbeResult>,
    pub verdict: Option<SelfTestVerdict>,
    pub timestamp: String,
}

/// LLM-generated or deterministic verdict for the fleet health.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelfTestVerdict {
    pub level: VerdictLevel,
    pub analysis: String,
    pub auto_fix_recommendations: Vec<String>,
}

/// Health level classification.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum VerdictLevel {
    Healthy,
    Degraded,
    Critical,
}

const LOG_TARGET: &str = "self-test";

// ─── Shared HTTP client for Ollama probes ────────────────────────────────────

#[cfg(feature = "ai-debugger")]
static SELF_TEST_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

#[cfg(feature = "ai-debugger")]
fn self_test_client() -> &'static reqwest::Client {
    SELF_TEST_CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .expect("self_test HTTP client build failed")
    })
}

// ─── Individual Probe Functions ───────────────────────────────────────────────

/// Probe 1: WebSocket connected to racecontrol.
async fn probe_ws_connected(status: &Arc<HeartbeatStatus>) -> ProbeResult {
    let connected = status.ws_connected.load(Ordering::Relaxed);
    ProbeResult {
        name: "ws_connected".to_string(),
        status: if connected { ProbeStatus::Pass } else { ProbeStatus::Fail },
        detail: format!("ws_connected={}", connected),
    }
}

/// Probe 2–5: TCP port reachability (lock_screen, remote_ops, overlay, debug_server).
async fn probe_tcp_port(name: &str, addr: &str) -> ProbeResult {
    let addr = addr.to_string();
    let addr2 = addr.clone();
    let name = name.to_string();
    let result = spawn_blocking(move || {
        std::net::TcpStream::connect_timeout(
            &addr.parse().unwrap(),
            Duration::from_secs(1),
        )
    })
    .await;

    match result {
        Ok(Ok(_)) => ProbeResult {
            name,
            status: ProbeStatus::Pass,
            detail: format!("port {} reachable", addr2),
        },
        Ok(Err(e)) => ProbeResult {
            name,
            status: ProbeStatus::Fail,
            detail: format!("port {} unreachable: {}", addr2, e),
        },
        Err(e) => ProbeResult {
            name,
            status: ProbeStatus::Fail,
            detail: format!("spawn_blocking error: {}", e),
        },
    }
}

/// Parse netstat output and determine if a UDP port is bound.
/// Extracted for unit testability.
pub fn probe_udp_port_from_netstat_output(port: u16, netstat_stdout: &str) -> ProbeStatus {
    let target = format!(":{} ", port);
    let target_eol = format!(":{}\r", port);
    let target_eol2 = format!(":{}\n", port);
    let found = netstat_stdout.lines().any(|line| {
        line.contains("UDP")
            && (line.contains(&target)
                || line.contains(&target_eol)
                || line.contains(&target_eol2))
    });
    if found {
        ProbeStatus::Pass
    } else {
        ProbeStatus::Fail
    }
}

/// Probe 6–10: UDP telemetry port bound (AC=9996, F1=20777, Forza=5300, iRacing=6789, LMU=5555).
async fn probe_udp_port(port: u16) -> ProbeResult {
    let result = spawn_blocking(move || {
        std::process::Command::new("netstat")
            .args(["-ano"])
            .output()
    })
    .await;

    let game_name = match port {
        9996 => "AC",
        20777 => "F1",
        5300 => "Forza",
        6789 => "iRacing",
        5555 => "LMU",
        _ => "unknown",
    };
    let name = format!("udp_port_{}", game_name);

    match result {
        Ok(Ok(output)) => {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let status = probe_udp_port_from_netstat_output(port, &stdout);
            let detail = match &status {
                ProbeStatus::Pass => format!("UDP :{} bound", port),
                _ => format!("UDP :{} not found in netstat output", port),
            };
            ProbeResult { name, status, detail }
        }
        Ok(Err(e)) => ProbeResult {
            name,
            status: ProbeStatus::Fail,
            detail: format!("netstat failed: {}", e),
        },
        Err(e) => ProbeResult {
            name,
            status: ProbeStatus::Fail,
            detail: format!("spawn_blocking error: {}", e),
        },
    }
}

/// Probe 11: OpenFFBoard HID device detected (enumerate only — do NOT open).
async fn probe_hid() -> ProbeResult {
    let result = spawn_blocking(|| {
        let api = hidapi::HidApi::new()?;
        let found = api
            .device_list()
            .any(|d| d.vendor_id() == 0x1209 && d.product_id() == 0xFFB0);
        Ok::<bool, hidapi::HidError>(found)
    })
    .await;

    match result {
        Ok(Ok(true)) => ProbeResult {
            name: "hid_wheelbase".to_string(),
            status: ProbeStatus::Pass,
            detail: "OpenFFBoard VID:0x1209 PID:0xFFB0 detected".to_string(),
        },
        Ok(Ok(false)) => ProbeResult {
            name: "hid_wheelbase".to_string(),
            status: ProbeStatus::Fail,
            detail: "OpenFFBoard VID:0x1209 PID:0xFFB0 not found in HID device list".to_string(),
        },
        Ok(Err(e)) => ProbeResult {
            name: "hid_wheelbase".to_string(),
            status: ProbeStatus::Fail,
            detail: format!("HidApi error: {}", e),
        },
        Err(e) => ProbeResult {
            name: "hid_wheelbase".to_string(),
            status: ProbeStatus::Fail,
            detail: format!("spawn_blocking error: {}", e),
        },
    }
}

/// Probe 12: Ollama reachable and rp-debug model available.
#[cfg(feature = "ai-debugger")]
async fn probe_ollama(ollama_url: &str) -> ProbeResult {
    let url = format!("{}/api/tags", ollama_url);
    match self_test_client().get(&url).send().await {
        Ok(resp) if resp.status().is_success() => {
            match resp.text().await {
                Ok(body) if body.contains("rp-debug") || body.contains("rc-bot") => ProbeResult {
                    name: "ollama".to_string(),
                    status: ProbeStatus::Pass,
                    detail: "Ollama reachable and rp-debug/rc-bot model found".to_string(),
                },
                Ok(_) => ProbeResult {
                    name: "ollama".to_string(),
                    status: ProbeStatus::Fail,
                    detail: "Ollama reachable but rp-debug/rc-bot model not found in tag list".to_string(),
                },
                Err(e) => ProbeResult {
                    name: "ollama".to_string(),
                    status: ProbeStatus::Fail,
                    detail: format!("Ollama response parse error: {}", e),
                },
            }
        }
        Ok(resp) => ProbeResult {
            name: "ollama".to_string(),
            status: ProbeStatus::Fail,
            detail: format!("Ollama returned HTTP {}", resp.status()),
        },
        Err(e) => ProbeResult {
            name: "ollama".to_string(),
            status: ProbeStatus::Fail,
            detail: format!("Ollama unreachable: {}", e),
        },
    }
}

/// Probe 13: CLOSE_WAIT socket count on :8090 (< 20 = pass).
async fn probe_close_wait() -> ProbeResult {
    let result = spawn_blocking(|| {
        std::process::Command::new("netstat")
            .args(["-ano"])
            .output()
    })
    .await;

    match result {
        Ok(Ok(output)) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let count = stdout
                .lines()
                .filter(|l| l.contains("CLOSE_WAIT") && l.contains(":8090"))
                .count();
            if count < 20 {
                ProbeResult {
                    name: "close_wait".to_string(),
                    status: ProbeStatus::Pass,
                    detail: format!("CLOSE_WAIT sockets on :8090: {} (< 20)", count),
                }
            } else {
                ProbeResult {
                    name: "close_wait".to_string(),
                    status: ProbeStatus::Fail,
                    detail: format!("CLOSE_WAIT sockets on :8090: {} (>= 20 — socket flood)", count),
                }
            }
        }
        Ok(Err(e)) => ProbeResult {
            name: "close_wait".to_string(),
            status: ProbeStatus::Fail,
            detail: format!("netstat failed: {}", e),
        },
        Err(e) => ProbeResult {
            name: "close_wait".to_string(),
            status: ProbeStatus::Fail,
            detail: format!("spawn_blocking error: {}", e),
        },
    }
}

/// Probe 14: Exactly one rc-agent.exe process running.
async fn probe_single_instance() -> ProbeResult {
    let result = spawn_blocking(|| {
        std::process::Command::new("tasklist")
            .args(["/FI", "IMAGENAME eq rc-agent.exe"])
            .output()
    })
    .await;

    match result {
        Ok(Ok(output)) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let count = stdout
                .lines()
                .filter(|l| l.contains("rc-agent.exe"))
                .count();
            if count == 1 {
                ProbeResult {
                    name: "single_instance".to_string(),
                    status: ProbeStatus::Pass,
                    detail: "exactly 1 rc-agent.exe instance running".to_string(),
                }
            } else {
                ProbeResult {
                    name: "single_instance".to_string(),
                    status: ProbeStatus::Fail,
                    detail: format!("expected 1 rc-agent.exe, found {}", count),
                }
            }
        }
        Ok(Err(e)) => ProbeResult {
            name: "single_instance".to_string(),
            status: ProbeStatus::Fail,
            detail: format!("tasklist failed: {}", e),
        },
        Err(e) => ProbeResult {
            name: "single_instance".to_string(),
            status: ProbeStatus::Fail,
            detail: format!("spawn_blocking error: {}", e),
        },
    }
}

/// Probe 15: C: drive has > 2GB free.
async fn probe_disk() -> ProbeResult {
    let result = spawn_blocking(|| {
        use sysinfo::Disks;
        let disks = Disks::new_with_refreshed_list();
        disks.into_iter().find(|d| {
            d.mount_point()
                .to_str()
                .map(|s| s == "C:\\" || s == "C:" || s == "/")
                .unwrap_or(false)
        }).map(|d| d.available_space())
    })
    .await;

    match result {
        Ok(Some(available)) => {
            if available > 2_000_000_000 {
                ProbeResult {
                    name: "disk_space".to_string(),
                    status: ProbeStatus::Pass,
                    detail: format!("C: drive: {}GB free", available / 1_073_741_824),
                }
            } else {
                ProbeResult {
                    name: "disk_space".to_string(),
                    status: ProbeStatus::Fail,
                    detail: format!("C: drive low: {}MB free (< 2GB)", available / 1_048_576),
                }
            }
        }
        Ok(None) => ProbeResult {
            name: "disk_space".to_string(),
            status: ProbeStatus::Skip,
            detail: "C: drive not found in sysinfo disk list".to_string(),
        },
        Err(e) => ProbeResult {
            name: "disk_space".to_string(),
            status: ProbeStatus::Fail,
            detail: format!("spawn_blocking error: {}", e),
        },
    }
}

/// Probe 16: System has > 1GB available memory.
async fn probe_memory() -> ProbeResult {
    let result = spawn_blocking(|| {
        use sysinfo::System;
        let mut system = System::new();
        system.refresh_memory();
        system.available_memory()
    })
    .await;

    match result {
        Ok(available) => {
            if available > 1_073_741_824 {
                ProbeResult {
                    name: "memory".to_string(),
                    status: ProbeStatus::Pass,
                    detail: format!("{}GB RAM available", available / 1_073_741_824),
                }
            } else {
                ProbeResult {
                    name: "memory".to_string(),
                    status: ProbeStatus::Fail,
                    detail: format!("low memory: {}MB available (< 1GB)", available / 1_048_576),
                }
            }
        }
        Err(e) => ProbeResult {
            name: "memory".to_string(),
            status: ProbeStatus::Fail,
            detail: format!("spawn_blocking error: {}", e),
        },
    }
}

/// Probe 17: NVIDIA shader cache directory exists.
async fn probe_shader_cache() -> ProbeResult {
    let result = spawn_blocking(|| {
        let path = r"C:\Users\Public\AppData\Local\NVIDIA\GLCache";
        std::path::Path::new(path).exists()
    })
    .await;

    match result {
        Ok(true) => ProbeResult {
            name: "shader_cache".to_string(),
            status: ProbeStatus::Pass,
            detail: "NVIDIA GLCache directory exists".to_string(),
        },
        Ok(false) => ProbeResult {
            name: "shader_cache".to_string(),
            status: ProbeStatus::Skip,
            detail: "NVIDIA GLCache directory not found — may be OK if not NVIDIA GPU".to_string(),
        },
        Err(e) => ProbeResult {
            name: "shader_cache".to_string(),
            status: ProbeStatus::Skip,
            detail: format!("spawn_blocking error: {}", e),
        },
    }
}

/// Probe 18: GIT_HASH build ID set at compile time.
async fn probe_build_id() -> ProbeResult {
    match option_env!("GIT_HASH") {
        Some(hash) if !hash.is_empty() => ProbeResult {
            name: "build_id".to_string(),
            status: ProbeStatus::Pass,
            detail: format!("GIT_HASH={}", hash),
        },
        Some(_) | None => ProbeResult {
            name: "build_id".to_string(),
            status: ProbeStatus::Skip,
            detail: "GIT_HASH not set at compile time".to_string(),
        },
    }
}

/// Probe: Billing state (informational — always Pass).
async fn probe_billing_state(status: &Arc<HeartbeatStatus>) -> ProbeResult {
    let billing_active = status.billing_active.load(Ordering::Relaxed);
    ProbeResult {
        name: "billing_state".to_string(),
        status: ProbeStatus::Pass,
        detail: format!("billing_active={}", billing_active),
    }
}

/// Probe: Session ID (Skip — requires billing context not accessible here).
async fn probe_session_id() -> ProbeResult {
    ProbeResult {
        name: "session_id".to_string(),
        status: ProbeStatus::Skip,
        detail: "session_id probe requires billing context".to_string(),
    }
}

/// Probe: GPU temperature via nvidia-smi (< 90°C = pass).
async fn probe_gpu_temp() -> ProbeResult {
    let result = spawn_blocking(|| {
        std::process::Command::new("nvidia-smi")
            .args(["--query-gpu=temperature.gpu", "--format=csv,noheader"])
            .output()
    })
    .await;

    match result {
        Ok(Ok(output)) if output.status.success() => {
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            match stdout.parse::<u32>() {
                Ok(temp) if temp < 90 => ProbeResult {
                    name: "gpu_temp".to_string(),
                    status: ProbeStatus::Pass,
                    detail: format!("GPU temperature: {}°C (< 90°C)", temp),
                },
                Ok(temp) => ProbeResult {
                    name: "gpu_temp".to_string(),
                    status: ProbeStatus::Fail,
                    detail: format!("GPU temperature: {}°C (>= 90°C — overheating)", temp),
                },
                Err(_) => ProbeResult {
                    name: "gpu_temp".to_string(),
                    status: ProbeStatus::Skip,
                    detail: format!("nvidia-smi output not parseable: {:?}", stdout),
                },
            }
        }
        Ok(Ok(_)) | Ok(Err(_)) | Err(_) => ProbeResult {
            name: "gpu_temp".to_string(),
            status: ProbeStatus::Skip,
            detail: "nvidia-smi not found or failed — may be OK on non-NVIDIA GPU".to_string(),
        },
    }
}

/// Probe: Steam process running.
async fn probe_steam() -> ProbeResult {
    let result = spawn_blocking(|| {
        std::process::Command::new("tasklist")
            .args(["/FI", "IMAGENAME eq steam.exe"])
            .output()
    })
    .await;

    match result {
        Ok(Ok(output)) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let found = stdout.lines().any(|l| l.contains("steam.exe"));
            if found {
                ProbeResult {
                    name: "steam".to_string(),
                    status: ProbeStatus::Pass,
                    detail: "Steam is running".to_string(),
                }
            } else {
                ProbeResult {
                    name: "steam".to_string(),
                    status: ProbeStatus::Skip,
                    detail: "Steam not running — may be OK if no Steam games configured".to_string(),
                }
            }
        }
        Ok(Err(e)) => ProbeResult {
            name: "steam".to_string(),
            status: ProbeStatus::Skip,
            detail: format!("tasklist failed: {}", e),
        },
        Err(e) => ProbeResult {
            name: "steam".to_string(),
            status: ProbeStatus::Skip,
            detail: format!("spawn_blocking error: {}", e),
        },
    }
}

// ─── run_all_probes ───────────────────────────────────────────────────────────

/// Helper: wrap a probe future in a 10s timeout. Returns Fail on timeout.
async fn timed_probe<F, Fut>(name: &str, f: F) -> ProbeResult
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = ProbeResult>,
{
    match timeout(Duration::from_secs(10), f()).await {
        Ok(result) => result,
        Err(_) => ProbeResult {
            name: name.to_string(),
            status: ProbeStatus::Fail,
            detail: "probe timed out after 10s".to_string(),
        },
    }
}

/// Run all 18 probes concurrently. Each probe has a 10-second timeout.
pub async fn run_all_probes(
    status: Arc<HeartbeatStatus>,
    ollama_url: &str,
) -> SelfTestReport {
    let ollama_url = ollama_url.to_string();
    let status2 = status.clone();

    // Compute the ollama probe future before tokio::join! so the macro always sees a fixed
    // number of arguments (22) regardless of which features are enabled.
    #[cfg(feature = "ai-debugger")]
    let ollama_fut = {
        let u = ollama_url.clone();
        timed_probe("ollama", move || {
            let u2 = u.clone();
            async move { probe_ollama(&u2).await }
        })
    };
    #[cfg(not(feature = "ai-debugger"))]
    let ollama_fut = timed_probe("ollama", || async {
        ProbeResult {
            name: "ollama".to_string(),
            status: ProbeStatus::Skip,
            detail: "ai-debugger feature disabled".to_string(),
        }
    });

    let (
        r_ws,
        r_lock_screen,
        r_remote_ops,
        r_overlay,
        r_debug_server,
        r_udp_ac,
        r_udp_f1,
        r_udp_forza,
        r_udp_iracing,
        r_udp_lmu,
        r_hid,
        r_ollama,
        r_close_wait,
        r_single_instance,
        r_disk,
        r_memory,
        r_shader_cache,
        r_build_id,
        r_billing_state,
        r_session_id,
        r_gpu_temp,
        r_steam,
    ) = tokio::join!(
        timed_probe("ws_connected", || probe_ws_connected(&status)),
        timed_probe("lock_screen", || probe_tcp_port("lock_screen", "127.0.0.1:18923")),
        timed_probe("remote_ops", || probe_tcp_port("remote_ops", "127.0.0.1:8090")),
        timed_probe("overlay", || probe_tcp_port("overlay", "127.0.0.1:18925")),
        timed_probe("debug_server", || probe_tcp_port("debug_server", "127.0.0.1:18924")),
        timed_probe("udp_port_AC", || probe_udp_port(9996)),
        timed_probe("udp_port_F1", || probe_udp_port(20777)),
        timed_probe("udp_port_Forza", || probe_udp_port(5300)),
        timed_probe("udp_port_iRacing", || probe_udp_port(6789)),
        timed_probe("udp_port_LMU", || probe_udp_port(5555)),
        timed_probe("hid_wheelbase", || probe_hid()),
        ollama_fut,
        timed_probe("close_wait", || probe_close_wait()),
        timed_probe("single_instance", || probe_single_instance()),
        timed_probe("disk_space", || probe_disk()),
        timed_probe("memory", || probe_memory()),
        timed_probe("shader_cache", || probe_shader_cache()),
        timed_probe("build_id", || probe_build_id()),
        timed_probe("billing_state", || probe_billing_state(&status2)),
        timed_probe("session_id", || probe_session_id()),
        timed_probe("gpu_temp", || probe_gpu_temp()),
        timed_probe("steam", || probe_steam()),
    );

    let probes = vec![
        r_ws,
        r_lock_screen,
        r_remote_ops,
        r_overlay,
        r_debug_server,
        r_udp_ac,
        r_udp_f1,
        r_udp_forza,
        r_udp_iracing,
        r_udp_lmu,
        r_hid,
        r_ollama,
        r_close_wait,
        r_single_instance,
        r_disk,
        r_memory,
        r_shader_cache,
        r_build_id,
        r_billing_state,
        r_session_id,
        r_gpu_temp,
        r_steam,
    ];

    let timestamp = chrono::Utc::now().to_rfc3339();

    SelfTestReport {
        probes,
        verdict: None,
        timestamp,
    }
}

// ─── LLM Verdict ─────────────────────────────────────────────────────────────

/// Shared reqwest client for LLM verdict queries.
#[cfg(feature = "ai-debugger")]
static VERDICT_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

#[cfg(feature = "ai-debugger")]
fn verdict_client() -> &'static reqwest::Client {
    VERDICT_CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("verdict HTTP client build failed")
    })
}

/// Query local Ollama for a verdict response.
#[cfg(feature = "ai-debugger")]
async fn query_ollama_for_verdict(url: &str, model: &str, prompt: &str) -> anyhow::Result<String> {
    #[derive(Deserialize)]
    struct OllamaResp {
        response: String,
    }
    let resp = verdict_client()
        .post(format!("{}/api/generate", url))
        .json(&serde_json::json!({
            "model": model,
            "prompt": prompt,
            "stream": false,
        }))
        .send()
        .await?
        .json::<OllamaResp>()
        .await?;
    Ok(resp.response)
}

/// Get LLM verdict from Ollama. Falls back to deterministic verdict on failure.
#[cfg(feature = "ai-debugger")]
pub async fn get_llm_verdict(
    ollama_url: &str,
    ollama_model: &str,
    probes: &[ProbeResult],
) -> SelfTestVerdict {
    // Format probe results as readable lines
    let probe_summary = probes
        .iter()
        .map(|p| format!("{}: {} ({})", p.name.to_uppercase(), format!("{:?}", p.status).to_uppercase(), p.detail))
        .collect::<Vec<_>>()
        .join("\n");

    let prompt = format!(
        "You are a fleet health diagnostic AI for a sim racing venue. \
        Analyze the following rc-agent self-test probe results and classify the pod health.\n\n\
        Probe Results:\n{}\n\n\
        Rules:\n\
        - CRITICAL if ws_connected=FAIL, lock_screen=FAIL, or billing_state=FAIL\n\
        - DEGRADED if any other probe fails\n\
        - HEALTHY if all probes pass or skip\n\n\
        Respond EXACTLY in this format:\n\
        VERDICT: [HEALTHY|DEGRADED|CRITICAL]\n\
        CORRELATION: [brief explanation of correlated failures]\n\
        FIX: [comma-separated list of recommended auto-fix actions]",
        probe_summary
    );

    match query_ollama_for_verdict(ollama_url, ollama_model, &prompt).await {
        Ok(response) => parse_verdict_response(&response),
        Err(e) => {
            tracing::warn!(target: LOG_TARGET, "Ollama verdict failed ({}), using deterministic fallback", e);
            deterministic_verdict(probes)
        }
    }
}

/// Parse LLM response to extract VERDICT/CORRELATION/FIX fields.
/// Case-insensitive. Falls back to Healthy if no VERDICT line found.
fn parse_verdict_response(response: &str) -> SelfTestVerdict {
    let upper = response.to_uppercase();

    // Prefer most severe verdict if multiple appear
    let level = if upper.contains("VERDICT: CRITICAL") {
        VerdictLevel::Critical
    } else if upper.contains("VERDICT: DEGRADED") {
        VerdictLevel::Degraded
    } else if upper.contains("VERDICT: HEALTHY") {
        VerdictLevel::Healthy
    } else {
        // No VERDICT line — fallback to Healthy
        VerdictLevel::Healthy
    };

    // Extract CORRELATION line
    let analysis = response
        .lines()
        .find(|l| l.to_uppercase().starts_with("CORRELATION:"))
        .map(|l| l["CORRELATION:".len()..].trim().to_string())
        .unwrap_or_else(|| "No correlation analysis provided".to_string());

    // Extract FIX line and split by comma
    let auto_fix_recommendations = response
        .lines()
        .find(|l| l.to_uppercase().starts_with("FIX:"))
        .map(|l| {
            l["FIX:".len()..]
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default();

    SelfTestVerdict {
        level,
        analysis,
        auto_fix_recommendations,
    }
}

/// Deterministic verdict — no LLM required.
/// Critical probes: ws_connected, lock_screen, billing_state.
/// Any other fail => Degraded. All pass/skip => Healthy.
pub fn deterministic_verdict(probes: &[ProbeResult]) -> SelfTestVerdict {
    const CRITICAL_PROBES: &[&str] = &["ws_connected", "lock_screen", "billing_state"];

    // Check critical probes first
    for probe in probes {
        if CRITICAL_PROBES.contains(&probe.name.as_str()) && probe.status == ProbeStatus::Fail {
            return SelfTestVerdict {
                level: VerdictLevel::Critical,
                analysis: format!("Critical probe failed: {} — {}", probe.name, probe.detail),
                auto_fix_recommendations: vec![
                    format!("Investigate {} failure immediately", probe.name),
                ],
            };
        }
    }

    // Check non-critical probes
    for probe in probes {
        if probe.status == ProbeStatus::Fail {
            return SelfTestVerdict {
                level: VerdictLevel::Degraded,
                analysis: format!("Non-critical probe failed: {} — {}", probe.name, probe.detail),
                auto_fix_recommendations: vec![
                    format!("Review {} failure when convenient", probe.name),
                ],
            };
        }
    }

    SelfTestVerdict {
        level: VerdictLevel::Healthy,
        analysis: "All probes passed or skipped — pod is healthy".to_string(),
        auto_fix_recommendations: vec![],
    }
}

// ─── Unit Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_probe_result_serde() {
        let probe = ProbeResult {
            name: "ws_connected".to_string(),
            status: ProbeStatus::Pass,
            detail: "ws_connected=true".to_string(),
        };
        let json = serde_json::to_string(&probe).unwrap();
        assert!(json.contains("\"name\":\"ws_connected\""), "json={}", json);
        assert!(json.contains("\"status\":\"pass\""), "json={}", json);
        assert!(json.contains("\"detail\""), "json={}", json);

        let deserialized: ProbeResult = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.name, "ws_connected");
        assert_eq!(deserialized.status, ProbeStatus::Pass);
    }

    #[test]
    fn test_probe_status_serde_lowercase() {
        // ProbeStatus variants must serialize as lowercase strings
        let pass = serde_json::to_string(&ProbeStatus::Pass).unwrap();
        let fail = serde_json::to_string(&ProbeStatus::Fail).unwrap();
        let skip = serde_json::to_string(&ProbeStatus::Skip).unwrap();
        assert_eq!(pass, "\"pass\"");
        assert_eq!(fail, "\"fail\"");
        assert_eq!(skip, "\"skip\"");
    }

    #[test]
    fn test_verdict_level_serde_screaming_snake_case() {
        let healthy = serde_json::to_string(&VerdictLevel::Healthy).unwrap();
        let degraded = serde_json::to_string(&VerdictLevel::Degraded).unwrap();
        let critical = serde_json::to_string(&VerdictLevel::Critical).unwrap();
        assert_eq!(healthy, "\"HEALTHY\"");
        assert_eq!(degraded, "\"DEGRADED\"");
        assert_eq!(critical, "\"CRITICAL\"");
    }

    #[test]
    fn test_self_test_report_serde_with_verdict_none() {
        let report = SelfTestReport {
            probes: vec![],
            verdict: None,
            timestamp: "2026-03-19T00:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&report).unwrap();
        let deserialized: SelfTestReport = serde_json::from_str(&json).unwrap();
        assert!(deserialized.verdict.is_none());
        assert_eq!(deserialized.timestamp, "2026-03-19T00:00:00Z");
    }

    #[test]
    fn test_verdict_parse_healthy() {
        let response = "VERDICT: HEALTHY\nCORRELATION: All systems nominal\nFIX: none required";
        let verdict = parse_verdict_response(response);
        assert_eq!(verdict.level, VerdictLevel::Healthy);
        assert!(verdict.analysis.contains("All systems nominal"));
    }

    #[test]
    fn test_verdict_parse_critical() {
        let response = "VERDICT: CRITICAL\nCORRELATION: WebSocket disconnected\nFIX: restart rc-agent, check network";
        let verdict = parse_verdict_response(response);
        assert_eq!(verdict.level, VerdictLevel::Critical);
        assert!(verdict.analysis.contains("WebSocket disconnected"));
        assert!(!verdict.auto_fix_recommendations.is_empty());
    }

    #[test]
    fn test_verdict_parse_extracts_correlation() {
        let response = "VERDICT: DEGRADED\nCORRELATION: Disk space low, memory pressure\nFIX: clear temp files";
        let verdict = parse_verdict_response(response);
        assert_eq!(verdict.level, VerdictLevel::Degraded);
        assert!(verdict.analysis.contains("Disk space low"));
    }

    #[test]
    fn test_verdict_parse_no_verdict_line_returns_healthy() {
        let response = "The pod looks okay based on probe results.";
        let verdict = parse_verdict_response(response);
        // No VERDICT line → fallback to Healthy
        assert_eq!(verdict.level, VerdictLevel::Healthy);
    }

    #[test]
    fn test_deterministic_verdict_critical_ws_connected() {
        let probes = vec![ProbeResult {
            name: "ws_connected".to_string(),
            status: ProbeStatus::Fail,
            detail: "ws_connected=false".to_string(),
        }];
        let verdict = deterministic_verdict(&probes);
        assert_eq!(verdict.level, VerdictLevel::Critical);
    }

    #[test]
    fn test_deterministic_verdict_critical_lock_screen() {
        let probes = vec![ProbeResult {
            name: "lock_screen".to_string(),
            status: ProbeStatus::Fail,
            detail: "port unreachable".to_string(),
        }];
        let verdict = deterministic_verdict(&probes);
        assert_eq!(verdict.level, VerdictLevel::Critical);
    }

    #[test]
    fn test_deterministic_verdict_critical_billing_state() {
        let probes = vec![
            ProbeResult {
                name: "ws_connected".to_string(),
                status: ProbeStatus::Pass,
                detail: "ws_connected=true".to_string(),
            },
            ProbeResult {
                name: "billing_state".to_string(),
                status: ProbeStatus::Fail,
                detail: "billing error".to_string(),
            },
        ];
        let verdict = deterministic_verdict(&probes);
        assert_eq!(verdict.level, VerdictLevel::Critical);
    }

    #[test]
    fn test_deterministic_verdict_degraded_non_critical() {
        // disk probe failing → Degraded (not Critical)
        let probes = vec![
            ProbeResult {
                name: "ws_connected".to_string(),
                status: ProbeStatus::Pass,
                detail: "ws_connected=true".to_string(),
            },
            ProbeResult {
                name: "disk_space".to_string(),
                status: ProbeStatus::Fail,
                detail: "C: drive low: 500MB free (< 2GB)".to_string(),
            },
        ];
        let verdict = deterministic_verdict(&probes);
        assert_eq!(verdict.level, VerdictLevel::Degraded);
    }

    #[test]
    fn test_deterministic_verdict_healthy_all_pass() {
        let probes = vec![
            ProbeResult {
                name: "ws_connected".to_string(),
                status: ProbeStatus::Pass,
                detail: "ws_connected=true".to_string(),
            },
            ProbeResult {
                name: "lock_screen".to_string(),
                status: ProbeStatus::Pass,
                detail: "port 18923 reachable".to_string(),
            },
        ];
        let verdict = deterministic_verdict(&probes);
        assert_eq!(verdict.level, VerdictLevel::Healthy);
    }

    #[test]
    fn test_deterministic_verdict_healthy_with_skips() {
        // Skip probes should not cause Degraded
        let probes = vec![
            ProbeResult {
                name: "ws_connected".to_string(),
                status: ProbeStatus::Pass,
                detail: "ws_connected=true".to_string(),
            },
            ProbeResult {
                name: "gpu_temp".to_string(),
                status: ProbeStatus::Skip,
                detail: "nvidia-smi not found".to_string(),
            },
        ];
        let verdict = deterministic_verdict(&probes);
        assert_eq!(verdict.level, VerdictLevel::Healthy);
    }

    #[test]
    fn test_probe_udp_port_from_netstat_output_pass() {
        let netstat_output = "\
Active Connections\r\n\
\r\n\
  Proto  Local Address          Foreign Address        State           PID\r\n\
  UDP    0.0.0.0:9996           *:*                                    12345\r\n\
  UDP    0.0.0.0:20777          *:*                                    12346\r\n";
        assert_eq!(
            probe_udp_port_from_netstat_output(9996, netstat_output),
            ProbeStatus::Pass
        );
        assert_eq!(
            probe_udp_port_from_netstat_output(20777, netstat_output),
            ProbeStatus::Pass
        );
    }

    #[test]
    fn test_probe_udp_port_from_netstat_output_fail() {
        let netstat_output = "\
  UDP    0.0.0.0:9997           *:*                                    12345\r\n";
        // Port 9996 not in output
        assert_eq!(
            probe_udp_port_from_netstat_output(9996, netstat_output),
            ProbeStatus::Fail
        );
    }

    #[test]
    fn test_probe_udp_port_not_matched_in_tcp_section() {
        // If port appears in a TCP line (not UDP), should not match
        let netstat_output = "\
  TCP    0.0.0.0:9996           0.0.0.0:0              LISTENING       12345\r\n";
        assert_eq!(
            probe_udp_port_from_netstat_output(9996, netstat_output),
            ProbeStatus::Fail
        );
    }
}
