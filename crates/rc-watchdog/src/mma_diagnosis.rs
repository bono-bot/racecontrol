//! SW-08 / SW-09 / SW-10 / SW-12 / SW-14: MMA diagnosis on restart loop.
//!
//! When rc-agent enters a restart loop (3+ restarts in 10 minutes), this module:
//! 1. Creates MMA_DIAGNOSING sentinel with TTL (SW-14)
//! 2. Spawns a dedicated tokio Runtime thread (SW-09) — the watchdog has NO runtime
//! 3. Queries OpenRouter API for AI diagnosis (SW-08)
//! 4. Falls back to deterministic diagnosis when API unreachable (SW-10)
//! 5. Persists budget state to budget_state.json (SW-12)
//!
//! The MMA diagnosis runs in a background thread and never blocks the main service loop.
//! Results are written to C:\RacingPoint\mma-diagnosis.json for the server to pick up.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// OpenRouter API URL.
const OPENROUTER_API_URL: &str = "https://openrouter.ai/api/v1/chat/completions";

/// Default model for MMA diagnosis.
const DEFAULT_MODEL: &str = "deepseek/deepseek-chat-v3-0324:free";

/// MMA diagnosing sentinel path.
const MMA_SENTINEL_PATH: &str = r"C:\RacingPoint\MMA_DIAGNOSING";

/// MMA diagnosis output path.
const MMA_OUTPUT_PATH: &str = r"C:\RacingPoint\mma-diagnosis.json";

/// Budget state persistence path.
const BUDGET_STATE_PATH: &str = r"C:\RacingPoint\budget_state.json";

/// Max TTL for MMA_DIAGNOSING sentinel (seconds).
const MMA_SENTINEL_TTL_SECS: u64 = 120;

/// Max budget per day (cents) — ~$0.05
const DAILY_BUDGET_CENTS: u32 = 5;

/// Static flag to prevent concurrent MMA runs.
static MMA_RUNNING: AtomicBool = AtomicBool::new(false);

/// Budget state — tracks daily API spend to avoid runaway costs.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BudgetState {
    /// ISO 8601 date string (e.g. "2026-03-31")
    pub current_date: String,
    /// Cumulative cost in cents for the current day
    pub spent_cents: u32,
    /// Total queries made today
    pub query_count: u32,
}

impl Default for BudgetState {
    fn default() -> Self {
        Self {
            current_date: today_date_string(),
            spent_cents: 0,
            query_count: 0,
        }
    }
}

impl BudgetState {
    pub fn load() -> Self {
        Self::load_from(BUDGET_STATE_PATH)
    }

    pub fn load_from(path: &str) -> Self {
        match std::fs::read_to_string(path) {
            Ok(content) => {
                let mut state: Self = serde_json::from_str(&content).unwrap_or_default();
                // Reset if new day
                let today = today_date_string();
                if state.current_date != today {
                    state.current_date = today;
                    state.spent_cents = 0;
                    state.query_count = 0;
                }
                state
            }
            Err(_) => Self::default(),
        }
    }

    pub fn save(&self) {
        self.save_to(BUDGET_STATE_PATH);
    }

    pub fn save_to(&self, path: &str) {
        let tmp = format!("{}.tmp", path);
        if let Ok(json) = serde_json::to_string_pretty(self) {
            if std::fs::write(&tmp, &json).is_ok() {
                if std::fs::rename(&tmp, path).is_err() {
                    tracing::error!("budget_state: rename tmp -> {} failed", path);
                    let _ = std::fs::remove_file(&tmp);
                }
            }
        }
    }

    /// Check if we have budget remaining for today.
    pub fn has_budget(&self) -> bool {
        let today = today_date_string();
        if self.current_date != today {
            return true; // New day = fresh budget
        }
        self.spent_cents < DAILY_BUDGET_CENTS
    }

    /// Record a query and its approximate cost.
    pub fn record_query(&mut self, cost_cents: u32) {
        let today = today_date_string();
        if self.current_date != today {
            self.current_date = today;
            self.spent_cents = 0;
            self.query_count = 0;
        }
        self.spent_cents = self.spent_cents.saturating_add(cost_cents);
        self.query_count = self.query_count.saturating_add(1);
    }
}

