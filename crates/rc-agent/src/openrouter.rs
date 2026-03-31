//! OpenRouter API client — 5-model diagnostic system for Meshed Intelligence.
//!
//! Tier 3: Single model diagnosis (Qwen3 ~$0.05)
//! Tier 4: 5-model parallel diagnosis (Qwen3+R1+V3+MiMo+Gemini ~$4)
//!
//! Each model has a role-specific system prompt trained from MMA audit methodologies:
//!   - Qwen3 235B: Scanner — exhaustive enumeration, volume coverage
//!   - DeepSeek R1: Reasoner — absence detection, state machine stuck states, logic bugs
//!   - DeepSeek V3: Code Expert — Rust/Windows code patterns, Session 0/1, process lifecycle
//!   - MiMo v2 Pro: SRE — operational gaps, stuck states, idempotency, "3am failures"
//!   - Gemini 2.5 Pro: Security — credential scanning, auth checklists, config errors
//!
//! API key is read from OPENROUTER_KEY env var — NEVER hardcoded.
//! Standing rules: no .unwrap() in production, all errors propagated via anyhow.

use serde::{Deserialize, Serialize};

const LOG_TARGET: &str = "openrouter";

/// OpenRouter API endpoint
const OPENROUTER_API_URL: &str = "https://openrouter.ai/api/v1/chat/completions";

/// Max retries for transient errors (429, 5xx)
const MAX_RETRIES: u32 = 4;
/// Base delay for exponential backoff (1 second)
const BASE_DELAY_MS: u64 = 1000;
/// Max backoff cap (10 seconds)
const MAX_DELAY_MS: u64 = 10_000;
/// Per-attempt timeout (30s) — total job timeout handled by caller
const PER_ATTEMPT_TIMEOUT_SECS: u64 = 30;

/// Tier 4 concurrency limiter — max 2 parallel Tier 4 diagnostic jobs per pod.
/// Prevents thundering herd when all 8 pods fire simultaneously.
static TIER4_SEMAPHORE: std::sync::LazyLock<tokio::sync::Semaphore> =
    std::sync::LazyLock::new(|| tokio::sync::Semaphore::new(2));

/// Model registry — version-pinned OpenRouter model IDs
#[derive(Debug, Clone)]
pub struct ModelConfig {
    pub id: &'static str,
    pub role: &'static str,
    pub system_prompt: &'static str,
}

/// Racing Point fleet context shared across all model prompts.
/// Encodes domain knowledge so models can diagnose with fleet-specific understanding.
const FLEET_CONTEXT: &str = "\
FLEET CONTEXT: Racing Point eSports — 8 Windows 11 sim racing pods running rc-agent (Rust/Axum :8090). \
Server at .23:8080 (racecontrol). WebSocket for state sync. SQLite billing. \
Key processes: rc-agent (pod agent), rc-sentry (watchdog), ConspitLink.exe (steering wheel HID), Edge (lock screen/kiosk). \
Known anomaly classes: health_check_fail, process_crash, game_launch_fail, display_mismatch, error_spike, \
ws_disconnect, sentinel_unexpected, violation_spike, preflight_failed. \
Critical sentinels: MAINTENANCE_MODE (blocks all restarts), OTA_DEPLOYING (blocks recovery during deploy), \
RCAGENT_SELF_RESTART (graceful restart). \
Session context: rc-agent MUST run in Session 1 (interactive desktop). Session 0 = all GUI ops fail silently. \
Recovery systems: rc-sentry watchdog, RCWatchdog Windows service, server pod_monitor, WoL auto-wake. \
These can fight each other — check for cascade conflicts. \
Process guard: allowlist-based, fetched from server at boot + every 5min. Empty allowlist = block everything. \
Known failure patterns: MAINTENANCE_MODE stuck (no TTL), ConspitLink multiplication, orphan PowerShell from self-restart, \
Edge blanking screen showing blanked state but edge_process_count=0, budget tracker not persisted across restarts. \
GAME LAUNCH (REVENUE-CRITICAL): AC launch sequence: kill existing → write race.ini → spawn acs.exe or Content Manager → wait 30s for process. \
Known launch failures: orphan acs.exe blocking new instance, Content Manager hung on error dialog, stale game.pid, \
missing FORCE_START=1 in gui.ini, car/track not installed, Steam not running, race.ini corrupted or missing sections, \
serde field mismatch (kiosk sends ai_difficulty string but agent expects ai_level u32 — silently ignored), \
CM shows 'Settings are not specified' when Quick Drive preset never configured on pod. \
COGNITIVE GATES: All diagnoses must follow Cognitive Gate Protocol v2.0 — \
G1: verify exact behavior path not proxies, G5: consider 2+ competing hypotheses, \
G8: check downstream cascade impact, G4: confidence reflects verified vs assumed.";

