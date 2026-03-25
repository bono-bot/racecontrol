//! Verification chain framework for Racing Point.
//!
//! # Hot-path vs Cold-path chains
//!
//! ## Hot-path (billing/WS chains)
//! Use [`HotVerificationChain`] for async fire-and-forget chains.
//! The caller is never blocked — results go to an in-memory ring buffer for
//! diagnostics. Suitable for billing session validation and WebSocket message
//! chains where blocking would add visible latency.
//!
//! ## Cold-path (config/allowlist/health chains)
//! Use [`ColdVerificationChain`] for synchronous chains where correctness is
//! more important than latency. The chain blocks until all steps complete.
//! Suitable for config validation at startup, allowlist verification, and
//! health check pipelines that must halt on the first failure.

use chrono::{DateTime, Utc};

// ─── Error Types ─────────────────────────────────────────────────────────────

/// Typed errors produced by verification chain steps.
///
/// Each variant carries the `step` name (for trace attribution) and the
/// `raw_value` that caused the failure (for diagnostics without needing to
/// re-fetch the original data).
#[derive(Debug, thiserror::Error)]
pub enum VerificationError {
    #[error("input parse failed at step '{step}': raw value = {raw_value}")]
    InputParseError { step: String, raw_value: String },

    #[error("transform failed at step '{step}': raw value = {raw_value}")]
    TransformError { step: String, raw_value: String },

    #[error("decision failed at step '{step}': raw value = {raw_value}")]
    DecisionError { step: String, raw_value: String },

    #[error("action failed at step '{step}': raw value = {raw_value}")]
    ActionError { step: String, raw_value: String },
}

// ─── VerifyStep trait ────────────────────────────────────────────────────────

/// A single step in a verification chain.
///
/// Implementors transform an `Input` into an `Output` or return a typed
/// [`VerificationError`] that includes the step name and the raw value that
/// caused the failure.
pub trait VerifyStep {
    type Input;
    type Output;

    /// Human-readable name used in tracing spans and ring-buffer entries.
    fn name(&self) -> &str;

    /// Execute this step, consuming `input`.
    fn run(&self, input: Self::Input) -> Result<Self::Output, VerificationError>;
}

// ─── VerificationResult ──────────────────────────────────────────────────────

/// One recorded outcome stored in a [`HotVerificationChain`] ring buffer.
#[derive(Debug, Clone)]
pub struct VerificationResult {
    pub chain_name: String,
    pub step_name: String,
    pub success: bool,
    pub error: Option<String>,
    pub timestamp: DateTime<Utc>,
}

// ─── ColdVerificationChain ───────────────────────────────────────────────────

/// Synchronous verification chain.
///
/// Wraps each step invocation in a `tracing::info_span` tagged with the chain
/// name and the step name. On success the output of one step is passed as the
/// input to the next. On failure the error propagates immediately — no further
/// steps are executed.
///
/// # Usage
/// ```rust,no_run
/// use rc_common::verification::{ColdVerificationChain, VerificationError, VerifyStep};
///
/// struct ParseVersion;
/// impl VerifyStep for ParseVersion {
///     type Input = String;
///     type Output = u32;
///     fn name(&self) -> &str { "parse_version" }
///     fn run(&self, input: String) -> Result<u32, VerificationError> {
///         input.trim().parse::<u32>().map_err(|_| VerificationError::InputParseError {
///             step: self.name().to_string(),
///             raw_value: input.clone(),
///         })
///     }
/// }
///
/// let chain = ColdVerificationChain::new("config_load");
/// let version = chain.execute_step(&ParseVersion, "42".to_string()).unwrap();
/// assert_eq!(version, 42);
/// ```
pub struct ColdVerificationChain {
    name: String,
}