/// MMA diagnosis result.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DiagnosisResult {
    pub timestamp: String,
    pub source: String, // "openrouter" | "deterministic_fallback"
    pub model: String,
    pub diagnosis: String,
    pub recommended_action: String,
    pub confidence: f64,
    pub restart_context: RestartContext,
}

/// Context about the restart loop that triggered diagnosis.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RestartContext {
    pub restart_count: u32,
    pub time_window_secs: u64,
    pub pod_id: String,
    pub last_exit_code: Option<i32>,
    pub rollback_depth: u32,
}

/// SW-14: Write MMA_DIAGNOSING sentinel with TTL.
fn write_mma_sentinel(context: &RestartContext) {
    let content = serde_json::json!({
        "started_at": chrono::Utc::now().to_rfc3339(),
        "ttl_secs": MMA_SENTINEL_TTL_SECS,
        "pod_id": context.pod_id,
        "restart_count": context.restart_count,
    });
    if let Ok(json) = serde_json::to_string_pretty(&content) {
        if let Err(e) = std::fs::write(MMA_SENTINEL_PATH, json) {
            tracing::error!("Failed to write MMA_DIAGNOSING sentinel: {}", e);
        }
    }
}

/// SW-14: Clear MMA_DIAGNOSING sentinel.
fn clear_mma_sentinel() {
    if Path::new(MMA_SENTINEL_PATH).is_file() {
        let _ = std::fs::remove_file(MMA_SENTINEL_PATH);
    }
}

/// Check if MMA_DIAGNOSING sentinel is active (exists and within TTL).
pub fn is_mma_diagnosing() -> bool {
    let path = Path::new(MMA_SENTINEL_PATH);
    if !path.is_file() {
        return false;
    }

    // Check if sentinel has expired
    match std::fs::metadata(path) {
        Ok(meta) => match meta.modified() {
            Ok(modified) => match modified.elapsed() {
                Ok(age) => {
                    if age.as_secs() > MMA_SENTINEL_TTL_SECS {
                        // Expired — clean up
                        tracing::info!("MMA_DIAGNOSING sentinel expired (age: {}s), clearing", age.as_secs());
                        let _ = std::fs::remove_file(path);
                        return false;
                    }
                    true
                }
                Err(_) => false,
            },
            Err(_) => false,
        },
        Err(_) => false,
    }
}

/// Launch MMA diagnosis in a background thread.
///
/// This is the main entry point called from the service loop when a restart loop
/// is detected. It spawns a thread with a dedicated tokio Runtime (SW-09).
/// The thread writes its result to mma-diagnosis.json and clears the sentinel.
///
/// Returns immediately — never blocks the caller.
pub fn launch_diagnosis(context: RestartContext) {
    // Prevent concurrent runs
    if MMA_RUNNING.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).is_err() {
        tracing::info!("MMA diagnosis already running — skipping");
        return;
    }

    // Write sentinel before spawning thread
    write_mma_sentinel(&context);

    std::thread::Builder::new()
        .name("mma-diagnosis".to_string())
        .spawn(move || {
            tracing::info!("MMA diagnosis thread started for pod={}", context.pod_id);
            let result = run_diagnosis(&context);

            // Write result to file
            if let Ok(json) = serde_json::to_string_pretty(&result) {
                let tmp = format!("{}.tmp", MMA_OUTPUT_PATH);
                if std::fs::write(&tmp, &json).is_ok() {
                    if let Err(e) = std::fs::rename(&tmp, MMA_OUTPUT_PATH) {
                        tracing::error!("Failed to write MMA diagnosis output: {}", e);
                        let _ = std::fs::remove_file(&tmp);
                    }
                }
            }

            // Clear sentinel and running flag
            clear_mma_sentinel();
            MMA_RUNNING.store(false, Ordering::SeqCst);
            tracing::info!("MMA diagnosis thread complete: source={}", result.source);
        })
        .map(|_handle| {
            // Thread spawned — it will clean up sentinel and MMA_RUNNING on completion.
            // We don't join — fire-and-forget.
        })
        .unwrap_or_else(|e| {
            tracing::error!("Failed to spawn MMA diagnosis thread: {}", e);
            clear_mma_sentinel();
            MMA_RUNNING.store(false, Ordering::SeqCst);
        });
}