/// The 5 models used for diagnosis — trained from MMA audit methodologies
pub const MODELS: [ModelConfig; 5] = [
    // ── Scanner: Qwen3 235B — exhaustive enumeration, volume coverage ──
    // MMA proven: 139 findings at $0.05. Best for: volume scanning, catching duplicates,
    // broad surface-area coverage. Weaknesses: duplicate findings, may restate obvious.
    ModelConfig {
        id: "qwen/qwen3-235b-a22b-2507",
        role: "Scanner",
        system_prompt: "You are a high-volume diagnostic scanner for a Racing Point sim racing pod fleet. \
            Your method: EXHAUSTIVE ENUMERATION. List every possible cause, then rank by likelihood. \
            Check ALL of these in order: \
            (1) Sentinel files (MAINTENANCE_MODE, OTA_DEPLOYING, FORCE_CLEAN, SAFE_MODE) — stuck or stale? \
            (2) Process state — is the expected process running? Is it multiplied? Is it in the right Session (1, not 0)? \
            (3) Network — WebSocket connected? Server reachable? DNS resolving? Port exhaustion (CLOSE_WAIT)? \
            (4) Filesystem — disk space? Log rotation working? Config file corrupted? Stale temp files? \
            (5) Resource pressure — RAM, CPU, handle count. Orphan PowerShell processes leaking ~90MB each? \
            (6) Configuration — allowlist empty? TOML parse error? Feature flags stale from boot-time fetch? \
            (7) Recovery conflicts — are multiple recovery systems (rc-sentry, RCWatchdog, pod_monitor, WoL) fighting? \
            Be concise. Output ONLY valid JSON: {\"root_cause\": \"...\", \"confidence\": 0.0-1.0, \
            \"fix_action\": \"...\", \"risk_level\": \"safe|caution|dangerous\"}",
    },
    // ── Reasoner: DeepSeek R1 — absence detection, state machine analysis ──
    // MMA proven: 46 findings at $0.43. Best for: "what SHOULD be here but isn't",
    // state machine stuck states, logic bugs, process lifecycle reasoning, timing/race conditions.
    ModelConfig {
        id: "deepseek/deepseek-r1-0528",
        role: "Reasoner",
        system_prompt: "You are a reasoning-focused debugger for Racing Point sim racing pods. \
            Your method: ABSENCE-BASED ANALYSIS + STATE MACHINE REASONING. \
            Step 1 — ABSENCE CHECK: What SHOULD exist but doesn't? Common absences: \
            - Missing TTL on MAINTENANCE_MODE (pod stuck forever with no timeout or auto-clear) \
            - Missing heartbeat for diagnostic engine itself (could silently fail) \
            - Missing session isolation between customers (information leakage) \
            - Missing game process resource limits (one game eats all RAM) \
            - Budget tracker not persisted (monthly spend lost on restart) \
            - Missing stuck-state detection for wheel inputs (false input reporting) \
            - Dual state variables (safe_mode.active AND safe_mode_active atomic) that can desync \
            Step 2 — STATE MACHINE: Trace the state transitions. Where can it get stuck? \
            - GameTracker: Idle→Launching→Running→Stopping. Can Launching get stuck if WS drops mid-launch? \
            - Lock screen: screen_blanked state set but Edge never spawned (edge_process_count=0) \
            - Billing guard: session started but auto-end notification dropped by try_send() on full channel \
            - Self-restart: MAINTENANCE_MODE written after 3 restarts, but no one clears it \
            Step 3 — TIMING: Race conditions between concurrent checks, PID reuse between verify and kill. \
            Use chain-of-thought reasoning. Output ONLY valid JSON: {\"root_cause\": \"...\", \"confidence\": 0.0-1.0, \
            \"fix_action\": \"...\", \"risk_level\": \"safe|caution|dangerous\"}",
    },
    // ── Code Expert: DeepSeek V3 — Rust/Windows code patterns, Session 0/1 ──
    // MMA proven: 17 high-quality findings at $0.16. Best for: Rust code patterns,
    // Session 0/1 detection, Windows-specific bugs, PID reuse, process spawning.
    ModelConfig {
        id: "deepseek/deepseek-chat-v3-0324",
        role: "Code Expert",
        system_prompt: "You are a code-level debugger specializing in Rust/Axum on Windows for Racing Point pods. \
            Your method: CODE PATH TRACING + WINDOWS-SPECIFIC PATTERN MATCHING. \
            Trace the likely code path from trigger to failure. Key patterns to check: \
            (1) SESSION 0/1: rc-agent MUST be in Session 1 (interactive). If launched by schtasks or \
            Windows service, it runs in Session 0 — Edge, game launch, keyboard hooks ALL fail silently. \
            Check: tasklist /V shows Console (Session 1) vs Services (Session 0). \
            (2) PROCESS SPAWNING: .spawn().is_ok() does NOT mean the child started on Windows. \
            CreateProcess accepted ≠ process alive. Must verify child process after spawn. \
            (3) PID REUSE: Windows recycles PIDs aggressively. TOCTOU between is_whitelisted and kill. \
            (4) CMD.EXE HOSTILITY: Any command via /exec goes through cmd /C. Spaces, $, \", \\ get mangled. \
            Use PID-based targeting, write bat files, or use Win32 APIs instead. \
            (5) SERDE SILENT DROPS: Missing deny_unknown_fields on boundary structs. Frontend sends \
            ai_difficulty (string) but agent expects ai_level (u32) — silently ignored, game launches with wrong config. \
            (6) LOCALE-DEPENDENT PARSING: netstat output, tasklist output assume English locale. \
            (7) POWERSHELL LEAK: self_monitor relaunch_self uses PowerShell+DETACHED_PROCESS, leaks ~90MB per restart. \
            Output ONLY valid JSON: {\"root_cause\": \"...\", \"confidence\": 0.0-1.0, \
            \"fix_action\": \"...\", \"risk_level\": \"safe|caution|dangerous\"}",
    },
    // ── SRE: MiMo v2 Pro — operational gaps, stuck states, idempotency ──
    // MMA proven: 48 findings at $0.77. Best for: "what breaks at 3am", operational completeness,
    // timeout gaps, idempotency failures, thundering herd, resource exhaustion patterns.
    // This model was MISSING from the original RC Doctor — adding it fills the SRE gap.
    ModelConfig {
        id: "xiaomi/mimo-v2-pro",
        role: "SRE",
        system_prompt: "You are an SRE-focused diagnostician for Racing Point's 8-pod sim racing fleet. \
            Your method: OPERATIONAL FAILURE MODE ANALYSIS — think 'what breaks at 3am with no one watching'. \
            Key SRE checks: \
            (1) STUCK STATES: Is anything waiting forever? MAINTENANCE_MODE with no TTL. GameTracker in Launching \
            with no timeout. Billing session that never auto-ended. WS reconnect backoff that hit ceiling and stopped. \
            (2) RESOURCE EXHAUSTION: TCP port exhaustion (CLOSE_WAIT accumulation on :8090). Handle leak from \
            spawned processes. Disk filling from log files without rotation. SQLite WAL growing unbounded. \
            Orphan PowerShell processes at ~90MB each from self-restart leaks. \
            (3) IDEMPOTENCY FAILURES: Is the fix safe to apply twice? Sentinel deletion is safe. SSH key \
            append without dedup creates duplicates. Process kill without PID verification hits wrong process. \
            (4) THUNDERING HERD: All 8 pods hitting server simultaneously after outage recovery. \
            No jitter on retry delays. All pods fetching allowlist at the same 5-min interval. \
            (5) RECOVERY CASCADE: rc-sentry restarts rc-agent. RCWatchdog also restarts it. Server pod_monitor \
            sends WoL. If all three fire at once: multiple rc-agent instances, port conflicts, crash loop → \
            MAINTENANCE_MODE → pod permanently down. \
            (6) MONITORING GAPS: Is the monitor itself being monitored? Is the diagnostic engine alive? \
            Is budget tracker tracking? Is the circuit breaker stuck open? \
            (7) GRACEFUL DEGRADATION: What works when the server is down? Feature flags stale? \
            Allowlist empty? Billing orphaned? Does the pod remain usable? \
            Output ONLY valid JSON: {\"root_cause\": \"...\", \"confidence\": 0.0-1.0, \
            \"fix_action\": \"...\", \"risk_level\": \"safe|caution|dangerous\"}",
    },
    // ── Security: Gemini 2.5 Pro — credential scanning, auth checklists ──
    // MMA proven: 84 findings at $1.65. Best for: security checklists, credential scanning,
    // auth gaps on endpoints, config errors. Weaknesses: stale training data (may flag valid Rust 2024 edition).
    ModelConfig {
        id: "google/gemini-2.5-pro-preview-03-25",
        role: "Security",
        system_prompt: "You are a security auditor for Racing Point's sim racing pod fleet. \
            Your method: SECURITY CHECKLIST + CONFIG VALIDATION. \
            Systematic checks: \
            (1) CREDENTIALS: Is ws_secret in plaintext in rc-agent.toml? OPENROUTER_KEY exposed? \
            SSH keys with wrong permissions? Passwords in log output? \
            (2) AUTH GAPS: Debug server on 0.0.0.0:18924 with no auth (screenshot endpoint = privacy breach). \
            Process guard whitelist matching by name only (not full path — name spoofing bypasses it). \
            (3) INJECTION: Command injection via game launch URL (cmd /C start with unsanitized args). \
            Registry key name injection in parse_run_key_entries. Path traversal in sentinel deletion. \
            (4) CONFIG ERRORS: TOML parse failure falling back to empty defaults silently. \
            SSH banner lines prepended to config files from piped ssh output. \
            process_guard enabled with empty allowlist = blocks everything. \
            (5) SENTINEL CORRUPTION: MAINTENANCE_MODE left from previous crash storm. \
            OTA_DEPLOYING never cleared after failed deploy. FORCE_CLEAN orphaned. \
            (6) NETWORK: Firewall rules that don't persist after reboot. \
            Steam directory path-based process guard exclusion (any exe in Steam dir runs freely). \
            (7) FILE PERMISSIONS: authorized_keys without strict ACLs. temp screenshot file race condition. \
            NOTE: Tailscale connections use ws:// not wss:// by design (tunnel is already encrypted). \
            NOTE: ALLOWED_BINARIES includes fleet ops commands by design. \
            Output ONLY valid JSON: {\"root_cause\": \"...\", \"confidence\": 0.0-1.0, \
            \"fix_action\": \"...\", \"risk_level\": \"safe|caution|dangerous\"}",
    },
];

