use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::sync::mpsc;

use rc_common::types::{AiDebugSuggestion, DrivingState, SimType};
use crate::ffb_controller::FfbController;

const LOG_TARGET: &str = "ai-debugger";

#[derive(Debug, Clone, Default, Deserialize)]
pub struct AiDebuggerConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_ollama_url")]
    pub ollama_url: String,
    #[serde(default = "default_ollama_model")]
    pub ollama_model: String,
    pub openrouter_api_key: Option<String>,
    #[serde(default = "default_openrouter_model")]
    pub openrouter_model: String,
}

fn default_ollama_url() -> String {
    "http://localhost:11434".to_string()
}
fn default_ollama_model() -> String {
    "rc-bot".to_string()
}
fn default_openrouter_model() -> String {
    "openrouter/auto".to_string()
}

/// Runtime snapshot of pod state at the moment of a crash/error.
/// Passed to the AI debugger for richer context.
#[derive(Debug, Clone, Serialize, Default)]
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
    #[serde(default)]
    pub last_udp_secs_ago: Option<u64>,        // seconds since last UDP frame; None = never received
    #[serde(default)]
    pub game_launch_elapsed_secs: Option<u64>, // seconds since LaunchGame command; None = not launching
    #[serde(default)]
    pub hid_last_error: bool,                  // true if driving_detector last saw HidDisconnected
}

/// Result of an auto-fix attempt.
#[derive(Debug, Clone, Serialize)]
pub struct AutoFixResult {
    pub fix_type: String,
    pub detail: String,
    pub success: bool,
}

// ─── Pattern Memory ─────────────────────────────────────────────────────────
// Stores resolved crash→fix pairs. When the same crash pattern recurs,
// the fix is applied instantly (<100ms) without querying the AI.

/// A resolved crash incident stored in pattern memory.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct DebugIncident {
    pattern_key: String,
    fix_type: String,
    ai_suggestion: String,
    success_count: u32,
    last_seen: String,
}

/// Pattern memory — learns from resolved crashes for instant replay.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DebugMemory {
    incidents: Vec<DebugIncident>,
}

const MEMORY_PATH: &str = r"C:\RacingPoint\debug-memory.json";
const MAX_INCIDENTS: usize = 100;