/// Run the actual diagnosis — tries OpenRouter, falls back to deterministic.
fn run_diagnosis(context: &RestartContext) -> DiagnosisResult {
    // Check budget
    let mut budget = BudgetState::load();
    if !budget.has_budget() {
        tracing::info!("MMA budget exhausted for today — using deterministic fallback");
        return deterministic_diagnosis(context);
    }

    // Check for OpenRouter API key
    let api_key = match std::env::var("OPENROUTER_API_KEY") {
        Ok(key) if !key.is_empty() => key,
        _ => {
            tracing::info!("OPENROUTER_API_KEY not set — using deterministic fallback");
            return deterministic_diagnosis(context);
        }
    };

    // SW-09: Create dedicated tokio Runtime for async HTTP
    let rt = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
            tracing::error!("Failed to create tokio Runtime for MMA: {}", e);
            return deterministic_diagnosis(context);
        }
    };

    // Run the async OpenRouter query in the dedicated runtime
    let diagnosis = rt.block_on(async {
        query_openrouter(&api_key, context).await
    });

    match diagnosis {
        Ok(result) => {
            // Record budget usage (~1 cent per query for free models)
            budget.record_query(1);
            budget.save();
            result
        }
        Err(e) => {
            tracing::warn!("OpenRouter query failed: {} — using deterministic fallback", e);
            deterministic_diagnosis(context)
        }
    }
}

/// SW-08: Query OpenRouter for AI diagnosis.
async fn query_openrouter(
    api_key: &str,
    context: &RestartContext,
) -> anyhow::Result<DiagnosisResult> {
    let prompt = format!(
        "You are an expert Windows systems administrator. A racing simulator pod agent \
         (rc-agent.exe) is in a restart loop on Pod {}.\n\n\
         Context:\n\
         - Restart count: {} in the last {} seconds\n\
         - Last exit code: {:?}\n\
         - Rollback depth: {}/3\n\n\
         Common causes include: corrupted binary, DLL missing, port 8090 conflict, \
         Session 0 vs Session 1, MAINTENANCE_MODE sentinel, stale config file, \
         USB hardware disconnected, GPU driver crash.\n\n\
         Provide a brief diagnosis (1-2 sentences) and recommended action. \
         Format as JSON: {{\"diagnosis\": \"...\", \"action\": \"...\", \"confidence\": 0.0-1.0}}",
        context.pod_id,
        context.restart_count,
        context.time_window_secs,
        context.last_exit_code,
        context.rollback_depth
    );

    let body = serde_json::json!({
        "model": DEFAULT_MODEL,
        "messages": [
            {"role": "user", "content": prompt}
        ],
        "max_tokens": 200,
        "temperature": 0.3,
    });

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()?;

    let resp = client
        .post(OPENROUTER_API_URL)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body_text = resp.text().await.unwrap_or_default();
        anyhow::bail!("OpenRouter API error: HTTP {} — {}", status, body_text);
    }

    let resp_json: serde_json::Value = resp.json().await?;

    // Extract the assistant's response text
    let content = resp_json
        .get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("message"))
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_str())
        .unwrap_or("");

    // Try to parse the response as JSON for structured data
    let (diagnosis, action, confidence) = parse_ai_response(content);

    Ok(DiagnosisResult {
        timestamp: chrono::Utc::now().to_rfc3339(),
        source: "openrouter".to_string(),
        model: DEFAULT_MODEL.to_string(),
        diagnosis,
        recommended_action: action,
        confidence,
        restart_context: context.clone(),
    })
}

/// Parse AI response — tries JSON first, falls back to plain text.
fn parse_ai_response(content: &str) -> (String, String, f64) {
    // Try to find JSON in the response (may be wrapped in markdown code block)
    let json_str = content
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(json_str) {
        let diagnosis = parsed
            .get("diagnosis")
            .and_then(|v| v.as_str())
            .unwrap_or("No diagnosis provided")
            .to_string();
        let action = parsed
            .get("action")
            .and_then(|v| v.as_str())
            .unwrap_or("Manual investigation required")
            .to_string();
        let confidence = parsed
            .get("confidence")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.5);
        (diagnosis, action, confidence)
    } else {
        // Fallback: treat entire content as diagnosis
        (
            content.to_string(),
            "Manual investigation required".to_string(),
            0.3,
        )
    }
}