/// Structured response from an OpenRouter model diagnosis.
/// MMA-First Protocol (v31.0) adds permanent_fix, verification, and fix_type.
/// These are optional for backward compatibility with Tier 3 single-model calls.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosisResult {
    pub root_cause: String,
    pub confidence: f64,
    pub fix_action: String,
    pub risk_level: String,
    /// MMA-First: What prevents this issue from recurring (permanent solution).
    /// "restart" is never acceptable here — must be a root cause fix.
    #[serde(default)]
    pub permanent_fix: Option<String>,
    /// MMA-First: How to verify the fix worked.
    #[serde(default)]
    pub verification: Option<String>,
    /// MMA-First: Classification for routing — "deterministic", "config", "code_change", "hardware".
    /// Determines auto-apply vs escalate to human.
    #[serde(default)]
    pub fix_type_class: Option<String>,
}

/// Response from a single model call
#[derive(Debug, Clone)]
pub struct ModelResponse {
    pub model_id: String,
    #[allow(dead_code)]
    pub role: String,
    pub diagnosis: Option<DiagnosisResult>,
    #[allow(dead_code)]
    pub raw_text: String,
    pub cost_estimate: f64,
    pub error: Option<String>,
}

/// OpenRouter chat completion response (subset we need)
#[derive(Debug, Deserialize)]
struct ChatResponse {
    choices: Option<Vec<ChatChoice>>,
    usage: Option<ChatUsage>,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    message: ChatMessage,
}

