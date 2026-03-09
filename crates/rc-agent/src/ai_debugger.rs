use chrono::Utc;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use rc_common::types::{AiDebugSuggestion, DrivingState, SimType};

#[derive(Debug, Clone, Default, Deserialize)]
pub struct AiDebuggerConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_ollama_url")]
    pub ollama_url: String,
    #[serde(default = "default_ollama_model")]
    pub ollama_model: String,
    pub anthropic_api_key: Option<String>,
}

fn default_ollama_url() -> String {
    "http://localhost:11434".to_string()
}
fn default_ollama_model() -> String {
    "racing-point-ops".to_string()
}

/// Runtime snapshot of pod state at the moment of a crash/error.
/// Passed to the AI debugger for richer context.
#[derive(Debug, Clone, Serialize)]
pub struct PodStateSnapshot {
    pub pod_id: String,
    pub pod_number: u32,
    pub lock_screen_active: bool,
    pub billing_active: bool,
    pub game_pid: Option<u32>,
    pub driving_state: Option<DrivingState>,
    pub wheelbase_connected: bool,
    pub ws_connected: bool,
    pub uptime_seconds: u64,
}

/// Result of an auto-fix attempt.
#[derive(Debug, Clone, Serialize)]
pub struct AutoFixResult {
    pub fix_type: String,
    pub detail: String,
    pub success: bool,
}

// Processes that must NEVER be killed by auto-fix
const PROTECTED_PROCESSES: &[&str] = &[
    "rc-agent.exe",
    "pod-agent.exe",
    "explorer.exe",
    "dwm.exe",
    "csrss.exe",
    "winlogon.exe",
    "services.exe",
    "svchost.exe",
    "lsass.exe",
];

/// Analyze a crash/error and produce a debug suggestion.
/// Runs as a spawned async task — makes HTTP calls to Ollama/Anthropic.
pub async fn analyze_crash(
    config: AiDebuggerConfig,
    pod_id: String,
    sim_type: SimType,
    error_context: String,
    snapshot: PodStateSnapshot,
    result_tx: mpsc::Sender<AiDebugSuggestion>,
) {
    let prompt = build_prompt(&sim_type, &error_context, &snapshot);

    // Try Ollama first (local, fast, no internet needed)
    match query_ollama(&config.ollama_url, &config.ollama_model, &prompt).await {
        Ok(suggestion) => {
            let _ = result_tx
                .send(AiDebugSuggestion {
                    pod_id,
                    sim_type,
                    error_context,
                    suggestion,
                    model: format!("ollama/{}", config.ollama_model),
                    created_at: Utc::now(),
                })
                .await;
            return;
        }
        Err(e) => {
            tracing::warn!("Ollama query failed: {}. Trying Anthropic fallback...", e);
        }
    }

    // Fallback to Anthropic API
    if let Some(api_key) = &config.anthropic_api_key {
        match query_anthropic(api_key, &prompt).await {
            Ok(suggestion) => {
                let _ = result_tx
                    .send(AiDebugSuggestion {
                        pod_id,
                        sim_type,
                        error_context,
                        suggestion,
                        model: "anthropic/claude-sonnet".to_string(),
                        created_at: Utc::now(),
                    })
                    .await;
            }
            Err(e) => {
                tracing::error!("Anthropic query also failed: {}", e);
            }
        }
    } else {
        tracing::warn!("No Anthropic API key configured and Ollama failed — no AI debug available");
    }
}

fn build_prompt(sim_type: &SimType, error_context: &str, snapshot: &PodStateSnapshot) -> String {
    format!(
        "You are James, the AI operations assistant for RacingPoint eSports. \
        A game crash occurred and you need to diagnose the issue.\n\n\
        CRASH DETAILS:\n\
        - Game: {:?}\n\
        - Error: {}\n\n\
        POD STATE AT CRASH TIME:\n\
        - Pod: {} (Pod #{})\n\
        - Billing active: {}\n\
        - Lock screen active: {}\n\
        - Game PID: {}\n\
        - Driving state: {:?}\n\
        - Wheelbase connected: {}\n\
        - WebSocket connected: {}\n\
        - Agent uptime: {}s\n\n\
        SYSTEM CONTEXT:\n\
        - 8 pods on subnet 192.168.31.x, server at .51:8080\n\
        - Wheelbases: Conspit Ares 8Nm (OpenFFBoard VID:0x1209 PID:0xFFB0)\n\
        - Games: AC (acs.exe, UDP 9996), F1 (F1_25.exe, 20777), iRacing (6789), LMU (5555), Forza (5300)\n\
        - Protected processes: rc-agent, pod-agent, ConspitLink2.0, explorer, dwm, csrss\n\
        - AC launch: acs.exe directly, AUTOSPAWN=1, CSP FORCE_START=1\n\
        - After AC launch, restart ConspitLink2.0 for wheel display\n\n\
        Provide a concise, actionable diagnosis (under 200 words). \
        Focus on the most likely cause and specific fix commands.",
        sim_type,
        error_context,
        snapshot.pod_id,
        snapshot.pod_number,
        snapshot.billing_active,
        snapshot.lock_screen_active,
        snapshot.game_pid.map(|p| p.to_string()).unwrap_or_else(|| "none".to_string()),
        snapshot.driving_state,
        snapshot.wheelbase_connected,
        snapshot.ws_connected,
        snapshot.uptime_seconds,
    )
}