/// SW-10: Deterministic fallback when OpenRouter is unreachable.
///
/// Applies rule-based diagnosis based on restart context.
fn deterministic_diagnosis(context: &RestartContext) -> DiagnosisResult {
    let (diagnosis, action, confidence) = match (context.restart_count, context.last_exit_code) {
        // High restart count with no exit code — likely process killed externally
        (count, None) if count >= 5 => (
            "Frequent restarts with no exit code — rc-agent being killed externally (taskkill, another watchdog, or Session 0 conflict)".to_string(),
            "Check for duplicate watchdog processes (tasklist /V /FI \"IMAGENAME eq rc-watchdog*\"). Verify rc-agent session with tasklist /V /FO CSV | findstr rc-agent".to_string(),
            0.7,
        ),
        // Exit code 1 — generic crash
        (_, Some(1)) => (
            "rc-agent exited with code 1 — likely config error or missing dependency".to_string(),
            "Check rc-agent.toml exists at C:\\RacingPoint\\ and is valid TOML. Verify rc-agent.exe has all DLLs (ldd equivalent: dumpbin /dependents)".to_string(),
            0.6,
        ),
        // Exit code -1073741819 (0xC0000005) — access violation
        (_, Some(code)) if code == -1073741819_i32 => (
            "Access violation (0xC0000005) — corrupted binary, bad memory, or driver crash".to_string(),
            "Reboot pod first. If persists: run sfc /scannow, test RAM with mdsched, check Windows Event Viewer for faulting module".to_string(),
            0.8,
        ),
        // Exit code -1073741515 (0xC0000135) — DLL not found
        (_, Some(code)) if code == -1073741515_i32 => (
            "DLL not found (0xC0000135) — missing runtime dependency".to_string(),
            "Verify static CRT build. Run: dumpbin /dependents rc-agent.exe and check all DLLs exist".to_string(),
            0.9,
        ),
        // Rollback depth > 0 — both current and previous binary failing
        (_, _) if context.rollback_depth >= 2 => (
            "Multiple rollbacks failed — both current and previous binaries are crashing. OS-level issue likely".to_string(),
            "Reboot pod via shutdown /r /t 5 /f. Check Event Viewer after reboot. Consider fresh binary deploy via pendrive".to_string(),
            0.7,
        ),
        // Default case
        (count, exit_code) => (
            format!("rc-agent restart loop: {} restarts, exit_code={:?}", count, exit_code),
            "Check MAINTENANCE_MODE sentinel (del C:\\RacingPoint\\MAINTENANCE_MODE). Verify port 8090 is free (netstat -an | findstr 8090). Reboot if persists.".to_string(),
            0.4,
        ),
    };

    DiagnosisResult {
        timestamp: chrono::Utc::now().to_rfc3339(),
        source: "deterministic_fallback".to_string(),
        model: "rule_engine_v1".to_string(),
        diagnosis,
        recommended_action: action,
        confidence,
        restart_context: context.clone(),
    }
}