#[derive(Debug, Deserialize)]
struct ChatMessage {
    content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ChatUsage {
    prompt_tokens: Option<u64>,
    completion_tokens: Option<u64>,
}

/// Get the OpenRouter API key from environment.
/// Returns None if not set — caller should skip model calls gracefully.
pub fn get_api_key() -> Option<String> {
    std::env::var("OPENROUTER_KEY").ok().filter(|k| !k.is_empty())
}

/// Classify HTTP status into retry strategy.
#[derive(Debug, PartialEq)]
enum ErrorClass {
    /// 401/403 — auth/permission error, never retry
    Auth,
    /// 402 — out of credits, never retry
    OutOfCredits,
    /// 429 — rate limited, retry with backoff (honor Retry-After)
    RateLimited,
    /// 500-599 — transient upstream error, retry limited times
    ServerError,
    /// Other 4xx — client error, don't retry
    ClientError,
}

fn classify_error(status: u16) -> ErrorClass {
    match status {
        401 | 403 => ErrorClass::Auth,
        402 => ErrorClass::OutOfCredits,
        429 => ErrorClass::RateLimited,
        500..=599 => ErrorClass::ServerError,
        _ => ErrorClass::ClientError,
    }
}

/// Compute jittered backoff delay.
/// Base: 1s, 2s, 4s, 8s... capped at MAX_DELAY_MS. Jitter: ±25%.
fn backoff_delay(attempt: u32) -> std::time::Duration {
    use rand::Rng;
    let base = BASE_DELAY_MS.saturating_mul(1u64 << attempt).min(MAX_DELAY_MS);
    let jitter_range = base / 4; // ±25%
    let jitter = rand::thread_rng().gen_range(0..=jitter_range * 2);
    let delay = base.saturating_sub(jitter_range).saturating_add(jitter);
    std::time::Duration::from_millis(delay)
}

/// Parse Retry-After header value (seconds or HTTP date → seconds).
fn parse_retry_after(headers: &reqwest::header::HeaderMap) -> Option<std::time::Duration> {
    let val = headers.get("retry-after")?.to_str().ok()?;
    // Try parsing as seconds first
    if let Ok(secs) = val.parse::<u64>() {
        return Some(std::time::Duration::from_secs(secs.min(30)));
    }
    // Could parse HTTP date here, but seconds is the common case for OpenRouter
    None
}

/// Call a single OpenRouter model with diagnostic symptoms.
/// Retries transient errors (429, 5xx) with jittered exponential backoff.
/// Honors Retry-After header. Aborts immediately on auth/credit errors.
pub async fn call_model(
    client: &reqwest::Client,
    api_key: &str,
    model: &ModelConfig,
    symptoms: &str,
) -> ModelResponse {
    let request_body = serde_json::json!({
        "model": model.id,
        "messages": [
            {"role": "system", "content": model.system_prompt},
            {"role": "user", "content": symptoms}
        ],
        "max_tokens": 500,
        "temperature": 0.1
    });

    let mut last_error = String::new();

    for attempt in 0..=MAX_RETRIES {
        if attempt > 0 {
            tracing::info!(
                target: LOG_TARGET,
                model = model.id,
                attempt = attempt,
                "Retrying OpenRouter call"
            );
        } else {
            tracing::debug!(
                target: LOG_TARGET,
                model = model.id,
                role = model.role,
                "Calling OpenRouter model"
            );
        }

        let result = client
            .post(OPENROUTER_API_URL)
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .header("HTTP-Referer", "https://racingpoint.in")
            .header("X-Title", "Racing Point Mesh Intelligence")
            .json(&request_body)
            .timeout(std::time::Duration::from_secs(PER_ATTEMPT_TIMEOUT_SECS))
            .send()
            .await;

        let response = match result {
            Ok(resp) => resp,
            Err(e) => {
                last_error = format!("Request failed: {}", e);
                if e.is_timeout() && attempt < MAX_RETRIES {
                    // Timeout is retryable
                    tracing::warn!(target: LOG_TARGET, model = model.id, attempt, "OpenRouter timeout — retrying");
                    tokio::time::sleep(backoff_delay(attempt)).await;
                    continue;
                }
                tracing::warn!(target: LOG_TARGET, model = model.id, error = %e, "OpenRouter request failed (final)");
                return ModelResponse {
                    model_id: model.id.to_string(),
                    role: model.role.to_string(),
                    diagnosis: None,
                    raw_text: String::new(),
                    cost_estimate: 0.0,
                    error: Some(last_error),
                };
            }
        };

        let status = response.status();
        let status_code = status.as_u16();

        if !status.is_success() {
            let error_class = classify_error(status_code);
            let retry_after = parse_retry_after(response.headers());
            let body = response.text().await.unwrap_or_default();
            last_error = format!("HTTP {}: {}", status_code, &body[..body.len().min(200)]);

            match error_class {
                ErrorClass::Auth => {
                    tracing::error!(
                        target: LOG_TARGET, model = model.id, status = status_code,
                        "OpenRouter auth error — check OPENROUTER_KEY (not retrying)"
                    );
                    return ModelResponse {
                        model_id: model.id.to_string(),
                        role: model.role.to_string(),
                        diagnosis: None, raw_text: body, cost_estimate: 0.0,
                        error: Some(last_error),
                    };
                }
                ErrorClass::OutOfCredits => {
                    tracing::error!(
                        target: LOG_TARGET, model = model.id,
                        "OpenRouter out of credits (402) — not retrying"
                    );
                    return ModelResponse {
                        model_id: model.id.to_string(),
                        role: model.role.to_string(),
                        diagnosis: None, raw_text: body, cost_estimate: 0.0,
                        error: Some(last_error),
                    };
                }
                ErrorClass::RateLimited if attempt < MAX_RETRIES => {
                    let delay = retry_after.unwrap_or_else(|| backoff_delay(attempt));
                    tracing::warn!(
                        target: LOG_TARGET, model = model.id, attempt,
                        delay_ms = delay.as_millis() as u64,
                        "OpenRouter 429 rate limited — backing off"
                    );
                    tokio::time::sleep(delay).await;
                    continue;
                }
                ErrorClass::ServerError if attempt < MAX_RETRIES => {
                    let delay = retry_after.unwrap_or_else(|| backoff_delay(attempt));
                    tracing::warn!(
                        target: LOG_TARGET, model = model.id, status = status_code, attempt,
                        "OpenRouter server error — retrying"
                    );
                    tokio::time::sleep(delay).await;
                    continue;
                }
                _ => {
                    tracing::warn!(
                        target: LOG_TARGET, model = model.id, status = status_code,
                        "OpenRouter error (not retrying)"
                    );
                    return ModelResponse {
                        model_id: model.id.to_string(),
                        role: model.role.to_string(),
                        diagnosis: None, raw_text: body, cost_estimate: 0.0,
                        error: Some(last_error),
                    };
                }
            }
        }

        // ── Success path ──
        let body = response.text().await.unwrap_or_default();

        let chat_resp: ChatResponse = match serde_json::from_str(&body) {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(target: LOG_TARGET, model = model.id, error = %e, "Failed to parse OpenRouter response");
                return ModelResponse {
                    model_id: model.id.to_string(),
                    role: model.role.to_string(),
                    diagnosis: None,
                    raw_text: body,
                    cost_estimate: 0.0,
                    error: Some(format!("Parse error: {}", e)),
                };
            }
        };

        let raw_text = chat_resp
            .choices
            .as_ref()
            .and_then(|c| c.first())
            .and_then(|c| c.message.content.as_ref())
            .cloned()
            .unwrap_or_default();

        let cost_estimate = chat_resp.usage.as_ref().map_or(0.0, |u| {
            let prompt = u.prompt_tokens.unwrap_or(0) as f64;
            let completion = u.completion_tokens.unwrap_or(0) as f64;
            (prompt * 0.5 + completion * 1.5) / 1_000_000.0
        });

        let diagnosis = extract_diagnosis(&raw_text);

        tracing::info!(
            target: LOG_TARGET,
            model = model.id,
            role = model.role,
            has_diagnosis = diagnosis.is_some(),
            cost = cost_estimate,
            retries = attempt,
            "OpenRouter model responded"
        );

        return ModelResponse {
            model_id: model.id.to_string(),
            role: model.role.to_string(),
            diagnosis,
            raw_text,
            cost_estimate,
            error: None,
        };
    }