/// Attempt a deterministic auto-fix based on the AI suggestion text.
/// Only safe, idempotent operations are attempted. Returns None if no known fix matched.
pub fn try_auto_fix(suggestion: &str, snapshot: &PodStateSnapshot) -> Option<AutoFixResult> {
    let lower = suggestion.to_lowercase();

    // Pattern 1: CLOSE_WAIT / zombie socket — kill stale TCP connections
    if lower.contains("close_wait") || lower.contains("zombie") || lower.contains("stale socket") {
        return Some(fix_stale_sockets(snapshot));
    }

    // Pattern 2: Restart ConspitLink
    if lower.contains("conspitlink") || lower.contains("conspit link") || lower.contains("wheelbase") && lower.contains("restart") {
        return Some(fix_restart_conspit_link());
    }

    // Pattern 3: Kill error dialogs (WerFault) — check before game relaunch since both may match
    if lower.contains("werfault") || lower.contains("error dialog") || lower.contains("crash dialog") {
        return Some(fix_kill_error_dialogs());
    }

    // Pattern 4: Relaunch game — kill crashed game process
    if lower.contains("relaunch") && lower.contains("game")
        || lower.contains("restart") && (lower.contains("acs.exe") || lower.contains("game"))
    {
        return Some(fix_kill_stale_game());
    }

    // Pattern 5: Disk space / temp files
    if lower.contains("disk space") || lower.contains("temp files") || lower.contains("clean temp") {
        return Some(fix_clean_temp());
    }

    None
}

// ─── Auto-Fix Implementations ────────────────────────────────────────────────

fn fix_stale_sockets(_snapshot: &PodStateSnapshot) -> AutoFixResult {
    tracing::info!("[auto-fix] Attempting to clear stale sockets");

    // Kill any CLOSE_WAIT state by resetting network stack for our ports
    // Safe: only affects orphaned connections, not active ones
    let result = std::process::Command::new("powershell")
        .args([
            "-NoProfile", "-Command",
            "Get-NetTCPConnection -State CloseWait -ErrorAction SilentlyContinue | \
             Where-Object { $_.LocalPort -in @(18923, 18924, 18925, 8090) } | \
             ForEach-Object { \
                $pid = $_.OwningProcess; \
                $proc = Get-Process -Id $pid -ErrorAction SilentlyContinue; \
                if ($proc -and $proc.ProcessName -notin @('rc-agent','pod-agent','explorer','dwm','csrss')) { \
                    Stop-Process -Id $pid -Force -ErrorAction SilentlyContinue; \
                    \"Killed stale PID $pid ($($proc.ProcessName))\" \
                } \
             }",
        ])
        .output();

    match result {
        Ok(output) => {
            let detail = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let detail = if detail.is_empty() { "No stale sockets found".to_string() } else { detail };
            tracing::info!("[auto-fix] Stale sockets: {}", detail);
            AutoFixResult {
                fix_type: "clear_stale_sockets".to_string(),
                detail,
                success: output.status.success(),
            }
        }
        Err(e) => AutoFixResult {
            fix_type: "clear_stale_sockets".to_string(),
            detail: format!("Failed to run cleanup: {}", e),
            success: false,
        },
    }
}

fn fix_restart_conspit_link() -> AutoFixResult {
    tracing::info!("[auto-fix] Restarting ConspitLink2.0");

    // Kill existing instance
    let _ = std::process::Command::new("taskkill")
        .args(["/IM", "ConspitLink2.0.exe", "/F"])
        .output();

    // Wait briefly for process to die
    std::thread::sleep(std::time::Duration::from_secs(2));

    // Relaunch
    let result = std::process::Command::new("cmd")
        .args([
            "/C", "start", "", "/MIN",
            "C:\\Program Files (x86)\\Conspit Link 2.0\\ConspitLink2.0.exe",
        ])
        .output();

    match result {
        Ok(_) => {
            tracing::info!("[auto-fix] ConspitLink2.0 restarted");
            AutoFixResult {
                fix_type: "restart_conspit_link".to_string(),
                detail: "Killed and relaunched ConspitLink2.0.exe".to_string(),
                success: true,
            }
        }
        Err(e) => AutoFixResult {
            fix_type: "restart_conspit_link".to_string(),
            detail: format!("Failed to restart: {}", e),
            success: false,
        },
    }
}

