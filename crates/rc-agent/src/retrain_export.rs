//! Retrain Export — TRAIN-01/02/03.
//!
//! Every Sunday at midnight IST, reads the past 7 days of evaluation records and
//! KB solutions, assembles them into Ollama/Unsloth-compatible JSONL training pairs,
//! and writes them to `C:\RacingPoint\training\retrain_YYYY-MM-DD.jsonl`.
//!
//! Phase 293, Plan 01: TRAIN-01 — Weekly cron, TrainEntry struct, JSONL export.
//!
//! Key design choices:
//! - JSONL format: `{"messages": [{role, content}, ...], model_id, correct, cost_usd, fix_outcome, training_signal, created_at}`
//! - Compatible with Ollama fine-tune workflow AND Unsloth `apply_chat_template`
//! - KB solutions filtered to confidence >= 0.6 (remove low-quality entries)
//! - Never holds Mutex across `.await` — guard acquired and dropped in tight `{ }` block
//! - All rusqlite operations in `spawn_blocking` (Connection creation + queries are sync)
//! - Zero `.unwrap()` in production code paths
//! - Lifecycle logging: started, sleep, export-written

use std::fs;
use std::io::{BufWriter, Write};
use std::sync::{Arc, Mutex};

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::knowledge_base::{KnowledgeBase, KB_PATH};
use crate::model_eval_store::{EvalRecord, ModelEvalStore};

const LOG_TARGET: &str = "retrain-export";

/// Output directory for JSONL training files on the venue Windows machine.
pub const TRAINING_OUTPUT_DIR: &str = r"C:\RacingPoint\training";

/// Minimum KB solution confidence required to include as a training entry.
/// Filters low-quality or unvalidated solutions from the training set.
const MIN_KB_CONFIDENCE: f64 = 0.6;

/// System prompt injected into every training entry.
///
/// Defines the model's role and task for fine-tuning.
const SYSTEM_PROMPT: &str = "You are an expert racing simulator diagnostician. \
Given a symptom report from a sim racing pod, identify the root cause \
and provide the correct fix action.";

// ---- TrainEntry — one row in the JSONL file ---------------------------------

/// One entry in the JSONL training file.
///
/// Ollama fine-tune format: `{"messages": [{role, content}, ...]}`
/// Unsloth format: same `"messages"` array — compatible with `apply_chat_template`.
///
/// Additional fields (model_id, correct, cost_usd, fix_outcome, training_signal,
/// created_at) are preserved in the JSONL for future analytics and filtering.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainEntry {
    /// Ollama/Unsloth conversation format — always system -> user -> assistant.
    pub messages: Vec<ChatMessage>,
    /// Source model that produced this diagnosis (e.g. "deepseek/deepseek-r1-0528").
    /// "kb" for entries sourced from KnowledgeBase solutions.
    pub model_id: String,
    /// Whether the diagnosis was correct (outcome == "fixed" or kb solution).
    pub correct: bool,
    /// API cost in USD for this entry (0.0 for KB entries).
    pub cost_usd: f64,
    /// "fixed" | "failed_to_fix" | "kb_solution"
    pub fix_outcome: String,
    /// "positive" | "negative" (derived from `correct` flag).
    /// Positive = use as training signal; Negative = counter-example.
    pub training_signal: String,
    /// RFC 3339 UTC timestamp from the source record.
    pub created_at: String,
}

/// A single message in the conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    /// "system" | "user" | "assistant"
    pub role: String,
    pub content: String,
}

// ---- Entry builders — pure, no IO -------------------------------------------