    // Exhausted all retries
    tracing::error!(
        target: LOG_TARGET, model = model.id,
        retries = MAX_RETRIES,
        "OpenRouter call failed after all retries"
    );
    ModelResponse {
        model_id: model.id.to_string(),
        role: model.role.to_string(),
        diagnosis: None,
        raw_text: String::new(),
        cost_estimate: 0.0,
        error: Some(last_error),
    }
}

/// Tier 3: Call single cheapest model (Qwen3) for diagnosis.
pub async fn tier3_diagnose(client: &reqwest::Client, api_key: &str, symptoms: &str) -> ModelResponse {
    call_model(client, api_key, &MODELS[0], symptoms).await
}

/// Tier 4: Call all 5 models in parallel, gated by concurrency semaphore.
/// Max 2 Tier 4 jobs can run concurrently per pod — prevents thundering herd
/// when all 8 pods fire diagnostics simultaneously (would be 40+ parallel requests).
pub async fn tier4_diagnose_parallel(
    client: &reqwest::Client,
    api_key: &str,
    symptoms: &str,
) -> Vec<ModelResponse> {
    // Acquire semaphore permit — blocks if 2 Tier 4 jobs already in flight
    let _permit = match TIER4_SEMAPHORE.acquire().await {
        Ok(p) => p,
        Err(_) => {
            tracing::error!(target: LOG_TARGET, "Tier 4 semaphore closed — skipping");
            return vec![];
        }
    };

    let futures: Vec<_> = MODELS
        .iter()
        .map(|model| call_model(client, api_key, model, symptoms))
        .collect();

    futures_util::future::join_all(futures).await
    // _permit dropped here — releases slot for next Tier 4 job
}

/// Find consensus among multiple model responses.
/// Groups diagnoses by root-cause similarity and returns the best-supported diagnosis.
///
/// MMA-trained consensus logic (replaces naive highest-confidence):
/// 1. Extract keyword tokens from each root_cause
/// 2. Group diagnoses that share 2+ keywords (same underlying issue)
/// 3. Pick the largest group (most models agree on the same root cause)
/// 4. Within that group, return the diagnosis with highest confidence
/// 5. If no group has 2+ members, fall back to highest-confidence single diagnosis (>= 0.7)
pub fn find_consensus(responses: &[ModelResponse]) -> Option<DiagnosisResult> {
    let diagnoses: Vec<&DiagnosisResult> = responses
        .iter()
        .filter_map(|r| r.diagnosis.as_ref())
        .collect();

    if diagnoses.is_empty() {
        return None;
    }

    // Single diagnosis — use if high confidence
    if diagnoses.len() == 1 {
        let d = diagnoses[0];
        if d.confidence >= 0.7 {
            return Some(d.clone());
        }
        return None;
    }

    // Extract keyword tokens from root_cause for similarity matching (using HashSet for O(1) lookups)
    use std::collections::HashSet;
    let tokenize = |s: &str| -> HashSet<String> {
        s.to_lowercase()
            .split(|c: char| !c.is_alphanumeric() && c != '_')
            .filter(|w| w.len() >= 3) // skip short words like "a", "is", "to"
            .filter(|w| !matches!(*w, "the" | "and" | "for" | "with" | "from" | "that" | "this" | "not"))
            .map(|w| w.to_string())
            .collect()
    };

    let token_sets: Vec<HashSet<String>> = diagnoses.iter().map(|d| tokenize(&d.root_cause)).collect();

    // Union-Find for transitive grouping: if A~B and B~C, then A,B,C are in one group.
    // This fixes the greedy non-transitive bug where order-dependent assignment could
    // split a true majority across two groups.
    let n = diagnoses.len();
    let mut parent: Vec<usize> = (0..n).collect();
    let find = |parent: &mut Vec<usize>, mut x: usize| -> usize {
        while parent[x] != x {
            parent[x] = parent[parent[x]]; // path compression
            x = parent[x];
        }
        x
    };

    // Build edges: pair (i,j) if they share 2+ keyword tokens
    for i in 0..n {
        for j in (i + 1)..n {
            let shared = token_sets[i].intersection(&token_sets[j]).count();
            if shared >= 2 {
                let ri = find(&mut parent, i);
                let rj = find(&mut parent, j);
                if ri != rj {
                    parent[ri] = rj; // union
                }
            }
        }
    }

    // Collect groups by root
    let mut group_map: std::collections::HashMap<usize, Vec<usize>> = std::collections::HashMap::new();
    for i in 0..n {
        let root = find(&mut parent, i);
        group_map.entry(root).or_default().push(i);
    }
    let mut groups: Vec<Vec<usize>> = group_map.into_values().collect();

    // Find the largest group (most models agreeing on similar root cause)
    groups.sort_by(|a, b| b.len().cmp(&a.len()));

    if let Some(best_group) = groups.first() {
        if best_group.len() >= 2 {
            // True consensus — 2+ models agree. Pick highest confidence within group.
            let best = best_group.iter()
                .map(|&idx| diagnoses[idx])
                .max_by(|a, b| a.confidence.partial_cmp(&b.confidence).unwrap_or(std::cmp::Ordering::Equal));
            if let Some(d) = best {
                tracing::info!(
                    target: "openrouter",
                    consensus_size = best_group.len(),
                    root_cause = %d.root_cause,
                    confidence = d.confidence,
                    "Consensus found: {} models agree",
                    best_group.len()
                );
                return Some(d.clone());
            }
        }
    }

    // No true consensus — fall back to highest confidence if >= 0.7
    diagnoses
        .iter()
        .filter(|d| d.confidence >= 0.7)
        .max_by(|a, b| a.confidence.partial_cmp(&b.confidence).unwrap_or(std::cmp::Ordering::Equal))
        .map(|d| (*d).clone())
}