impl ColdVerificationChain {
    /// Create a new cold chain with the given name.
    /// The name appears in every tracing span emitted during execution.
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }

    /// Execute `step` with `input`, wrapping the call in a tracing span.
    ///
    /// Returns the step output on success, or the step's [`VerificationError`]
    /// (with raw_value embedded) on failure.
    pub fn execute_step<S>(&self, step: &S, input: S::Input) -> Result<S::Output, VerificationError>
    where
        S: VerifyStep,
    {
        let span = tracing::info_span!(
            target: "verification",
            "chain_step",
            chain = %self.name,
            step = step.name(),
        );
        let _enter = span.enter();
        step.run(input)
    }
}

// ─── HotVerificationChain ────────────────────────────────────────────────────

/// Async fire-and-forget verification chain (requires `tokio` feature).
///
/// Results are stored in an in-memory ring buffer (capacity 64). The caller is
/// never blocked. Use [`HotVerificationChain::recent_results`] to read the
/// last N outcomes for diagnostics.
#[cfg(feature = "tokio")]
pub struct HotVerificationChain {
    name: String,
    results: std::sync::Arc<std::sync::Mutex<std::collections::VecDeque<VerificationResult>>>,
}

#[cfg(feature = "tokio")]
impl HotVerificationChain {
    const RING_CAPACITY: usize = 64;