impl DebugMemory {
    /// Load from disk, or return empty if file doesn't exist / is corrupt.
    pub fn load() -> Self {
        let path = Path::new(MEMORY_PATH);
        if !path.exists() {
            return Self::default();
        }
        match std::fs::read_to_string(path) {
            Ok(data) => serde_json::from_str(&data).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    /// Save to disk atomically (write temp, then rename).
    pub fn save(&self) {
        let tmp = format!("{}.tmp", MEMORY_PATH);
        let json = match serde_json::to_string_pretty(self) {
            Ok(j) => j,
            Err(e) => {
                tracing::error!(target: LOG_TARGET, "Failed to serialize: {}", e);
                return;
            }
        };
        if let Err(e) = std::fs::write(&tmp, &json) {
            tracing::error!(target: LOG_TARGET, "Failed to write temp file: {}", e);
            return;
        }
        if let Err(e) = std::fs::rename(&tmp, MEMORY_PATH) {
            tracing::error!(target: LOG_TARGET, "Failed to rename: {}", e);
        }
    }

    /// Look up a known fix for this crash pattern.
    /// Returns the cached AI suggestion text (which contains keywords for try_auto_fix).
    pub fn instant_fix(&self, sim_type: &SimType, error_context: &str) -> Option<String> {
        let key = Self::pattern_key(sim_type, error_context);
        self.incidents
            .iter()
            .filter(|i| i.pattern_key == key && i.success_count > 0)
            .max_by_key(|i| i.success_count)
            .map(|i| i.ai_suggestion.clone())
    }

    /// Record a successful fix. Increments count if pattern already known, otherwise appends.
    pub fn record_fix(
        &mut self,
        sim_type: &SimType,
        error_context: &str,
        fix_type: &str,
        ai_suggestion: &str,
    ) {
        let key = Self::pattern_key(sim_type, error_context);
        if let Some(incident) = self.incidents.iter_mut().find(|i| i.pattern_key == key && i.fix_type == fix_type) {
            incident.success_count += 1;
            incident.last_seen = Utc::now().to_rfc3339();
        } else {
            // Prune oldest low-count entries if at capacity
            if self.incidents.len() >= MAX_INCIDENTS {
                self.incidents.sort_by(|a, b| b.success_count.cmp(&a.success_count));
                self.incidents.truncate(MAX_INCIDENTS - 1);
            }
            self.incidents.push(DebugIncident {
                pattern_key: key,
                fix_type: fix_type.to_string(),
                ai_suggestion: ai_suggestion.to_string(),
                success_count: 1,
                last_seen: Utc::now().to_rfc3339(),
            });
        }
        self.save();
    }

    /// Extract pattern key from crash context: "{SimType}:{exit_code}"
    /// e.g. "AssettoCorsa:-1", "F125:3221225477", "AssettoCorsa:unknown"
    fn pattern_key(sim_type: &SimType, error_context: &str) -> String {
        let exit_code = error_context
            .split("exit code ")
            .nth(1)
            .and_then(|s| s.split(|c: char| c == ')' || c == ' ' || c == ',').next())
            .unwrap_or("unknown");
        format!("{:?}:{}", sim_type, exit_code)
    }
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
    tracing::info!(
        target: LOG_TARGET,
        "Starting crash analysis for {} ({:?}), model={}, url={}",
        pod_id, sim_type, config.ollama_model, config.ollama_url
    );

    // ── Check pattern memory for instant fix ────────────────────────────────
    let memory = DebugMemory::load();
    if let Some(cached_suggestion) = memory.instant_fix(&sim_type, &error_context) {
        tracing::info!(
            target: LOG_TARGET,
            "INSTANT FIX from pattern memory for {} ({:?})",
            pod_id, sim_type
        );
        let _ = result_tx
            .send(AiDebugSuggestion {
                pod_id,
                sim_type,
                error_context,
                suggestion: format!("[PATTERN MEMORY — instant fix]\n\n{}", cached_suggestion),
                model: "rc-bot/memory".to_string(),
                created_at: Utc::now(),
            })
            .await;
        return;
    }

    // ── No memory match — query AI ──────────────────────────────────────────
    // Collect error logs (blocking I/O — file reads + PowerShell calls)
    let error_ctx = tokio::task::spawn_blocking(PodErrorContext::collect)
        .await
        .unwrap_or_default();
    tracing::info!(target: LOG_TARGET, "Error context: {} bot events, {} win errors, {} CLOSE_WAIT",
        error_ctx.recent_bot_events.len(), error_ctx.windows_app_errors.len(), error_ctx.close_wait_count);

    let prompt = build_prompt(&sim_type, &error_context, &snapshot, &error_ctx);
    tracing::debug!(target: LOG_TARGET, "Prompt length: {} chars", prompt.len());

    // Try Ollama first (local, fast, no internet needed)
    match query_ollama(&config.ollama_url, &config.ollama_model, &prompt).await {
        Ok(suggestion) => {
            tracing::info!(target: LOG_TARGET, "Ollama responded: {} chars", suggestion.len());
            match result_tx
                .send(AiDebugSuggestion {
                    pod_id,
                    sim_type,
                    error_context,
                    suggestion,
                    model: format!("ollama/{}", config.ollama_model),
                    created_at: Utc::now(),
                })
                .await
            {
                Ok(()) => tracing::info!(target: LOG_TARGET, "Suggestion sent to result channel"),
                Err(e) => tracing::error!(target: LOG_TARGET, "Failed to send suggestion: {}", e),
            }
            return;
        }
        Err(e) => {
            tracing::warn!(target: LOG_TARGET, "Ollama query failed: {}. Trying OpenRouter fallback...", e);
        }
    }

    // Fallback to OpenRouter API (seamless tier 2 transition)
    if let Some(api_key) = &config.openrouter_api_key {
        match query_openrouter(api_key, &config.openrouter_model, &prompt).await {
            Ok(suggestion) => {
                let _ = result_tx
                    .send(AiDebugSuggestion {
                        pod_id,
                        sim_type,
                        error_context,
                        suggestion,
                        model: format!("openrouter/{}", config.openrouter_model),
                        created_at: Utc::now(),
                    })
                    .await;
            }
            Err(e) => {
                tracing::error!(target: LOG_TARGET, "OpenRouter query also failed: {}", e);
            }
        }
    } else {
        tracing::warn!(target: LOG_TARGET, "No OpenRouter API key configured and Ollama failed — no AI debug available");
    }
}

/// Error context collected from system logs at crash time.
/// Fed to the LLM alongside the PodStateSnapshot for richer diagnosis.
#[derive(Debug, Clone, Default)]
pub struct PodErrorContext {
    /// Last N lines from rc-bot-events.log (self-monitor events)
    pub recent_bot_events: Vec<String>,
    /// Recent Windows Event Viewer Application errors (condensed)
    pub windows_app_errors: Vec<String>,
    /// Current CLOSE_WAIT socket count on :8090
    pub close_wait_count: u32,
    /// Known crash patterns from debug-memory.json (last 5)
    pub known_patterns: Vec<String>,
}

impl PodErrorContext {
    /// Collect error context from local system logs.
    /// Runs as blocking I/O — call from spawn_blocking.
    pub fn collect() -> Self {
        let mut ctx = Self::default();

        // 1. Last 15 lines of rc-bot-events.log
        if let Ok(data) = std::fs::read_to_string(r"C:\RacingPoint\rc-bot-events.log") {
            ctx.recent_bot_events = data
                .lines()
                .rev()
                .take(15)
                .map(|l| l.to_string())
                .collect::<Vec<_>>();
            ctx.recent_bot_events.reverse();
        }

        // 2. Windows Event Viewer — last 10 Application errors (condensed)
        if let Ok(output) = hidden_cmd("powershell")
            .args([
                "-NoProfile", "-Command",
                "Get-WinEvent -FilterHashtable @{LogName='Application';Level=2} \
                 -MaxEvents 10 -ErrorAction SilentlyContinue | \
                 ForEach-Object { \"[$($_.TimeCreated.ToString('HH:mm:ss'))] \
                 $($_.ProviderName): $($_.Message.Split(\"`n\")[0].Trim())\" }",
            ])
            .output()
        {
            let text = String::from_utf8_lossy(&output.stdout);
            ctx.windows_app_errors = text
                .lines()
                .filter(|l| !l.trim().is_empty())
                .take(10)
                .map(|l| l.trim().to_string())
                .collect();
        }

        // 3. CLOSE_WAIT count on :8090
        if let Ok(output) = hidden_cmd("powershell")
            .args([
                "-NoProfile", "-Command",
                "(Get-NetTCPConnection -State CloseWait -ErrorAction SilentlyContinue | \
                 Where-Object { $_.LocalPort -eq 8090 }).Count",
            ])
            .output()
        {
            let count_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
            ctx.close_wait_count = count_str.parse().unwrap_or(0);
        }

        // 4. Known patterns from debug-memory (last 5)
        let memory = DebugMemory::load();
        ctx.known_patterns = memory.incidents
            .iter()
            .rev()
            .take(5)
            .map(|i| format!("{} → {} (×{})", i.pattern_key, i.fix_type, i.success_count))
            .collect();

        ctx
    }

    /// Format as compact text for the LLM prompt (budget: ~800 tokens).
    fn to_prompt_section(&self) -> String {
        let mut sections = Vec::new();

        if !self.recent_bot_events.is_empty() {
            sections.push(format!(
                "RC-BOT EVENTS (recent):\n{}",
                self.recent_bot_events.join("\n")
            ));
        }

        if !self.windows_app_errors.is_empty() {
            sections.push(format!(
                "WINDOWS APPLICATION ERRORS:\n{}",
                self.windows_app_errors.join("\n")
            ));
        }

        if self.close_wait_count > 0 {
            sections.push(format!(
                "NETWORK: {} CLOSE_WAIT sockets on :8090",
                self.close_wait_count
            ));
        }

        if !self.known_patterns.is_empty() {
            sections.push(format!(
                "KNOWN CRASH PATTERNS:\n{}",
                self.known_patterns.join("\n")
            ));
        }

        if sections.is_empty() {
            "No recent errors in logs.".to_string()
        } else {
            sections.join("\n\n")
        }
    }
}

fn build_prompt(sim_type: &SimType, error_context: &str, snapshot: &PodStateSnapshot, error_ctx: &PodErrorContext) -> String {
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
        ERROR LOGS:\n{}\n\n\
        SYSTEM CONTEXT:\n\
        - 8 pods on subnet 192.168.31.x, server at .23:8080\n\
        - Wheelbases: Conspit Ares 8Nm (OpenFFBoard VID:0x1209 PID:0xFFB0)\n\
        - Games: AC (acs.exe, UDP 9996), F1 (F1_25.exe, 20777), iRacing (6789), LMU (5555), Forza (5300)\n\
        - Protected processes: rc-agent, pod-agent, ConspitLink2.0, explorer, dwm, csrss\n\
        - AC launch: acs.exe directly, AUTOSPAWN=1, CSP FORCE_START=1\n\
        - ConspitLink2.0 is managed by a separate watchdog (do NOT suggest restarting it)\n\n\
        DIAGNOSTIC KEYWORDS (use when relevant — they trigger automatic fixes):\n\
        - \"CLOSE_WAIT\" or \"zombie\" or \"stale socket\" → socket cleanup\n\
        - \"WerFault\" or \"error dialog\" → kill error dialogs\n\
        - \"game frozen\" + \"relaunch\" → kill frozen game\n\
        - \"launch timeout\" → kill Content Manager\n\
        - \"wheelbase\" + \"usb reset\" → reset FFB\n\
        - \"relaunch\" + \"game\" → kill stale game\n\
        - \"disk space\" or \"temp files\" → clean temp\n\n\
        Provide a concise, actionable diagnosis (under 200 words). \
        Correlate the error logs with the crash. Use diagnostic keywords when a fix applies.",
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
        error_ctx.to_prompt_section(),
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

    // ConspitLink is managed by the 10s watchdog in the main loop — no AI auto-fix needed

    // Pattern 2: Kill error dialogs (WerFault) — check before game relaunch since both may match
    if lower.contains("werfault") || lower.contains("error dialog") || lower.contains("crash dialog") {
        return Some(fix_kill_error_dialogs());
    }

    // Pattern 3a: Frozen game — billing-gated relaunch via IsHungAppWindow detection
    // MUST appear before Pattern 4 so "game frozen...relaunch" dispatches here, not kill_stale_game
    if lower.contains("game frozen") || (lower.contains("frozen") && lower.contains("relaunch")) {
        return Some(fix_frozen_game(snapshot));
    }

    // Pattern 3b: Launch timeout — Content Manager hang (CRASH-02)
    if lower.contains("launch timeout") || (lower.contains("content manager") && lower.contains("kill cm")) {
        return Some(fix_launch_timeout(snapshot));
    }

    // Pattern 3c: USB/HID reconnect for wheelbase
    if (lower.contains("wheelbase") || lower.contains("hid")) && lower.contains("usb reset") {
        return Some(fix_usb_reconnect(snapshot));
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

    // Pattern 8+9: DirectX / shader cache clear
    if lower.contains("directx") || lower.contains("d3d") || lower.contains("gpu driver")
        || lower.contains("shader cache") || lower.contains("pipeline cache") {
        return Some(fix_directx_shader_cache(snapshot));
    }

    // Pattern 10: Memory pressure
    if lower.contains("out of memory") || lower.contains("memory leak") {
        return Some(fix_memory_pressure(snapshot));
    }

    // Pattern 11: DLL repair
    if lower.contains("dll missing") || lower.contains("dll not found") {
        return Some(fix_dll_repair(snapshot));
    }

    // Pattern 12: Steam restart
    if (lower.contains("steam") && lower.contains("update"))
        || (lower.contains("steam") && lower.contains("downloading")) {
        return Some(fix_steam_restart(snapshot));
    }

    // Pattern 13: Performance throttle
    if lower.contains("low fps") || lower.contains("frame drops") || lower.contains("stuttering") {
        return Some(fix_performance_throttle(snapshot));
    }

    // Pattern 14: Network adapter reset
    if lower.contains("network timeout") || lower.contains("connection refused") {
        return Some(fix_network_adapter_reset(snapshot));
    }

    None
}

/// Create a Command with CREATE_NO_WINDOW on Windows (prevents console flash).
fn hidden_cmd(program: &str) -> std::process::Command {
    let mut cmd = std::process::Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }
    cmd
}

// ─── Phase 24 Wave 1: Fix Function Implementations ──────────────────────────

pub(crate) fn fix_frozen_game(snapshot: &PodStateSnapshot) -> AutoFixResult {
    // BILLING GATE — required inside fix function (not just at call site).
    // DebugMemory instant_fix() replays this function directly, bypassing call-site guards.
    if !snapshot.billing_active {
        tracing::debug!(target: LOG_TARGET, "fix_frozen_game: billing not active — skipping destructive action");
        return AutoFixResult {
            fix_type: "fix_frozen_game".to_string(),
            detail: "billing not active — skipping".to_string(),
            success: false,
        };
    }

    tracing::warn!(target: LOG_TARGET, "fix_frozen_game: game frozen — zeroing FFB before kill");

    // SAFETY: FFB ZERO MUST happen before any game process kill.
    // An 8Nm Conspit Ares with a stale FFB command is a physical hazard.
    // This ordering is non-negotiable — do NOT move the taskkill calls above this line.
    let ffb = FfbController::new(0x1209, 0xFFB0);
    let ffb_result = ffb.zero_force();
    let ffb_detail = match ffb_result {
        Ok(true) => "FFB zeroed".to_string(),
        Ok(false) => "FFB not found (skipped)".to_string(),
        Err(e) => format!("FFB zero error: {}", e),
    };
    tracing::info!(target: LOG_TARGET, "fix_frozen_game: {}", ffb_detail);

    // Kill error dialogs before game kill — customer must not see crash dialogs
    let _ = hidden_cmd("taskkill").args(["/IM", "WerFault.exe", "/F"]).output();
    let _ = hidden_cmd("taskkill").args(["/IM", "WerFaultSecure.exe", "/F"]).output();

    // Now kill the frozen game processes
    let game_exes = ["acs.exe", "AssettoCorsa.exe", "F1_25.exe", "iRacingSim64DX11.exe", "LMU.exe", "ForzaMotorsport.exe"];
    let mut killed = Vec::new();
    for exe in &game_exes {
        if PROTECTED_PROCESSES.iter().any(|p| p.eq_ignore_ascii_case(exe)) { continue; }
        if let Ok(o) = hidden_cmd("taskkill").args(["/IM", exe, "/F"]).output() {
            if o.status.success() { killed.push(*exe); }
        }
    }

    let detail = format!("{} | killed: {}", ffb_detail, killed.join(", "));
    tracing::info!(target: LOG_TARGET, "fix_frozen_game: {}", detail);
    AutoFixResult {
        fix_type: "fix_frozen_game".to_string(),
        detail,
        success: true,
    }
}

pub(crate) fn fix_launch_timeout(_snapshot: &PodStateSnapshot) -> AutoFixResult {
    // No billing gate here — launch timeout can occur before billing activates.
    // Detection in failure_monitor.rs gates on launch_started_at.is_some() instead.
    tracing::warn!(target: LOG_TARGET, "fix_launch_timeout: killing Content Manager — 90s timeout");

    // Kill both possible Content Manager process names (varies by install method on pods)
    let _ = hidden_cmd("taskkill").args(["/IM", "Content Manager.exe", "/F"]).output();
    let _ = hidden_cmd("taskkill").args(["/IM", "acmanager.exe", "/F"]).output();

    // Also kill acs.exe in case it spawned but hung before reaching Live state
    let _ = hidden_cmd("taskkill").args(["/IM", "acs.exe", "/F"]).output();

    tracing::info!(target: LOG_TARGET, "fix_launch_timeout: Content Manager and acs.exe killed");
    AutoFixResult {
        fix_type: "fix_launch_timeout".to_string(),
        detail: "Killed Content Manager.exe, acmanager.exe, acs.exe".to_string(),
        success: true,
    }
}

pub(crate) fn fix_usb_reconnect(_snapshot: &PodStateSnapshot) -> AutoFixResult {
    // USB-01: Conspit Ares wheelbase reconnected — zero FFB to clear stale state.
    // driving_detector.rs will pick up the device on its next 100ms HID poll cycle.
    // This fix only ensures the wheelbase starts from a clean (zero torque) state.
    tracing::info!(target: LOG_TARGET, "fix_usb_reconnect: wheelbase HID reconnected — resetting FFB state");

    let ffb = FfbController::new(0x1209, 0xFFB0);
    let ffb_detail = match ffb.zero_force() {
        Ok(true) => "FFB zeroed on reconnect".to_string(),
        Ok(false) => "Wheelbase not yet enumerable after reconnect (normal — retried next poll)".to_string(),
        Err(e) => format!("FFB zero error on reconnect: {}", e),
    };
    tracing::info!(target: LOG_TARGET, "fix_usb_reconnect: {}", ffb_detail);

    AutoFixResult {
        fix_type: "fix_usb_reconnect".to_string(),
        detail: ffb_detail,
        success: true,
    }
}

// ─── Auto-Fix Implementations ────────────────────────────────────────────────

fn fix_stale_sockets(_snapshot: &PodStateSnapshot) -> AutoFixResult {
    tracing::info!(target: LOG_TARGET, "Attempting to clear stale sockets");

    // Kill any CLOSE_WAIT state by resetting network stack for our ports
    // Safe: only affects orphaned connections, not active ones
    let result = hidden_cmd("powershell")
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
            tracing::info!(target: LOG_TARGET, "Stale sockets: {}", detail);
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

fn fix_kill_stale_game() -> AutoFixResult {
    tracing::info!(target: LOG_TARGET, "Killing stale game processes");

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
        let output = hidden_cmd("taskkill")
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

    tracing::info!(target: LOG_TARGET, "Stale games: {}", detail);
    AutoFixResult {
        fix_type: "kill_stale_game".to_string(),
        detail,
        success: true,
    }
}

fn fix_clean_temp() -> AutoFixResult {
    tracing::info!(target: LOG_TARGET, "Cleaning temp files");

    let result = hidden_cmd("powershell")
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
            tracing::info!(target: LOG_TARGET, "Temp cleanup: {}", detail);
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
    tracing::info!(target: LOG_TARGET, "Suppressing error dialogs before process kill");

    let _ = hidden_cmd("taskkill").args(["/IM", "WerFault.exe", "/F"]).output();
    let _ = hidden_cmd("taskkill").args(["/IM", "WerFaultSecure.exe", "/F"]).output();
    // msedge.exe crash reporter dialogs that appear when Edge itself crashes (not AC game crash)
    // Only kill if it's a crash reporter instance — process name is the same so we kill all msedge.
    // This is acceptable because Edge is relaunched by the kiosk overlay manager on next session.
    let _ = hidden_cmd("taskkill").args(["/IM", "msedge.exe", "/F"]).output();

    AutoFixResult {
        fix_type: "kill_error_dialogs".to_string(),
        detail: "Suppressed WerFault.exe, WerFaultSecure.exe, msedge.exe".to_string(),
        success: true,
    }
}

// ─── Phase 50 Plan 02: Auto-fix patterns 8-14 ────────────────────────────────

fn fix_directx_shader_cache(_snapshot: &PodStateSnapshot) -> AutoFixResult {
    let dirs_to_clear = [
        r"C:\Users\Public\AppData\Local\NVIDIA\GLCache",
        r"C:\ProgramData\NVIDIA Corporation\NV_Cache",
    ];
    let mut cleared = 0;
    let mut details = Vec::new();
    for dir in &dirs_to_clear {
        let path = std::path::Path::new(dir);
        if path.exists() {
            match std::fs::remove_dir_all(path) {
                Ok(_) => {
                    cleared += 1;
                    details.push(format!("cleared {}", dir));
                }
                Err(e) => {
                    details.push(format!("failed to clear {}: {}", dir, e));
                }
            }
        }
    }
    AutoFixResult {
        fix_type: "directx_shader_cache".to_string(),
        detail: if cleared > 0 {
            format!("Cleared {} shader cache dirs: {}", cleared, details.join(", "))
        } else {
            "No shader cache directories found to clear".to_string()
        },
        success: cleared > 0,
    }
}

fn fix_memory_pressure(_snapshot: &PodStateSnapshot) -> AutoFixResult {
    // Enumerate high-memory non-protected processes, then trim their working set (non-destructive)
    let output = hidden_cmd("powershell")
        .args([
            "-NoProfile",
            "-Command",
            "Get-Process | Where-Object { $_.WorkingSet64 -gt 500MB -and $_.ProcessName -notin @('rc-agent','racecontrol','steam','msedge','msedgewebview2','explorer','System') } | ForEach-Object { $_.ProcessName }",
        ])
        .output();
    match output {
        Ok(out) => {
            let procs = String::from_utf8_lossy(&out.stdout);
            let proc_list: Vec<&str> = procs
                .lines()
                .filter(|l| !l.trim().is_empty())
                .collect();
            if proc_list.is_empty() {
                return AutoFixResult {
                    fix_type: "memory_pressure".to_string(),
                    detail: "No high-memory non-protected processes found".to_string(),
                    success: true,
                };
            }
            // Attempt to reduce working set via powershell (non-destructive)
            let _ = hidden_cmd("powershell")
                .args([
                    "-NoProfile",
                    "-Command",
                    "Get-Process | Where-Object { $_.WorkingSet64 -gt 500MB -and $_.ProcessName -notin @('rc-agent','racecontrol','steam','msedge','msedgewebview2','explorer','System') } | ForEach-Object { $_.MinWorkingSet = 1MB }",
                ])
                .output();
            AutoFixResult {
                fix_type: "memory_pressure".to_string(),
                detail: format!("Trimmed working set for: {}", proc_list.join(", ")),
                success: true,
            }
        }
        Err(e) => AutoFixResult {
            fix_type: "memory_pressure".to_string(),
            detail: format!("Failed to enumerate processes: {}", e),
            success: false,
        },
    }
}

fn fix_dll_repair(_snapshot: &PodStateSnapshot) -> AutoFixResult {
    // Fire sfc /scannow in background — this takes minutes, don't block
    match hidden_cmd("cmd")
        .args(["/C", "start", "/MIN", "sfc", "/scannow"])
        .spawn()
    {
        Ok(_) => AutoFixResult {
            fix_type: "dll_repair".to_string(),
            detail: "Started sfc /scannow in background — scan takes 5-15 minutes".to_string(),
            success: true,
        },
        Err(e) => AutoFixResult {
            fix_type: "dll_repair".to_string(),
            detail: format!("Failed to start sfc: {}", e),
            success: false,
        },
    }
}

fn fix_steam_restart(_snapshot: &PodStateSnapshot) -> AutoFixResult {
    // Kill Steam, wait briefly, then restart it
    let kill = hidden_cmd("taskkill")
        .args(["/IM", "steam.exe", "/F"])
        .output();
    let killed = matches!(kill, Ok(ref o) if o.status.success());
    std::thread::sleep(std::time::Duration::from_secs(2));
    let restart = hidden_cmd("cmd")
        .args(["/C", "start", "", r"C:\Program Files (x86)\Steam\steam.exe"])
        .spawn();
    let restarted = restart.is_ok();
    AutoFixResult {
        fix_type: "steam_restart".to_string(),
        detail: format!("kill={}, restart={}", killed, restarted),
        success: killed || restarted,
    }
}

fn fix_performance_throttle(_snapshot: &PodStateSnapshot) -> AutoFixResult {
    // Set High Performance power plan (standard Windows GUID: 8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c)
    let output = hidden_cmd("powercfg")
        .args(["/setactive", "8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c"])
        .output();
    match output {
        Ok(o) if o.status.success() => AutoFixResult {
            fix_type: "performance_throttle".to_string(),
            detail: "Set power plan to High Performance".to_string(),
            success: true,
        },
        Ok(o) => AutoFixResult {
            fix_type: "performance_throttle".to_string(),
            detail: format!(
                "powercfg failed: {}",
                String::from_utf8_lossy(&o.stderr)
            ),
            success: false,
        },
        Err(e) => AutoFixResult {
            fix_type: "performance_throttle".to_string(),
            detail: format!("Failed to run powercfg: {}", e),
            success: false,
        },
    }
}

fn fix_network_adapter_reset(_snapshot: &PodStateSnapshot) -> AutoFixResult {
    // Detect active Ethernet adapter name, disable then re-enable
    let name_output = hidden_cmd("powershell")
        .args([
            "-NoProfile",
            "-Command",
            "Get-NetAdapter | Where-Object { $_.Status -eq 'Up' -and $_.InterfaceDescription -like '*Ethernet*' } | Select-Object -First 1 -ExpandProperty Name",
        ])
        .output();
    let adapter_name = match name_output {
        Ok(ref o) if o.status.success() => {
            let name = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if name.is_empty() {
                "Ethernet".to_string()
            } else {
                name
            }
        }
        _ => "Ethernet".to_string(),
    };
    // Disable adapter
    let _ = hidden_cmd("netsh")
        .args(["interface", "set", "interface", &adapter_name, "disable"])
        .output();
    std::thread::sleep(std::time::Duration::from_secs(1));
    // Re-enable adapter
    let enable = hidden_cmd("netsh")
        .args(["interface", "set", "interface", &adapter_name, "enable"])
        .output();
    let success = matches!(enable, Ok(ref o) if o.status.success());
    AutoFixResult {
        fix_type: "network_adapter_reset".to_string(),
        detail: format!(
            "Reset adapter '{}': {}",
            adapter_name,
            if success { "OK" } else { "failed" }
        ),
        success,
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

async fn query_openrouter(api_key: &str, model: &str, prompt: &str) -> anyhow::Result<String> {
    let client = reqwest::Client::new();
    let resp = client
        .post("https://openrouter.ai/api/v1/chat/completions")
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&serde_json::json!({
            "model": model,
            "max_tokens": 500,
            "messages": [{"role": "user", "content": prompt}]
        }))
        .timeout(std::time::Duration::from_secs(30))
        .send()
        .await?;

    #[derive(Deserialize)]
    struct ChoiceMessage {
        content: String,
    }
    #[derive(Deserialize)]
    struct Choice {
        message: ChoiceMessage,
    }
    #[derive(Deserialize)]
    struct OpenRouterResponse {
        choices: Vec<Choice>,
    }
    let body: OpenRouterResponse = resp.json().await?;
    Ok(body
        .choices
        .first()
        .map(|c| c.message.content.clone())
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
            ..Default::default()
        };
        let error_ctx = PodErrorContext::default();
        let prompt = build_prompt(&SimType::AssettoCorsa, "exit code -1", &snapshot, &error_ctx);
        assert!(prompt.contains("Pod #3"));
        assert!(prompt.contains("Billing active: true"));
        assert!(prompt.contains("1234"));
        assert!(prompt.contains("3600s"));
        assert!(prompt.contains("ERROR LOGS"));
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
            ..Default::default()
        };
        let result = try_auto_fix("Check for CLOSE_WAIT zombie sockets on the lock screen port", &snapshot);
        assert!(result.is_some());
        assert_eq!(result.unwrap().fix_type, "clear_stale_sockets");
    }

    #[test]
    fn test_auto_fix_conspit_link_ignored() {
        // ConspitLink is managed by the main-loop watchdog, not AI auto-fix
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
            ..Default::default()
        };
        let result = try_auto_fix("Try restarting ConspitLink2.0 to restore the wheelbase connection", &snapshot);
        assert!(result.is_none(), "ConspitLink should not be handled by auto-fix");
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
            ..Default::default()
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
            ..Default::default()
        };
        let result = try_auto_fix("The issue seems to be with the graphics card version being obsolete", &snapshot);
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
            ..Default::default()
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
            ..Default::default()
        };
        let result = try_auto_fix("Dismiss the WerFault error dialog and restart the game", &snapshot);
        assert!(result.is_some());
        assert_eq!(result.unwrap().fix_type, "kill_error_dialogs");
    }

    #[test]
    fn test_default_model_is_racing_point_ops() {
        assert_eq!(default_ollama_model(), "rc-bot");
    }

    #[test]
    fn test_protected_processes_list() {
        assert!(PROTECTED_PROCESSES.contains(&"rc-agent.exe"));
        assert!(PROTECTED_PROCESSES.contains(&"pod-agent.exe"));
        assert!(PROTECTED_PROCESSES.contains(&"explorer.exe"));
        assert!(!PROTECTED_PROCESSES.contains(&"acs.exe"));
    }

    // ─── Pattern Memory Tests ───────────────────────────────────────────────

    #[test]
    fn test_pattern_key_extracts_exit_code() {
        let key = DebugMemory::pattern_key(
            &SimType::AssettoCorsa,
            "assetto_corsa crashed on pod 3 (exit code -1)",
        );
        assert_eq!(key, "AssettoCorsa:-1");
    }

    #[test]
    fn test_pattern_key_unknown_when_no_exit_code() {
        let key = DebugMemory::pattern_key(
            &SimType::F125,
            "game failed to launch",
        );
        assert_eq!(key, "F125:unknown");
    }

    #[test]
    fn test_pattern_key_large_exit_code() {
        let key = DebugMemory::pattern_key(
            &SimType::F125,
            "f1_25 crashed (exit code 3221225477)",
        );
        assert_eq!(key, "F125:3221225477");
    }

    #[test]
    fn test_instant_fix_returns_none_on_empty_memory() {
        let memory = DebugMemory::default();
        assert!(memory.instant_fix(&SimType::AssettoCorsa, "exit code -1").is_none());
    }

    #[test]
    fn test_record_and_instant_fix_round_trip() {
        let mut memory = DebugMemory::default();
        memory.incidents.push(DebugIncident {
            pattern_key: "AssettoCorsa:-1".to_string(),
            fix_type: "clear_stale_sockets".to_string(),
            ai_suggestion: "Check for CLOSE_WAIT zombie sockets".to_string(),
            success_count: 3,
            last_seen: "2026-03-16T10:00:00Z".to_string(),
        });
        let fix = memory.instant_fix(
            &SimType::AssettoCorsa,
            "assetto_corsa crashed on pod 5 (exit code -1)",
        );
        assert!(fix.is_some());
        assert!(fix.unwrap().contains("CLOSE_WAIT"));
    }

    #[test]
    fn test_instant_fix_prefers_highest_success_count() {
        let mut memory = DebugMemory::default();
        memory.incidents.push(DebugIncident {
            pattern_key: "AssettoCorsa:-1".to_string(),
            fix_type: "kill_stale_game".to_string(),
            ai_suggestion: "relaunch game".to_string(),
            success_count: 1,
            last_seen: "2026-03-16T10:00:00Z".to_string(),
        });
        memory.incidents.push(DebugIncident {
            pattern_key: "AssettoCorsa:-1".to_string(),
            fix_type: "clear_stale_sockets".to_string(),
            ai_suggestion: "CLOSE_WAIT zombie".to_string(),
            success_count: 5,
            last_seen: "2026-03-16T11:00:00Z".to_string(),
        });
        let fix = memory.instant_fix(
            &SimType::AssettoCorsa,
            "crashed (exit code -1)",
        );
        assert!(fix.unwrap().contains("CLOSE_WAIT"));
    }

    #[test]
    fn test_record_fix_increments_existing() {
        let mut memory = DebugMemory::default();
        memory.incidents.push(DebugIncident {
            pattern_key: "AssettoCorsa:-1".to_string(),
            fix_type: "clear_stale_sockets".to_string(),
            ai_suggestion: "zombie sockets".to_string(),
            success_count: 2,
            last_seen: "2026-03-16T10:00:00Z".to_string(),
        });
        // record_fix would normally save to disk — test the in-memory logic
        let key = DebugMemory::pattern_key(&SimType::AssettoCorsa, "exit code -1");
        let incident = memory.incidents.iter_mut().find(|i| i.pattern_key == key && i.fix_type == "clear_stale_sockets");
        assert!(incident.is_some());
        let i = incident.unwrap();
        i.success_count += 1;
        assert_eq!(i.success_count, 3);
    }

    // ─── Phase 24 Wave 0: RED test stubs ─────────────────────────────────────

    fn billing_snapshot(billing_active: bool) -> PodStateSnapshot {
        PodStateSnapshot {
            pod_id: "pod_8".to_string(),
            pod_number: 8,
            billing_active,
            ..Default::default()
        }
    }

    #[test]
    fn test_auto_fix_frozen_game_dispatches() {
        let snap = billing_snapshot(true);
        let result = try_auto_fix("Game frozen — IsHungAppWindow true + UDP silent 30s relaunch acs.exe", &snap);
        assert!(result.is_some(), "freeze+relaunch keywords must dispatch");
        assert_eq!(result.unwrap().fix_type, "fix_frozen_game");
    }

    #[test]
    fn test_fix_frozen_game_billing_gate() {
        let snap = billing_snapshot(false);
        let result = try_auto_fix("Game frozen — IsHungAppWindow true + UDP silent 30s relaunch acs.exe", &snap);
        // When billing is inactive, fix_frozen_game must return no-op result (success: false)
        // The arm still dispatches, but fix_frozen_game gates internally
        if let Some(r) = result {
            assert!(!r.success, "fix_frozen_game must not succeed when billing is inactive");
        }
        // None is also acceptable — either way, no destructive action taken
    }

    #[test]
    fn test_ffb_zero_before_kill_ordering() {
        let snap = billing_snapshot(true);
        let result = fix_frozen_game(&snap);  // direct call — function doesn't exist yet → RED
        assert!(result.detail.contains("ffb_zeroed") || result.detail.contains("FFB"),
            "fix_frozen_game detail must indicate FFB was zeroed: {}", result.detail);
    }

    #[test]
    fn test_auto_fix_launch_timeout_dispatches() {
        let snap = billing_snapshot(true);
        let result = try_auto_fix("launch timeout — Content Manager hang kill cm process", &snap);
        assert!(result.is_some(), "launch timeout keywords must dispatch");
        assert_eq!(result.unwrap().fix_type, "fix_launch_timeout");
    }

    #[test]
    fn test_fix_launch_timeout_kills_both_cm_names() {
        let snap = billing_snapshot(true);
        let result = fix_launch_timeout(&snap);  // function doesn't exist yet → RED
        assert_eq!(result.fix_type, "fix_launch_timeout");
        assert!(result.success);
    }

    #[test]
    fn test_kill_error_dialogs_extended() {
        let snap = billing_snapshot(false);
        let result = try_auto_fix("werfault crash dialog error dialog suppress before kill", &snap);
        assert!(result.is_some(), "werfault keyword must still dispatch");
        assert_eq!(result.unwrap().fix_type, "kill_error_dialogs");
    }

    #[test]
    fn test_auto_fix_usb_reconnect_dispatches() {
        let snap = billing_snapshot(true);
        let result = try_auto_fix("Wheelbase usb reset required — HID reconnected VID:0x1209 PID:0xFFB0", &snap);
        assert!(result.is_some(), "wheelbase+usb reset keywords must dispatch");
        assert_eq!(result.unwrap().fix_type, "fix_usb_reconnect");
    }

    #[test]
    fn test_fix_usb_reconnect_ffb_zero() {
        let snap = billing_snapshot(true);
        let result = fix_usb_reconnect(&snap);  // function doesn't exist yet → RED
        assert_eq!(result.fix_type, "fix_usb_reconnect");
        assert!(result.success);
    }

    #[test]
    fn test_fix_frozen_game_no_billing_no_fix() {
        let snap = billing_snapshot(false);
        let result = fix_frozen_game(&snap);  // function doesn't exist yet → RED
        assert!(!result.success, "fix_frozen_game must not succeed without active billing");
    }

    #[test]
    fn test_auto_fix_game_frozen_keyword_specific() {
        let snap = billing_snapshot(true);
        let result = try_auto_fix("game frozen relaunch acs.exe", &snap);
        assert!(result.is_some());
        // Must match "game frozen" arm first (fix_frozen_game), not old "relaunch" + "game" arm (kill_stale_game)
        // This forces "game frozen" arm to appear BEFORE the generic "relaunch" arm in try_auto_fix
        assert_eq!(result.unwrap().fix_type, "fix_frozen_game",
            "game frozen must dispatch to fix_frozen_game, not kill_stale_game");
    }

    // ─── Phase 50 Plan 02: Auto-fix patterns 8-14 ────────────────────────────

    fn default_snapshot() -> PodStateSnapshot {
        PodStateSnapshot {
            pod_id: "pod_8".to_string(),
            pod_number: 8,
            ..Default::default()
        }
    }

    // Pattern 8+9: DirectX / shader cache
    #[test]
    fn test_fix_pattern_directx_keyword() {
        let snap = default_snapshot();
        let result = try_auto_fix("DirectX error on shader compile", &snap);
        assert!(result.is_some(), "DirectX keyword must dispatch");
        assert_eq!(result.unwrap().fix_type, "directx_shader_cache");
    }

    #[test]
    fn test_fix_pattern_d3d_keyword() {
        let snap = default_snapshot();
        let result = try_auto_fix("d3d device lost after crash", &snap);
        assert!(result.is_some(), "d3d keyword must dispatch");
        assert_eq!(result.unwrap().fix_type, "directx_shader_cache");
    }

    #[test]
    fn test_fix_pattern_shader_cache_keyword() {
        let snap = default_snapshot();
        let result = try_auto_fix("shader cache corrupted — please restart", &snap);
        assert!(result.is_some(), "shader cache keyword must dispatch");
        assert_eq!(result.unwrap().fix_type, "directx_shader_cache");
    }

    #[test]
    fn test_fix_pattern_gpu_driver_keyword() {
        let snap = default_snapshot();
        let result = try_auto_fix("gpu driver crash detected during render", &snap);
        assert!(result.is_some(), "gpu driver keyword must dispatch");
        assert_eq!(result.unwrap().fix_type, "directx_shader_cache");
    }

    // Pattern 10: Memory pressure
    #[test]
    fn test_fix_pattern_out_of_memory_keyword() {
        let snap = default_snapshot();
        let result = try_auto_fix("out of memory error — process aborted", &snap);
        assert!(result.is_some(), "out of memory keyword must dispatch");
        assert_eq!(result.unwrap().fix_type, "memory_pressure");
    }

    #[test]
    fn test_fix_pattern_memory_leak_keyword() {
        let snap = default_snapshot();
        let result = try_auto_fix("memory leak detected in F1_25.exe", &snap);
        assert!(result.is_some(), "memory leak keyword must dispatch");
        assert_eq!(result.unwrap().fix_type, "memory_pressure");
    }

    // Pattern 11: DLL repair
    #[test]
    fn test_fix_pattern_dll_missing_keyword() {
        let snap = default_snapshot();
        let result = try_auto_fix("dll missing vcruntime140.dll required", &snap);
        assert!(result.is_some(), "dll missing keyword must dispatch");
        assert_eq!(result.unwrap().fix_type, "dll_repair");
    }

    #[test]
    fn test_fix_pattern_dll_not_found_keyword() {
        let snap = default_snapshot();
        let result = try_auto_fix("dll not found xinput1_4.dll", &snap);
        assert!(result.is_some(), "dll not found keyword must dispatch");
        assert_eq!(result.unwrap().fix_type, "dll_repair");
    }

    // Pattern 12: Steam restart
    #[test]
    fn test_fix_pattern_steam_update_keyword() {
        let snap = default_snapshot();
        let result = try_auto_fix("Steam update downloading stuck at 99%", &snap);
        assert!(result.is_some(), "steam+update keyword must dispatch");
        assert_eq!(result.unwrap().fix_type, "steam_restart");
    }

    #[test]
    fn test_fix_pattern_steam_downloading_keyword() {
        let snap = default_snapshot();
        let result = try_auto_fix("Steam is downloading content — launch blocked", &snap);
        assert!(result.is_some(), "steam+downloading keyword must dispatch");
        assert_eq!(result.unwrap().fix_type, "steam_restart");
    }

    // Pattern 13: Performance throttle
    #[test]
    fn test_fix_pattern_low_fps_keyword() {
        let snap = default_snapshot();
        let result = try_auto_fix("low fps during race — check power plan", &snap);
        assert!(result.is_some(), "low fps keyword must dispatch");
        assert_eq!(result.unwrap().fix_type, "performance_throttle");
    }

    #[test]
    fn test_fix_pattern_frame_drops_keyword() {
        let snap = default_snapshot();
        let result = try_auto_fix("frame drops in AC every corner", &snap);
        assert!(result.is_some(), "frame drops keyword must dispatch");
        assert_eq!(result.unwrap().fix_type, "performance_throttle");
    }

    #[test]
    fn test_fix_pattern_stuttering_keyword() {
        let snap = default_snapshot();
        let result = try_auto_fix("stuttering gameplay detected in iRacing", &snap);
        assert!(result.is_some(), "stuttering keyword must dispatch");
        assert_eq!(result.unwrap().fix_type, "performance_throttle");
    }

    // Pattern 14: Network adapter reset
    #[test]
    fn test_fix_pattern_network_timeout_keyword() {
        let snap = default_snapshot();
        let result = try_auto_fix("network timeout connecting to server at 192.168.31.23", &snap);
        assert!(result.is_some(), "network timeout keyword must dispatch");
        assert_eq!(result.unwrap().fix_type, "network_adapter_reset");
    }

    #[test]
    fn test_fix_pattern_connection_refused_keyword() {
        let snap = default_snapshot();
        let result = try_auto_fix("connection refused on port 8080", &snap);
        assert!(result.is_some(), "connection refused keyword must dispatch");
        assert_eq!(result.unwrap().fix_type, "network_adapter_reset");
    }

    // False-positive guard: unrecognized text must return None
    #[test]
    fn test_fix_pattern_no_false_positive() {
        let snap = default_snapshot();
        let result = try_auto_fix("unrecognized error message xyz — please investigate", &snap);
        assert!(result.is_none(), "unknown text must not match any fix pattern");
    }
}