/// Format a DiagnosticEvent into a symptom string for model prompts.
/// Includes fleet context so models can diagnose with domain-specific understanding.
pub fn format_symptoms(
    trigger: &str,
    problem_key: &str,
    environment: &str,
    pod_state_summary: &str,
) -> String {
    format!(
        "{}\n\n\
         --- DIAGNOSTIC EVENT ---\n\
         Trigger: {}\n\
         Problem Key: {}\n\
         Environment: {}\n\
         Pod State: {}\n\n\
         Analyze this event using your specialized methodology. \
         Consider: sentinel files, process state, session context, recovery cascade, resource exhaustion. \
         What is the root cause and what is the safest fix action?\n\n\
         --- MMA-FIRST PROTOCOL (v31.0) ---\n\
         CRITICAL: You MUST provide ALL of these in your JSON response:\n\
         1. root_cause — the ACTUAL cause, not symptoms, not \"restart fixed it\"\n\
         2. fix_action — what to do NOW (may be a workaround)\n\
         3. permanent_fix — what PREVENTS recurrence. \"Restart\" is NOT acceptable.\n\
            If restarting fixes it, explain WHY restarting fixes it and what config/code change prevents it.\n\
         4. verification — how to CONFIRM the fix worked\n\
         5. fix_type_class — one of: \"deterministic\" (auto-apply), \"config\" (auto-apply), \
            \"code_change\" (escalate to James), \"hardware\" (alert staff)\n\
         6. confidence — your confidence 0.0-1.0\n\
         7. risk_level — \"safe\" or \"caution\" or \"dangerous\"\n\n\
         --- COGNITIVE GATE PROTOCOL v2.0 ---\n\
         Before finalizing your diagnosis, apply these mandatory checks:\n\
         G1 OUTCOME: Your root_cause must name the EXACT broken behavior path, not a proxy metric. \
         \"health returns 200\" is NOT a root cause — trace the actual failure path.\n\
         G5 COMPETING HYPOTHESES: You MUST consider at least 2 competing root causes before settling on one. \
         If you can only think of one cause, your analysis is incomplete.\n\
         G8 DEPENDENCY CASCADE: Your fix_action must account for downstream consumers. \
         Will this fix break anything else? List affected systems in your root_cause analysis.\n\
         G4 CONFIDENCE: Your confidence score must reflect what you ACTUALLY verified vs assumed. \
         If your diagnosis is based on pattern-matching rather than trace-level reasoning, cap confidence at 0.6.\n\n\
         Output ONLY valid JSON with these fields.",
        FLEET_CONTEXT, trigger, problem_key, environment, pod_state_summary
    )
}

/// Enrich symptom string with trigger-specific context bundle.
/// Called before MMA diagnosis to provide models with rich, structured data
/// instead of the generic `{:?}` trigger dump.
pub fn enrich_with_context_bundle(
    base_symptoms: &str,
    trigger: &crate::diagnostic_engine::DiagnosticTrigger,
    pod_state: &crate::failure_monitor::FailureMonitorState,
) -> String {
    use crate::diagnostic_engine::DiagnosticTrigger;

    let context = match trigger {
        DiagnosticTrigger::GameMidSessionCrash { exit_code, session_duration_secs } => {
            format!(
                "--- CONTEXT BUNDLE: GAME MID-SESSION CRASH ---\n\
                 Exit code: {:?}\n\
                 Session duration at crash: {}s\n\
                 Billing was active: {}\n\
                 This is revenue-critical — customer was in the middle of a paid session.",
                exit_code, session_duration_secs, pod_state.billing_active
            )
        }
        DiagnosticTrigger::PostSessionAnalysis { session_quality_pct } => {
            format!(
                "--- CONTEXT BUNDLE: POST-SESSION ANALYSIS ---\n\
                 Session quality score: {}%\n\
                 This is a lightweight analysis — only escalate if quality < 70%.",
                session_quality_pct
            )
        }
        DiagnosticTrigger::PreShiftAudit => {
            "--- CONTEXT BUNDLE: PRE-SHIFT AUDIT ---\n\
             This is a comprehensive morning health check before venue opens.\n\
             Check ALL systems: GPU temp, disk space, network, process state, config freshness.\n\
             Identify any drift from baseline healthy state."
                .to_string()
        }
        DiagnosticTrigger::DeployVerification { new_build_id } => {
            format!(
                "--- CONTEXT BUNDLE: DEPLOY VERIFICATION ---\n\
                 New build_id: {}\n\
                 Verify: binary running, health endpoint correct build_id, \
                 no new crashes in first 60s, all endpoints responding.",
                new_build_id
            )
        }
        DiagnosticTrigger::GameLaunchFail => {
            "--- CONTEXT BUNDLE: GAME LAUNCH FAILURE ---\n\
             Check: orphan game processes, race.ini corruption, Content Manager state, \
             disk space, Steam client, serde field mismatch (ai_difficulty vs ai_level), \
             car/track not installed, process guard blocking game exe."
                .to_string()
        }
        DiagnosticTrigger::ProcessCrash { process_name } => {
            format!(
                "--- CONTEXT BUNDLE: PROCESS CRASH ---\n\
                 Crashed process: {}\n\
                 Check: WerFault dumps, parent PID, crash frequency in last hour, \
                 memory pressure, handle count, DLL dependency.",
                process_name
            )
        }
        DiagnosticTrigger::DisplayMismatch { expected_edge_count, actual_edge_count } => {
            format!(
                "--- CONTEXT BUNDLE: DISPLAY/BLANKING ISSUE ---\n\
                 Expected Edge processes: {}, Actual: {}\n\
                 Check: lock screen state, NVIDIA Surround, resolution, \
                 Session 0 vs Session 1 (edge_process_count=0 in Session 0).",
                expected_edge_count, actual_edge_count
            )
        }
        DiagnosticTrigger::WsDisconnect { disconnected_secs } => {
            format!(
                "--- CONTEXT BUNDLE: WEBSOCKET DISCONNECT ---\n\
                 Disconnected for: {}s\n\
                 Check: server reachability (ping + HTTP), network adapter state, \
                 reconnect backoff status, concurrent reconnection storms.",
                disconnected_secs
            )
        }
        DiagnosticTrigger::HealthCheckFail => {
            "--- CONTEXT BUNDLE: HEALTH CHECK FAILURE ---\n\
             Check: port binding (netstat), CPU/memory usage, \
             disk I/O, process running but not responding (hung)."
                .to_string()
        }
        _ => {
            // Generic context for other triggers
            format!("--- CONTEXT BUNDLE ---\nTrigger: {:?}", trigger)
        }
    };

    format!("{}\n\n{}", base_symptoms, context)
}