    /// Create a new hot chain with the given name.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            results: std::sync::Arc::new(std::sync::Mutex::new(std::collections::VecDeque::with_capacity(
                Self::RING_CAPACITY,
            ))),
        }
    }

    /// Execute `step` with `input` on a background tokio task, fire-and-forget.
    ///
    /// The result (success or error) is appended to the ring buffer. If the
    /// buffer is full the oldest entry is dropped.
    pub fn execute_step_async<S>(&self, step: S, input: S::Input)
    where
        S: VerifyStep + Send + 'static,
        S::Input: Send + 'static,
        S::Output: Send + 'static,
    {
        let chain_name = self.name.clone();
        let results = std::sync::Arc::clone(&self.results);

        tokio::spawn(async move {
            let step_name = step.name().to_string();
            let outcome = step.run(input);
            let record = VerificationResult {
                chain_name,
                step_name,
                success: outcome.is_ok(),
                error: outcome.err().map(|e| e.to_string()),
                timestamp: Utc::now(),
            };
            let mut guard = results.lock().unwrap_or_else(|p| p.into_inner());
            if guard.len() >= HotVerificationChain::RING_CAPACITY {
                guard.pop_front();
            }
            guard.push_back(record);
        });
    }

    /// Return a snapshot of all results currently in the ring buffer (oldest first).
    pub fn recent_results(&self) -> Vec<VerificationResult> {
        let guard = self.results.lock().unwrap_or_else(|p| p.into_inner());
        guard.iter().cloned().collect()
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Test helpers ──────────────────────────────────────────────────────────

    struct PassStep {
        name: &'static str,
        suffix: &'static str,
    }
    impl VerifyStep for PassStep {
        type Input = String;
        type Output = String;
        fn name(&self) -> &str { self.name }
        fn run(&self, input: String) -> Result<String, VerificationError> {
            Ok(format!("{}{}", input, self.suffix))
        }
    }

    struct FailStep {
        name: &'static str,
    }
    impl VerifyStep for FailStep {
        type Input = String;
        type Output = String;
        fn name(&self) -> &str { self.name }
        fn run(&self, input: String) -> Result<String, VerificationError> {
            Err(VerificationError::TransformError {
                step: self.name.to_string(),
                raw_value: input,
            })
        }
    }

    // ── VerificationError display tests ──────────────────────────────────────

    #[test]
    fn verification_error_input_parse_display_contains_raw_value() {
        let err = VerificationError::InputParseError {
            step: "parse_toml".to_string(),
            raw_value: "bad_bytes".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("bad_bytes"), "Display must contain raw_value: {msg}");
        assert!(msg.contains("parse_toml"), "Display must contain step name: {msg}");
    }

    #[test]
    fn verification_error_transform_display_contains_raw_value() {
        let err = VerificationError::TransformError {
            step: "normalize".to_string(),
            raw_value: "0xff".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("0xff"), "Display must contain raw_value: {msg}");
    }

    #[test]
    fn verification_error_decision_display_contains_raw_value() {
        let err = VerificationError::DecisionError {
            step: "check_threshold".to_string(),
            raw_value: "999".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("999"), "Display must contain raw_value: {msg}");
    }

    #[test]
    fn verification_error_action_display_contains_raw_value() {
        let err = VerificationError::ActionError {
            step: "write_db".to_string(),
            raw_value: "row_data".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("row_data"), "Display must contain raw_value: {msg}");
    }

    // ── ColdVerificationChain tests ───────────────────────────────────────────

    #[test]
    fn cold_chain_single_passing_step_returns_output() {
        let chain = ColdVerificationChain::new("test_chain");
        let step = PassStep { name: "step1", suffix: "_ok" };
        let result = chain.execute_step(&step, "input".to_string());
        assert_eq!(result.unwrap(), "input_ok");
    }

    #[test]
    fn cold_chain_three_steps_all_pass_returns_final_output() {
        let chain = ColdVerificationChain::new("three_step_chain");
        let s1 = PassStep { name: "step1", suffix: "_a" };
        let s2 = PassStep { name: "step2", suffix: "_b" };
        let s3 = PassStep { name: "step3", suffix: "_c" };

        let r1 = chain.execute_step(&s1, "start".to_string()).unwrap();
        let r2 = chain.execute_step(&s2, r1).unwrap();
        let r3 = chain.execute_step(&s3, r2).unwrap();
        assert_eq!(r3, "start_a_b_c");
    }

    #[test]
    fn cold_chain_step2_fails_returns_transform_error_with_raw_value() {
        let chain = ColdVerificationChain::new("fail_chain");
        let s1 = PassStep { name: "step1", suffix: "_ok" };
        let s2 = FailStep { name: "step2" };

        let r1 = chain.execute_step(&s1, "raw".to_string()).unwrap();
        // r1 = "raw_ok" — this becomes the raw_value in the TransformError
        let err = chain.execute_step(&s2, r1.clone()).unwrap_err();
        match &err {
            VerificationError::TransformError { step, raw_value } => {
                assert_eq!(step, "step2");
                assert_eq!(raw_value, &r1, "raw_value must be the input to the failing step");
            }
            other => panic!("expected TransformError, got {:?}", other),
        }
    }

    // ── HotVerificationChain compile test (feature-gated) ────────────────────

    #[cfg(feature = "tokio")]
    #[tokio::test]
    async fn hot_chain_exists_and_stores_results() {
        let chain = HotVerificationChain::new("hot_test");
        let step = PassStep { name: "hot_step", suffix: "_done" };
        chain.execute_step_async(step, "value".to_string());

        // Give the spawned task a tick to complete
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;

        let results = chain.recent_results();
        assert!(!results.is_empty(), "ring buffer should have at least one result");
        assert!(results[0].success, "step should have succeeded");
        assert_eq!(results[0].step_name, "hot_step");
    }

    #[cfg(feature = "tokio")]
    #[tokio::test]
    async fn hot_chain_ring_buffer_records_failure() {
        let chain = HotVerificationChain::new("hot_fail_test");
        let step = FailStep { name: "fail_hot_step" };
        chain.execute_step_async(step, "bad_input".to_string());

        tokio::time::sleep(std::time::Duration::from_millis(20)).await;

        let results = chain.recent_results();
        assert!(!results.is_empty(), "ring buffer should have at least one result");
        assert!(!results[0].success, "step should have failed");
        assert!(results[0].error.is_some(), "error string should be present");
        let err_msg = results[0].error.as_ref().unwrap();
        assert!(err_msg.contains("bad_input"), "error must mention the raw value");
    }
}