/// Get today's date as ISO 8601 string (e.g. "2026-03-31").
fn today_date_string() -> String {
    chrono::Utc::now().format("%Y-%m-%d").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_context() -> RestartContext {
        RestartContext {
            restart_count: 5,
            time_window_secs: 300,
            pod_id: "pod_1".to_string(),
            last_exit_code: None,
            rollback_depth: 0,
        }
    }

    #[test]
    fn test_deterministic_high_restart_no_exit() {
        let ctx = test_context();
        let result = deterministic_diagnosis(&ctx);
        assert_eq!(result.source, "deterministic_fallback");
        assert!(result.diagnosis.contains("killed externally"));
        assert!(result.confidence > 0.0);
    }

    #[test]
    fn test_deterministic_access_violation() {
        let ctx = RestartContext {
            last_exit_code: Some(-1073741819),
            ..test_context()
        };
        let result = deterministic_diagnosis(&ctx);
        assert!(result.diagnosis.contains("Access violation"));
        assert_eq!(result.confidence, 0.8);
    }

    #[test]
    fn test_deterministic_dll_not_found() {
        let ctx = RestartContext {
            last_exit_code: Some(-1073741515),
            ..test_context()
        };
        let result = deterministic_diagnosis(&ctx);
        assert!(result.diagnosis.contains("DLL not found"));
        assert_eq!(result.confidence, 0.9);
    }

    #[test]
    fn test_deterministic_deep_rollback() {
        let ctx = RestartContext {
            rollback_depth: 2,
            last_exit_code: Some(1),
            ..test_context()
        };
        let result = deterministic_diagnosis(&ctx);
        assert!(result.diagnosis.contains("Multiple rollbacks"));
    }

    #[test]
    fn test_deterministic_default_case() {
        let ctx = RestartContext {
            restart_count: 2,
            last_exit_code: Some(42),
            ..test_context()
        };
        let result = deterministic_diagnosis(&ctx);
        assert!(result.diagnosis.contains("restart loop"));
    }

    #[test]
    fn test_budget_state_default() {
        let state = BudgetState::default();
        assert!(state.has_budget());
        assert_eq!(state.spent_cents, 0);
        assert_eq!(state.query_count, 0);
    }

    #[test]
    fn test_budget_state_exhausted() {
        let state = BudgetState {
            current_date: today_date_string(),
            spent_cents: DAILY_BUDGET_CENTS,
            query_count: 5,
        };
        assert!(!state.has_budget());
    }

    #[test]
    fn test_budget_state_new_day_resets() {
        let state = BudgetState {
            current_date: "2020-01-01".to_string(),
            spent_cents: 100,
            query_count: 50,
        };
        // has_budget should return true for an old date (new day)
        assert!(state.has_budget());
    }

    #[test]
    fn test_budget_state_record_query() {
        let mut state = BudgetState::default();
        state.record_query(2);
        assert_eq!(state.spent_cents, 2);
        assert_eq!(state.query_count, 1);
        state.record_query(1);
        assert_eq!(state.spent_cents, 3);
        assert_eq!(state.query_count, 2);
    }

    #[test]
    fn test_budget_state_roundtrip() {
        let dir = std::env::temp_dir().join("rc_watchdog_test_budget");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("budget_state.json");

        let mut state = BudgetState::default();
        state.record_query(3);
        state.save_to(path.to_str().expect("valid path"));

        let loaded = BudgetState::load_from(path.to_str().expect("valid path"));
        assert_eq!(loaded.spent_cents, 3);
        assert_eq!(loaded.query_count, 1);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_parse_ai_response_valid_json() {
        let input = r#"{"diagnosis": "Port conflict", "action": "Kill stale process", "confidence": 0.85}"#;
        let (diag, action, conf) = parse_ai_response(input);
        assert_eq!(diag, "Port conflict");
        assert_eq!(action, "Kill stale process");
        assert!((conf - 0.85).abs() < 0.001);
    }

    #[test]
    fn test_parse_ai_response_json_in_code_block() {
        let input = "```json\n{\"diagnosis\": \"test\", \"action\": \"fix\", \"confidence\": 0.5}\n```";
        let (diag, action, conf) = parse_ai_response(input);
        assert_eq!(diag, "test");
        assert_eq!(action, "fix");
        assert!((conf - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_parse_ai_response_plain_text() {
        let input = "The binary is corrupted and needs to be replaced.";
        let (diag, action, conf) = parse_ai_response(input);
        assert!(diag.contains("corrupted"));
        assert_eq!(action, "Manual investigation required");
        assert!((conf - 0.3).abs() < 0.001);
    }

    #[test]
    fn test_is_mma_diagnosing_no_sentinel() {
        // Clean up any leftover sentinel from other tests
        let _ = std::fs::remove_file(MMA_SENTINEL_PATH);
        // On non-Windows this path won't exist anyway
        assert!(!is_mma_diagnosing());
    }

    #[test]
    fn test_diagnosis_result_serialization() {
        let result = DiagnosisResult {
            timestamp: "2026-03-31T12:00:00Z".to_string(),
            source: "deterministic_fallback".to_string(),
            model: "rule_engine_v1".to_string(),
            diagnosis: "test diagnosis".to_string(),
            recommended_action: "test action".to_string(),
            confidence: 0.75,
            restart_context: test_context(),
        };
        let json = serde_json::to_string(&result).expect("serialize OK");
        assert!(json.contains("\"source\":\"deterministic_fallback\""));
        assert!(json.contains("\"confidence\":0.75"));
    }
}