fn fix_kill_stale_game() -> AutoFixResult {
    tracing::info!("[auto-fix] Killing stale game processes");

    let game_exes = [
        "acs.exe", "AssettoCorsa.exe",
        "F1_25.exe", "iRacingSim64DX11.exe",
        "LMU.exe", "ForzaMotorsport.exe",
    ];

    let mut killed = Vec::new();
    for exe in &game_exes {
        // Verify it's not a protected process (it never should be, but safety first)
        if PROTECTED_PROCESSES.iter().any(|p| p.eq_ignore_ascii_case(exe)) {
            continue;
        }
        let output = std::process::Command::new("taskkill")
            .args(["/IM", exe, "/F"])
            .output();
        if let Ok(o) = output {
            if o.status.success() {
                killed.push(*exe);
            }
        }
    }

    let detail = if killed.is_empty() {
        "No stale game processes found".to_string()
    } else {
        format!("Killed: {}", killed.join(", "))
    };

    tracing::info!("[auto-fix] Stale games: {}", detail);
    AutoFixResult {
        fix_type: "kill_stale_game".to_string(),
        detail,
        success: true,
    }
}

fn fix_clean_temp() -> AutoFixResult {
    tracing::info!("[auto-fix] Cleaning temp files");

    let result = std::process::Command::new("powershell")
        .args([
            "-NoProfile", "-Command",
            "Remove-Item -Path \"$env:TEMP\\*\" -Recurse -Force -ErrorAction SilentlyContinue; \
             $freed = [math]::Round((Get-PSDrive C).Free / 1GB, 1); \
             \"Cleaned temp. Free disk: ${freed}GB\"",
        ])
        .output();

    match result {
        Ok(output) => {
            let detail = String::from_utf8_lossy(&output.stdout).trim().to_string();
            tracing::info!("[auto-fix] Temp cleanup: {}", detail);
            AutoFixResult {
                fix_type: "clean_temp".to_string(),
                detail,
                success: output.status.success(),
            }
        }
        Err(e) => AutoFixResult {
            fix_type: "clean_temp".to_string(),
            detail: format!("Failed to clean temp: {}", e),
            success: false,
        },
    }
}

fn fix_kill_error_dialogs() -> AutoFixResult {
    tracing::info!("[auto-fix] Killing error dialogs");

    let _ = std::process::Command::new("taskkill")
        .args(["/IM", "WerFault.exe", "/F"])
        .output();

    AutoFixResult {
        fix_type: "kill_error_dialogs".to_string(),
        detail: "Killed WerFault.exe error dialogs".to_string(),
        success: true,
    }
}

// ─── AI Provider Queries ─────────────────────────────────────────────────────

async fn query_ollama(url: &str, model: &str, prompt: &str) -> anyhow::Result<String> {
    let client = reqwest::Client::new();
    let resp = client
        .post(&format!("{}/api/generate", url))
        .json(&serde_json::json!({
            "model": model,
            "prompt": prompt,
            "stream": false,
        }))
        .timeout(std::time::Duration::from_secs(30))
        .send()
        .await?;

    #[derive(Deserialize)]
    struct OllamaResponse {
        response: String,
    }
    let body: OllamaResponse = resp.json().await?;
    Ok(body.response)
}