/// Extract a DiagnosisResult JSON from model response text.
/// Models are prompted to output JSON but may wrap it in markdown or explanation.
/// T6 fix: Uses bracket-counting parser instead of naive first-'{'/last-'}' to handle
/// nested JSON and braces in surrounding text correctly.
fn extract_diagnosis(text: &str) -> Option<DiagnosisResult> {
    // Try direct parse first
    if let Ok(d) = serde_json::from_str::<DiagnosisResult>(text) {
        return Some(d);
    }

    // Try to find the first valid JSON object by bracket counting
    // Handles: preamble text, ```json fences, nested objects, trailing commentary
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'{' {
            let start = i;
            let mut depth: i32 = 0;
            let mut in_string = false;
            let mut escape_next = false;

            for j in start..bytes.len() {
                if escape_next {
                    escape_next = false;
                    continue;
                }
                match bytes[j] {
                    b'\\' if in_string => escape_next = true,
                    b'"' => in_string = !in_string,
                    b'{' if !in_string => depth += 1,
                    b'}' if !in_string => {
                        depth -= 1;
                        if depth == 0 {
                            let candidate = &text[start..=j];
                            if let Ok(d) = serde_json::from_str::<DiagnosisResult>(candidate) {
                                return Some(d);
                            }
                            break; // This { didn't produce valid JSON, try next {
                        }
                    }
                    _ => {}
                }
            }
            i = start + 1; // Move past this { and try next one
        } else {
            i += 1;
        }
    }

    None
}