/// Convert a slice of `EvalRecord`s into `TrainEntry`s.
///
/// Each entry maps:
/// - system: the shared diagnostic system prompt
/// - user: pod + trigger context
/// - assistant: the model's prediction (root cause text)
///
/// `fix_outcome` is derived from `actual_outcome`:
/// - "fixed" -> "fixed"
/// - anything else -> "failed_to_fix"
pub fn build_entries_from_evals(records: &[EvalRecord]) -> Vec<TrainEntry> {
    records
        .iter()
        .map(|record| {
            let fix_outcome = if record.actual_outcome == "fixed" {
                "fixed".to_string()
            } else {
                "failed_to_fix".to_string()
            };
            let training_signal = if record.correct {
                "positive".to_string()
            } else {
                "negative".to_string()
            };

            TrainEntry {
                messages: vec![
                    ChatMessage {
                        role: "system".to_string(),
                        content: SYSTEM_PROMPT.to_string(),
                    },
                    ChatMessage {
                        role: "user".to_string(),
                        content: format!(
                            "Pod: {}. Event type: {}. Symptoms observed on pod during session.",
                            record.pod_id, record.trigger_type
                        ),
                    },
                    ChatMessage {
                        role: "assistant".to_string(),
                        content: record.prediction.clone(),
                    },
                ],
                model_id: record.model_id.clone(),
                correct: record.correct,
                cost_usd: record.cost_usd,
                fix_outcome,
                training_signal,
                created_at: record.created_at.clone(),
            }
        })
        .collect()
}

/// Convert a slice of KB `Solution`s into `TrainEntry`s.
///
/// Only includes solutions with `confidence >= MIN_KB_CONFIDENCE` to filter
/// low-quality or unvalidated solutions. KB entries are always `correct=true`
/// (they represent validated fixes applied successfully to the fleet).
///
/// Each entry maps:
/// - system: the shared diagnostic system prompt
/// - user: problem_key + root_cause context
/// - assistant: the validated fix_action
pub fn build_entries_from_solutions(solutions: &[crate::knowledge_base::Solution]) -> Vec<TrainEntry> {
    solutions
        .iter()
        .filter(|s| s.confidence >= MIN_KB_CONFIDENCE)
        .map(|solution| TrainEntry {
            messages: vec![
                ChatMessage {
                    role: "system".to_string(),
                    content: SYSTEM_PROMPT.to_string(),
                },
                ChatMessage {
                    role: "user".to_string(),
                    content: format!(
                        "Pod: unknown. Event type: {}. Root cause: {}",
                        solution.problem_key, solution.root_cause
                    ),
                },
                ChatMessage {
                    role: "assistant".to_string(),
                    content: solution.fix_action.clone(),
                },
            ],
            model_id: "kb".to_string(),
            correct: true,
            cost_usd: 0.0,
            fix_outcome: "kb_solution".to_string(),
            training_signal: "positive".to_string(),
            created_at: solution.updated_at.clone(),
        })
        .collect()
}

// ---- JSONL writer — pure, no async ------------------------------------------

/// Write training entries to a JSONL file in `output_dir`.
///
/// File name: `retrain_YYYY-MM-DD.jsonl` (UTC date of export).
///
/// Returns:
/// - `Ok(0)` if `entries` is empty (file NOT created — skips empty exports)
/// - `Ok(n)` where `n` is the number of lines written on success
/// - `Err(_)` if directory creation or file I/O fails
///
/// This function is synchronous and safe to call from `spawn_blocking`.
pub fn write_jsonl(entries: &[TrainEntry], output_dir: &str) -> anyhow::Result<usize> {
    if entries.is_empty() {
        tracing::info!(
            target: LOG_TARGET,
            "TRAIN-01: no entries to export — skipping file creation"
        );
        return Ok(0);
    }

    fs::create_dir_all(output_dir)?;

    let date_str = Utc::now().format("%Y-%m-%d").to_string();
    // Use std::path::Path::join for cross-platform path construction in tests,
    // but keep the Windows-style separator for the production path constant.
    let file_path = std::path::Path::new(output_dir)
        .join(format!("retrain_{}.jsonl", date_str))
        .to_string_lossy()
        .to_string();

    let file = fs::File::create(&file_path)?;
    let mut writer = BufWriter::new(file);
    let mut count = 0usize;

    for entry in entries {
        let line = serde_json::to_string(entry)?;
        writeln!(writer, "{}", line)?;
        count += 1;
    }

    writer.flush()?;

    tracing::info!(
        target: LOG_TARGET,
        path = %file_path,
        entry_count = count,
        "TRAIN-01: retrain JSONL export written"
    );

    Ok(count)
}

// ---- Weekly cron ------------------------------------------------------------