async fn query_anthropic(api_key: &str, prompt: &str) -> anyhow::Result<String> {
    let client = reqwest::Client::new();
    let resp = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .json(&serde_json::json!({
            "model": "claude-sonnet-4-20250514",
            "max_tokens": 500,
            "messages": [{"role": "user", "content": prompt}]
        }))
        .timeout(std::time::Duration::from_secs(30))
        .send()
        .await?;

    #[derive(Deserialize)]
    struct AnthropicContent {
        text: String,
    }
    #[derive(Deserialize)]
    struct AnthropicResponse {
        content: Vec<AnthropicContent>,
    }
    let body: AnthropicResponse = resp.json().await?;
    Ok(body
        .content
        .first()
        .map(|c| c.text.clone())
        .unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_prompt_includes_snapshot() {
        let snapshot = PodStateSnapshot {
            pod_id: "pod_3".to_string(),
            pod_number: 3,
            lock_screen_active: true,
            billing_active: true,
            game_pid: Some(1234),
            driving_state: Some(DrivingState::Active),
            wheelbase_connected: true,
            ws_connected: true,
            uptime_seconds: 3600,
        };
        let prompt = build_prompt(&SimType::AssettoCorsa, "exit code -1", &snapshot);
        assert!(prompt.contains("Pod #3"));
        assert!(prompt.contains("Billing active: true"));
        assert!(prompt.contains("1234"));
        assert!(prompt.contains("3600s"));
        assert!(prompt.contains("RacingPoint"));
    }

    #[test]
    fn test_auto_fix_close_wait() {
        let snapshot = PodStateSnapshot {
            pod_id: "pod_1".to_string(),
            pod_number: 1,
            lock_screen_active: false,
            billing_active: false,
            game_pid: None,
            driving_state: None,
            wheelbase_connected: false,
            ws_connected: true,
            uptime_seconds: 100,
        };
        let result = try_auto_fix("Check for CLOSE_WAIT zombie sockets on the lock screen port", &snapshot);
        assert!(result.is_some());
        assert_eq!(result.unwrap().fix_type, "clear_stale_sockets");
    }

    #[test]
    fn test_auto_fix_conspit_link() {
        let snapshot = PodStateSnapshot {
            pod_id: "pod_2".to_string(),
            pod_number: 2,
            lock_screen_active: false,
            billing_active: false,
            game_pid: None,
            driving_state: None,
            wheelbase_connected: false,
            ws_connected: true,
            uptime_seconds: 100,
        };
        let result = try_auto_fix("Try restarting ConspitLink2.0 to restore the wheelbase connection", &snapshot);
        assert!(result.is_some());
        assert_eq!(result.unwrap().fix_type, "restart_conspit_link");
    }

    #[test]
    fn test_auto_fix_relaunch_game() {
        let snapshot = PodStateSnapshot {
            pod_id: "pod_5".to_string(),
            pod_number: 5,
            lock_screen_active: false,
            billing_active: true,
            game_pid: Some(5678),
            driving_state: None,
            wheelbase_connected: true,
            ws_connected: true,
            uptime_seconds: 200,
        };
        let result = try_auto_fix("Kill stale acs.exe process and relaunch the game", &snapshot);
        assert!(result.is_some());
        assert_eq!(result.unwrap().fix_type, "kill_stale_game");
    }

    #[test]
    fn test_auto_fix_no_match() {
        let snapshot = PodStateSnapshot {
            pod_id: "pod_1".to_string(),
            pod_number: 1,
            lock_screen_active: false,
            billing_active: false,
            game_pid: None,
            driving_state: None,
            wheelbase_connected: false,
            ws_connected: true,
            uptime_seconds: 100,
        };
        let result = try_auto_fix("The issue seems to be with the GPU driver version being outdated", &snapshot);
        assert!(result.is_none(), "Should not match any auto-fix pattern");
    }

    #[test]
    fn test_auto_fix_clean_temp() {
        let snapshot = PodStateSnapshot {
            pod_id: "pod_4".to_string(),
            pod_number: 4,
            lock_screen_active: false,
            billing_active: false,
            game_pid: None,
            driving_state: None,
            wheelbase_connected: true,
            ws_connected: true,
            uptime_seconds: 500,
        };
        let result = try_auto_fix("Low disk space detected — clean temp files and free up space", &snapshot);
        assert!(result.is_some());
        assert_eq!(result.unwrap().fix_type, "clean_temp");
    }

    #[test]
    fn test_auto_fix_error_dialogs() {
        let snapshot = PodStateSnapshot {
            pod_id: "pod_6".to_string(),
            pod_number: 6,
            lock_screen_active: false,
            billing_active: false,
            game_pid: None,
            driving_state: None,
            wheelbase_connected: true,
            ws_connected: true,
            uptime_seconds: 300,
        };
        let result = try_auto_fix("Dismiss the WerFault error dialog and restart the game", &snapshot);
        assert!(result.is_some());
        assert_eq!(result.unwrap().fix_type, "kill_error_dialogs");
    }

    #[test]
    fn test_default_model_is_racing_point_ops() {
        assert_eq!(default_ollama_model(), "racing-point-ops");
    }

    #[test]
    fn test_protected_processes_list() {
        assert!(PROTECTED_PROCESSES.contains(&"rc-agent.exe"));
        assert!(PROTECTED_PROCESSES.contains(&"pod-agent.exe"));
        assert!(PROTECTED_PROCESSES.contains(&"explorer.exe"));
        assert!(!PROTECTED_PROCESSES.contains(&"acs.exe"));
    }
}