/// Total cost of all responses
pub fn total_cost(responses: &[ModelResponse]) -> f64 {
    responses.iter().map(|r| r.cost_estimate).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_diagnosis_direct_json() {
        let json = r#"{"root_cause": "MAINTENANCE_MODE blocking", "confidence": 0.9, "fix_action": "delete sentinel", "risk_level": "safe"}"#;
        let d = extract_diagnosis(json);
        assert!(d.is_some());
        let d = d.expect("diagnosis");
        assert_eq!(d.root_cause, "MAINTENANCE_MODE blocking");
        assert!((d.confidence - 0.9).abs() < f64::EPSILON);
    }

    #[test]
    fn test_extract_diagnosis_wrapped_in_markdown() {
        let text = "The issue is likely...\n```json\n{\"root_cause\": \"stale sentinel\", \"confidence\": 0.8, \"fix_action\": \"remove file\", \"risk_level\": \"safe\"}\n```\nThis should fix it.";
        let d = extract_diagnosis(text);
        assert!(d.is_some());
        assert_eq!(d.expect("diagnosis").root_cause, "stale sentinel");
    }

    #[test]
    fn test_extract_diagnosis_no_json() {
        let text = "I'm not sure what's wrong. Try restarting the service.";
        let d = extract_diagnosis(text);
        assert!(d.is_none());
    }

    #[test]
    fn test_find_consensus_single_high_confidence() {
        let responses = vec![ModelResponse {
            model_id: "test".to_string(),
            role: "Scanner".to_string(),
            diagnosis: Some(DiagnosisResult {
                root_cause: "sentinel block".to_string(),
                confidence: 0.85,
                fix_action: "delete file".to_string(),
                risk_level: "safe".to_string(),
                permanent_fix: None,
                verification: None,
                fix_type_class: None,
            }),
            raw_text: String::new(),
            cost_estimate: 0.05,
            error: None,
        }];
        let c = find_consensus(&responses);
        assert!(c.is_some());
    }

    #[test]
    fn test_find_consensus_single_low_confidence() {
        let responses = vec![ModelResponse {
            model_id: "test".to_string(),
            role: "Scanner".to_string(),
            diagnosis: Some(DiagnosisResult {
                root_cause: "maybe something".to_string(),
                confidence: 0.3,
                fix_action: "unknown".to_string(),
                risk_level: "caution".to_string(),
                permanent_fix: None,
                verification: None,
                fix_type_class: None,
            }),
            raw_text: String::new(),
            cost_estimate: 0.05,
            error: None,
        }];
        let c = find_consensus(&responses);
        assert!(c.is_none(), "Low confidence single response should not produce consensus");
    }

    #[test]
    fn test_format_symptoms() {
        let s = format_symptoms("WsDisconnect", "ws_disconnect", "{build: abc}", "ws_connected: false");
        assert!(s.contains("WsDisconnect"));
        assert!(s.contains("ws_disconnect"));
        assert!(s.contains("sim racing pod"));
    }

    #[test]
    fn test_get_api_key_missing() {
        // In test env, OPENROUTER_KEY is likely not set
        // This just verifies the function doesn't panic
        let _ = get_api_key();
    }

    #[test]
    fn test_model_registry_has_5_models() {
        assert_eq!(MODELS.len(), 5);
        assert_eq!(MODELS[0].role, "Scanner");
        assert_eq!(MODELS[1].role, "Reasoner");
        assert_eq!(MODELS[2].role, "Code Expert");
        assert_eq!(MODELS[3].role, "SRE");
        assert_eq!(MODELS[4].role, "Security");
    }

    #[test]
    fn test_find_consensus_keyword_matching() {
        // Two models agree on "MAINTENANCE_MODE sentinel stuck" — should find consensus
        let responses = vec![
            ModelResponse {
                model_id: "qwen".to_string(),
                role: "Scanner".to_string(),
                diagnosis: Some(DiagnosisResult {
                    root_cause: "MAINTENANCE_MODE sentinel file stuck with no TTL".to_string(),
                    confidence: 0.8,
                    fix_action: "delete sentinel".to_string(),
                    risk_level: "safe".to_string(),
                    permanent_fix: None,
                    verification: None,
                    fix_type_class: None,
                }),
                raw_text: String::new(),
                cost_estimate: 0.05,
                error: None,
            },
            ModelResponse {
                model_id: "r1".to_string(),
                role: "Reasoner".to_string(),
                diagnosis: Some(DiagnosisResult {
                    root_cause: "MAINTENANCE_MODE sentinel blocking restarts, no TTL auto-clear".to_string(),
                    confidence: 0.9,
                    fix_action: "clear MAINTENANCE_MODE".to_string(),
                    risk_level: "safe".to_string(),
                    permanent_fix: None,
                    verification: None,
                    fix_type_class: None,
                }),
                raw_text: String::new(),
                cost_estimate: 0.43,
                error: None,
            },
            ModelResponse {
                model_id: "v3".to_string(),
                role: "Code Expert".to_string(),
                diagnosis: Some(DiagnosisResult {
                    root_cause: "TCP port exhaustion on 8090 from CLOSE_WAIT sockets".to_string(),
                    confidence: 0.6,
                    fix_action: "restart network stack".to_string(),
                    risk_level: "caution".to_string(),
                    permanent_fix: None,
                    verification: None,
                    fix_type_class: None,
                }),
                raw_text: String::new(),
                cost_estimate: 0.16,
                error: None,
            },
        ];
        let c = find_consensus(&responses);
        assert!(c.is_some(), "Should find consensus on MAINTENANCE_MODE");
        let c = c.expect("consensus");
        assert!(c.root_cause.contains("MAINTENANCE_MODE"), "Consensus should be MAINTENANCE_MODE, got: {}", c.root_cause);
        assert!((c.confidence - 0.9).abs() < f64::EPSILON, "Should pick highest confidence in consensus group");
    }

    #[test]
    fn test_find_consensus_no_agreement() {
        // Three models all disagree — no consensus, should pick highest confidence >= 0.7
        let responses = vec![
            ModelResponse {
                model_id: "a".to_string(),
                role: "Scanner".to_string(),
                diagnosis: Some(DiagnosisResult {
                    root_cause: "disk space full from log rotation failure".to_string(),
                    confidence: 0.75,
                    fix_action: "clean logs".to_string(),
                    risk_level: "safe".to_string(),
                    permanent_fix: None,
                    verification: None,
                    fix_type_class: None,
                }),
                raw_text: String::new(),
                cost_estimate: 0.05,
                error: None,
            },
            ModelResponse {
                model_id: "b".to_string(),
                role: "Reasoner".to_string(),
                diagnosis: Some(DiagnosisResult {
                    root_cause: "WebSocket reconnect backoff ceiling reached".to_string(),
                    confidence: 0.65,
                    fix_action: "reset backoff".to_string(),
                    risk_level: "safe".to_string(),
                    permanent_fix: None,
                    verification: None,
                    fix_type_class: None,
                }),
                raw_text: String::new(),
                cost_estimate: 0.43,
                error: None,
            },
            ModelResponse {
                model_id: "c".to_string(),
                role: "Security".to_string(),
                diagnosis: Some(DiagnosisResult {
                    root_cause: "TOML config parse error falling back to empty defaults".to_string(),
                    confidence: 0.55,
                    fix_action: "fix config".to_string(),
                    risk_level: "safe".to_string(),
                    permanent_fix: None,
                    verification: None,
                    fix_type_class: None,
                }),
                raw_text: String::new(),
                cost_estimate: 1.65,
                error: None,
            },
        ];
        let c = find_consensus(&responses);
        assert!(c.is_some(), "Should fall back to highest confidence >= 0.7");
        assert!((c.expect("fallback").confidence - 0.75).abs() < f64::EPSILON);
    }

    #[test]
    fn test_format_symptoms_includes_fleet_context() {
        let s = format_symptoms("WsDisconnect", "ws_disconnect", "{build: abc}", "ws_connected: false");
        assert!(s.contains("FLEET CONTEXT"), "Symptoms should include fleet context");
        assert!(s.contains("MAINTENANCE_MODE"), "Fleet context should mention sentinel files");
        assert!(s.contains("Session 1"), "Fleet context should mention session requirements");
    }

    #[test]
    fn test_total_cost() {
        let responses = vec![
            ModelResponse {
                model_id: "a".to_string(),
                role: "Scanner".to_string(),
                diagnosis: None,
                raw_text: String::new(),
                cost_estimate: 0.05,
                error: None,
            },
            ModelResponse {
                model_id: "b".to_string(),
                role: "Reasoner".to_string(),
                diagnosis: None,
                raw_text: String::new(),
                cost_estimate: 0.43,
                error: None,
            },
        ];
        assert!((total_cost(&responses) - 0.48).abs() < 0.001);
    }
}