/// Spawn the weekly retrain export cron.
///
/// Sleeps until next Sunday midnight IST (reusing `weekly_report::seconds_until_next_sunday_midnight_ist`),
/// adds a 0-10 minute jitter (offset from eval_rollup's 0-5 min jitter so files don't race),
/// then reads the last 7 days of eval records + KB solutions and writes a JSONL file.
///
/// The `eval_store` Arc<Mutex> is never held across `.await` — the guard is acquired and
/// dropped in a tight `{ }` block before any async work begins.
pub fn spawn(eval_store: Arc<Mutex<ModelEvalStore>>) {
    tokio::spawn(async move {
        tracing::info!(
            target: "state",
            task = "retrain_export",
            event = "lifecycle",
            "lifecycle: started"
        );
        loop {
            let secs = crate::weekly_report::seconds_until_next_sunday_midnight_ist();
            let jitter_secs: u64 = rand::random::<u64>() % 600; // 0-10 min jitter (after rollup)
            tracing::info!(
                target: LOG_TARGET,
                secs_until_export = secs,
                "Sleeping until next Sunday midnight IST for retrain export"
            );
            tokio::time::sleep(std::time::Duration::from_secs(secs + jitter_secs)).await;
            run_weekly_export(&eval_store).await;
        }
    });
}

/// Execute one weekly retrain export cycle.
///
/// 1. Compute 7-day window (from/to as RFC 3339 strings).
/// 2. Lock eval_store, call query_all(), drop lock immediately (no .await while locked).
/// 3. Build eval entries via build_entries_from_evals().
/// 4. In spawn_blocking: open KnowledgeBase, query solutions from past 7 days, build KB entries.
/// 5. Combine eval + KB entries; call write_jsonl().
/// 6. Log total count with training_signal breakdown.
async fn run_weekly_export(eval_store: &Arc<Mutex<ModelEvalStore>>) {
    let to = Utc::now().to_rfc3339();
    let from = (Utc::now() - chrono::Duration::days(7)).to_rfc3339();
    let from_clone = from.clone();

    // Acquire lock, query records, drop lock before any async work.
    let eval_records = {
        match eval_store.lock() {
            Ok(guard) => match guard.query_all(Some(&from), Some(&to)) {
                Ok(recs) => recs,
                Err(e) => {
                    tracing::warn!(
                        target: LOG_TARGET,
                        error = %e,
                        "TRAIN-01: failed to query eval records — skipping cycle"
                    );
                    return;
                }
            },
            Err(e) => {
                tracing::warn!(
                    target: LOG_TARGET,
                    error = %e,
                    "TRAIN-01: eval store Mutex poisoned — skipping cycle"
                );
                return;
            }
        }
    }; // guard dropped here

    let eval_entries = build_entries_from_evals(&eval_records);
    let eval_count = eval_entries.len();

    // Open KnowledgeBase + query solutions in spawn_blocking (rusqlite is sync).
    let kb_entries_result = tokio::task::spawn_blocking(move || -> anyhow::Result<Vec<TrainEntry>> {
        let kb = KnowledgeBase::open(KB_PATH)?;

        // Raw SQL to query solutions updated in the past 7 days.
        // KnowledgeBase::lookup_all() requires a problem_key; use conn() for a time-range query.
        let mut stmt = kb.conn().prepare(
            "SELECT id, problem_key, root_cause, fix_action, confidence, \
             success_count, fail_count, cost_to_diagnose, created_at, updated_at \
             FROM solutions WHERE updated_at >= ?1 ORDER BY updated_at DESC LIMIT 5000",
        )?;

        let solutions: Vec<crate::knowledge_base::Solution> = stmt
            .query_map(rusqlite::params![from_clone], |row| {
                Ok(crate::knowledge_base::Solution {
                    id: row.get(0)?,
                    problem_key: row.get(1)?,
                    problem_hash: String::new(),
                    symptoms: String::new(),
                    environment: String::new(),
                    root_cause: row.get(2)?,
                    fix_action: row.get(3)?,
                    fix_type: String::new(),
                    success_count: row.get(5)?,
                    fail_count: row.get(6)?,
                    confidence: row.get(4)?,
                    cost_to_diagnose: row.get(7)?,
                    models_used: None,
                    source_node: String::new(),
                    created_at: row.get(8)?,
                    updated_at: row.get(9)?,
                    version: 1,
                    ttl_days: 90,
                    tags: None,
                    diagnosis_method: None,
                    fix_permanence: "workaround".to_string(),
                    recurrence_count: 0,
                    permanent_fix_id: None,
                    last_recurrence: None,
                    permanent_attempt_at: None,
                })
            })?
            .filter_map(|r| match r {
                Ok(s) => Some(s),
                Err(e) => {
                    tracing::warn!(
                        target: LOG_TARGET,
                        error = %e,
                        "TRAIN-01: failed to deserialize KB solution row — skipping"
                    );
                    None
                }
            })
            .collect();

        Ok(build_entries_from_solutions(&solutions))
    })
    .await;

    let kb_entries = match kb_entries_result {
        Ok(Ok(entries)) => entries,
        Ok(Err(e)) => {
            tracing::warn!(
                target: LOG_TARGET,
                error = %e,
                "TRAIN-01: failed to query KB solutions — proceeding with eval entries only"
            );
            Vec::new()
        }
        Err(e) => {
            tracing::warn!(
                target: LOG_TARGET,
                error = %e,
                "TRAIN-01: spawn_blocking for KB query panicked — proceeding with eval entries only"
            );
            Vec::new()
        }
    };

    let kb_count = kb_entries.len();

    // Combine eval entries and KB entries.
    let mut all_entries = eval_entries;
    all_entries.extend(kb_entries);

    let positive_count = all_entries.iter().filter(|e| e.training_signal == "positive").count();
    let negative_count = all_entries.iter().filter(|e| e.training_signal == "negative").count();

    tracing::info!(
        target: LOG_TARGET,
        eval_entries = eval_count,
        kb_entries = kb_count,
        total_entries = all_entries.len(),
        positive = positive_count,
        negative = negative_count,
        "TRAIN-01: assembled training entries"
    );

    // write_jsonl is synchronous — safe to call directly (no DB, no async needed).
    match write_jsonl(&all_entries, TRAINING_OUTPUT_DIR) {
        Ok(0) => {
            tracing::info!(
                target: LOG_TARGET,
                "TRAIN-01: no entries — export skipped for this week"
            );
        }
        Ok(n) => {
            tracing::info!(
                target: LOG_TARGET,
                entry_count = n,
                positive = positive_count,
                negative = negative_count,
                "TRAIN-01: retrain export complete"
            );
        }
        Err(e) => {
            tracing::warn!(
                target: LOG_TARGET,
                error = %e,
                "TRAIN-01: failed to write JSONL export"
            );
        }
    }
}

// ---- Tests ------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_eval_record(pod_id: &str, model_id: &str, correct: bool, outcome: &str) -> EvalRecord {
        EvalRecord {
            id: uuid::Uuid::new_v4().to_string(),
            model_id: model_id.to_string(),
            pod_id: pod_id.to_string(),
            trigger_type: "ProcessCrash".to_string(),
            prediction: format!("orphan werfault process on {}", pod_id),
            actual_outcome: outcome.to_string(),
            correct,
            cost_usd: 0.10,
            created_at: chrono::Utc::now().to_rfc3339(),
        }
    }

    fn make_solution(problem_key: &str, confidence: f64) -> crate::knowledge_base::Solution {
        crate::knowledge_base::Solution {
            id: uuid::Uuid::new_v4().to_string(),
            problem_key: problem_key.to_string(),
            problem_hash: "abc123".to_string(),
            symptoms: "game crash".to_string(),
            environment: "{}".to_string(),
            root_cause: format!("root cause of {}", problem_key),
            fix_action: format!("apply fix for {}", problem_key),
            fix_type: "command".to_string(),
            success_count: 5,
            fail_count: 1,
            confidence,
            cost_to_diagnose: 0.25,
            models_used: None,
            source_node: "pod_1".to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
            updated_at: chrono::Utc::now().to_rfc3339(),
            version: 1,
            ttl_days: 90,
            tags: None,
            diagnosis_method: None,
            fix_permanence: "workaround".to_string(),
            recurrence_count: 0,
            permanent_fix_id: None,
            last_recurrence: None,
            permanent_attempt_at: None,
        }
    }

    // Test 1: build_entries_from_evals() with 3 EvalRecords returns 3 TrainEntry structs.
    // Each must have non-empty system/user/assistant, model_id populated,
    // correct matches input, fix_outcome is "fixed" or "failed_to_fix".
    #[test]
    fn test_build_entries_from_evals_basic() {
        let records = vec![
            make_eval_record("pod_1", "model_a", true, "fixed"),
            make_eval_record("pod_2", "model_b", false, "failed_to_fix"),
            make_eval_record("pod_3", "model_c", true, "fixed"),
        ];

        let entries = build_entries_from_evals(&records);
        assert_eq!(entries.len(), 3, "must produce 3 entries from 3 records");

        for (i, entry) in entries.iter().enumerate() {
            assert_eq!(entry.messages.len(), 3, "entry {} must have 3 messages", i);
            assert_eq!(entry.messages[0].role, "system");
            assert_eq!(entry.messages[1].role, "user");
            assert_eq!(entry.messages[2].role, "assistant");
            assert!(!entry.messages[0].content.is_empty(), "system content must be non-empty");
            assert!(!entry.messages[1].content.is_empty(), "user content must be non-empty");
            assert!(!entry.messages[2].content.is_empty(), "assistant content must be non-empty");
            assert!(!entry.model_id.is_empty(), "model_id must be non-empty");
            assert!(
                entry.fix_outcome == "fixed" || entry.fix_outcome == "failed_to_fix",
                "fix_outcome must be 'fixed' or 'failed_to_fix', got '{}'", entry.fix_outcome
            );
        }

        assert_eq!(entries[0].model_id, "model_a");
        assert_eq!(entries[1].model_id, "model_b");
        assert_eq!(entries[2].model_id, "model_c");
    }

    // Test 2: correct=true eval -> assistant contains prediction text,
    // fix_outcome="fixed", training_signal="positive".
    #[test]
    fn test_build_entries_correct_eval_maps_correctly() {
        let record = make_eval_record("pod_5", "deepseek/r1", true, "fixed");
        let prediction = record.prediction.clone();

        let entries = build_entries_from_evals(std::slice::from_ref(&record));
        assert_eq!(entries.len(), 1);

        let entry = &entries[0];
        assert_eq!(
            entry.messages[2].content, prediction,
            "assistant content must equal prediction text"
        );
        assert_eq!(entry.fix_outcome, "fixed");
        assert_eq!(entry.training_signal, "positive");
        assert!(entry.correct);
    }

    // Test 3: correct=false eval -> fix_outcome="failed_to_fix", training_signal="negative".
    #[test]
    fn test_build_entries_incorrect_eval_maps_correctly() {
        let record = make_eval_record("pod_2", "kimi-k2", false, "failed_to_fix");

        let entries = build_entries_from_evals(std::slice::from_ref(&record));
        assert_eq!(entries.len(), 1);

        let entry = &entries[0];
        assert_eq!(entry.fix_outcome, "failed_to_fix");
        assert_eq!(entry.training_signal, "negative");
        assert!(!entry.correct);
    }

    // Test 4: build_entries_from_solutions() with 2 Solution rows ->
    // each TrainEntry has fix_outcome="kb_solution", model_id="kb", correct=true.
    #[test]
    fn test_build_entries_from_solutions_basic() {
        let solutions = vec![
            make_solution("werfault_crash", 0.9),
            make_solution("conspit_disconnect", 0.75),
        ];

        let entries = build_entries_from_solutions(&solutions);
        assert_eq!(entries.len(), 2, "must produce 2 entries from 2 solutions");

        for entry in &entries {
            assert_eq!(entry.fix_outcome, "kb_solution");
            assert_eq!(entry.model_id, "kb");
            assert!(entry.correct, "KB entries must always be correct=true");
            assert_eq!(entry.cost_usd, 0.0);
            assert_eq!(entry.training_signal, "positive");
            assert_eq!(entry.messages.len(), 3);
            assert_eq!(entry.messages[0].role, "system");
        }
    }

    // Test 5: write_jsonl() given 5 TrainEntry rows writes a file where every line
    // parses as valid JSON with "messages" key containing system/user/assistant objects
    // (Ollama format), and the entry_count matches.
    #[test]
    fn test_write_jsonl_produces_valid_lines() {
        let records = vec![
            make_eval_record("pod_1", "model_a", true, "fixed"),
            make_eval_record("pod_2", "model_b", false, "failed_to_fix"),
            make_eval_record("pod_3", "model_c", true, "fixed"),
            make_eval_record("pod_4", "model_d", true, "fixed"),
            make_eval_record("pod_5", "model_e", false, "escalated"),
        ];
        let entries = build_entries_from_evals(&records);
        assert_eq!(entries.len(), 5);

        let output_dir = std::env::temp_dir()
            .join("retrain_export_test")
            .to_string_lossy()
            .to_string();
        let count = write_jsonl(&entries, &output_dir).expect("write_jsonl must succeed");
        assert_eq!(count, 5, "must write 5 lines");

        // Verify each line parses as valid JSON with messages array.
        let date_str = Utc::now().format("%Y-%m-%d").to_string();
        let file_path = std::path::Path::new(&output_dir)
            .join(format!("retrain_{}.jsonl", date_str))
            .to_string_lossy()
            .to_string();
        let content = std::fs::read_to_string(&file_path).expect("file must exist after write");

        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 5, "file must contain exactly 5 lines");

        for (i, line) in lines.iter().enumerate() {
            let parsed: serde_json::Value = serde_json::from_str(line)
                .unwrap_or_else(|e| panic!("line {} must be valid JSON: {}", i, e));

            let messages = parsed["messages"]
                .as_array()
                .unwrap_or_else(|| panic!("line {} must have 'messages' array", i));
            assert_eq!(messages.len(), 3, "line {} must have 3 messages", i);
            assert_eq!(messages[0]["role"], "system", "line {} first role must be 'system'", i);
            assert_eq!(messages[1]["role"], "user", "line {} second role must be 'user'", i);
            assert_eq!(messages[2]["role"], "assistant", "line {} third role must be 'assistant'", i);
        }

        // Cleanup
        let _ = std::fs::remove_dir_all(&output_dir);
    }

    // Test 6: write_jsonl() with 0 entries returns Ok(0) but does NOT create the file.
    #[test]
    fn test_write_jsonl_empty_entries_no_file_created() {
        let output_dir = std::env::temp_dir()
            .join("retrain_export_empty_test")
            .to_string_lossy()
            .to_string();

        // Ensure the directory does not exist from a previous test run.
        let _ = std::fs::remove_dir_all(&output_dir);

        let count = write_jsonl(&[], &output_dir).expect("write_jsonl must return Ok for empty input");
        assert_eq!(count, 0, "must return 0 for empty entries");

        // Directory should NOT have been created (no file, no dir).
        let date_str = Utc::now().format("%Y-%m-%d").to_string();
        let file_path = std::path::Path::new(&output_dir)
            .join(format!("retrain_{}.jsonl", date_str))
            .to_string_lossy()
            .to_string();
        assert!(
            !std::path::Path::new(&file_path).exists(),
            "file must NOT be created when entries is empty"
        );
    }

    // Test 7: TrainEntry serializes to JSON with all required fields present.
    #[test]
    fn test_train_entry_serializes_all_required_fields() {
        let record = make_eval_record("pod_7", "gemini/flash", true, "fixed");
        let entries = build_entries_from_evals(std::slice::from_ref(&record));
        let entry = &entries[0];

        let json_str = serde_json::to_string(entry).expect("serialization must succeed");
        let parsed: serde_json::Value = serde_json::from_str(&json_str).expect("must parse back");

        assert!(parsed["messages"].is_array(), "must have 'messages' field");
        assert!(parsed["model_id"].is_string(), "must have 'model_id' field");
        assert!(parsed["correct"].is_boolean(), "must have 'correct' field");
        assert!(parsed["cost_usd"].is_number(), "must have 'cost_usd' field");
        assert!(parsed["fix_outcome"].is_string(), "must have 'fix_outcome' field");
        assert!(parsed["training_signal"].is_string(), "must have 'training_signal' field");
        assert!(parsed["created_at"].is_string(), "must have 'created_at' field");
    }
}
