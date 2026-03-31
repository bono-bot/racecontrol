//! Safety guardrails for the tier engine: blast radius limiter, per-action
//! circuit breaker, and idempotency tracker.
//!
//! All types are thread-safe and designed for use in async contexts.
//! Lock scopes are kept tight (no lock held across `.await`).

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Instant;

/// Log target for safety module tracing.
const LOG_TARGET: &str = "safety";

// ──────────────────────────────────────────────────────────────────────
// SAFE-01: Blast Radius Limiter
// ──────────────────────────────────────────────────────────────────────

/// Maximum number of concurrent fixes allowed across the entire pod.
const MAX_CONCURRENT_FIXES: usize = 2;
/// Maximum concurrent fixes for a single action type (e.g., "kill_process").
const MAX_PER_ACTION: usize = 1;

/// Tracks an active fix being applied.
#[derive(Debug, Clone)]
pub struct ActiveFix {
    pub action_type: String,
    pub target: String,
    pub started_at: Instant,
}

/// Limits the blast radius of concurrent fixes to prevent cascading damage.
///
/// Uses interior mutability via `Mutex<HashMap>` with tight lock scopes.
/// The RAII `FixGuard` auto-releases the slot on drop.
pub struct BlastRadiusLimiter {
    active: Mutex<HashMap<String, ActiveFix>>,
}

impl BlastRadiusLimiter {
    pub fn new() -> Self {
        Self {
            active: Mutex::new(HashMap::new()),
        }
    }

    /// Try to acquire a fix slot. Returns `Some(FixGuard)` if allowed,
    /// `None` if the blast radius limit would be exceeded.
    ///
    /// The guard auto-releases the slot on drop (RAII pattern).
    pub fn try_acquire(
        &self,
        fix_id: String,
        action_type: String,
        target: String,
    ) -> Option<FixGuard<'_>> {
        let mut active = match self.active.lock() {
            Ok(g) => g,
            Err(poisoned) => {
                tracing::warn!(target: LOG_TARGET, "BlastRadiusLimiter lock poisoned — recovering");
                poisoned.into_inner()
            }
        };

        // Check global limit
        if active.len() >= MAX_CONCURRENT_FIXES {
            tracing::warn!(
                target: LOG_TARGET,
                current = active.len(),
                max = MAX_CONCURRENT_FIXES,
                action_type = %action_type,
                target = %target,
                "Blast radius limit reached — rejecting fix"
            );
            return None;
        }

        // Check per-action limit
        let same_action_count = active
            .values()
            .filter(|f| f.action_type == action_type)
            .count();
        if same_action_count >= MAX_PER_ACTION {
            tracing::warn!(
                target: LOG_TARGET,
                action_type = %action_type,
                current = same_action_count,
                max = MAX_PER_ACTION,
                "Per-action blast radius limit reached — rejecting fix"
            );
            return None;
        }

        // Check for duplicate fix_id (same fix already in flight)
        if active.contains_key(&fix_id) {
            tracing::debug!(
                target: LOG_TARGET,
                fix_id = %fix_id,
                "Fix already in flight — rejecting duplicate"
            );
            return None;
        }

        active.insert(
            fix_id.clone(),
            ActiveFix {
                action_type: action_type.clone(),
                target: target.clone(),
                started_at: Instant::now(),
            },
        );

        tracing::debug!(
            target: LOG_TARGET,
            fix_id = %fix_id,
            action_type = %action_type,
            target = %target,
            active_count = active.len(),
            "Fix slot acquired"
        );

        Some(FixGuard {
            limiter: self,
            fix_id,
        })
    }

    /// Returns the number of currently active fixes (for diagnostics/metrics).
    pub fn active_count(&self) -> usize {
        match self.active.lock() {
            Ok(g) => g.len(),
            Err(poisoned) => poisoned.into_inner().len(),
        }
    }

    /// Release a fix slot by ID. Called by `FixGuard::drop`.
    fn release(&self, fix_id: &str) {
        let mut active = match self.active.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        };
        if let Some(fix) = active.remove(fix_id) {
            tracing::debug!(
                target: LOG_TARGET,
                fix_id = %fix_id,
                duration_ms = fix.started_at.elapsed().as_millis() as u64,
                "Fix slot released"
            );
        }
    }
}

impl Default for BlastRadiusLimiter {
    fn default() -> Self {
        Self::new()
    }
}

/// RAII guard that auto-releases the blast radius slot on drop.
pub struct FixGuard<'a> {
    limiter: &'a BlastRadiusLimiter,
    fix_id: String,
}

impl<'a> std::fmt::Debug for FixGuard<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FixGuard")
            .field("fix_id", &self.fix_id)
            .finish()
    }
}

impl<'a> Drop for FixGuard<'a> {
    fn drop(&mut self) {
        self.limiter.release(&self.fix_id);
    }
}

// ──────────────────────────────────────────────────────────────────────
// SAFE-02: Per-Action Circuit Breaker
// ──────────────────────────────────────────────────────────────────────

/// Default failure threshold before the breaker opens for an action type.
const ACTION_CB_THRESHOLD: u32 = 3;
/// Default cooldown in seconds before the breaker resets.
const ACTION_CB_COOLDOWN_SECS: u64 = 300;

/// State of a single action-type circuit breaker.
#[derive(Debug, Clone)]
struct ActionBreakerState {
    consecutive_failures: u32,
    last_failure: Option<Instant>,
}

/// Per-action-type circuit breaker. Each action type (e.g., "kill_process",
/// "clear_sentinel", "mma_call") gets its own independent breaker.
///
/// This enhances the original single `CircuitBreaker` in tier_engine.rs
/// by preventing one failing action type from blocking all other action types.
pub struct PerActionCircuitBreaker {
    breakers: Mutex<HashMap<String, ActionBreakerState>>,
    threshold: u32,
    cooldown_secs: u64,
}

impl PerActionCircuitBreaker {
    pub fn new() -> Self {
        Self {
            breakers: Mutex::new(HashMap::new()),
            threshold: ACTION_CB_THRESHOLD,
            cooldown_secs: ACTION_CB_COOLDOWN_SECS,
        }
    }

    /// Create with custom threshold and cooldown.
    pub fn with_config(threshold: u32, cooldown_secs: u64) -> Self {
        Self {
            breakers: Mutex::new(HashMap::new()),
            threshold,
            cooldown_secs,
        }
    }

    /// Check if the circuit breaker for a given action type is open (should skip).
    pub fn is_open(&self, action_type: &str) -> bool {
        let breakers = match self.breakers.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        };

        let state = match breakers.get(action_type) {
            Some(s) => s,
            None => return false, // No state = never failed = closed
        };

        if state.consecutive_failures < self.threshold {
            return false;
        }

        // Check cooldown — if cooldown elapsed, breaker is half-open (allow retry)
        match state.last_failure {
            Some(t) => {
                if t.elapsed().as_secs() < self.cooldown_secs {
                    true // Still in cooldown
                } else {
                    false // Cooldown elapsed — half-open, allow one retry
                }
            }
            None => false,
        }
    }

    /// Record a successful action — resets the breaker for this action type.
    pub fn record_success(&self, action_type: &str) {
        let mut breakers = match self.breakers.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        };
        breakers.remove(action_type);
    }

    /// Record a failed action — increments failure count.
    pub fn record_failure(&self, action_type: &str) {
        let mut breakers = match self.breakers.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        };

        let state = breakers
            .entry(action_type.to_string())
            .or_insert(ActionBreakerState {
                consecutive_failures: 0,
                last_failure: None,
            });

        state.consecutive_failures += 1;
        state.last_failure = Some(Instant::now());

        if state.consecutive_failures >= self.threshold {
            tracing::warn!(
                target: LOG_TARGET,
                action_type = %action_type,
                failures = state.consecutive_failures,
                cooldown_secs = self.cooldown_secs,
                "Per-action circuit breaker OPEN for '{}' — {} consecutive failures",
                action_type,
                state.consecutive_failures
            );
        }
    }

    /// Get a snapshot of all breaker states for diagnostics.
    pub fn snapshot(&self) -> HashMap<String, (u32, bool)> {
        let breakers = match self.breakers.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        };

        breakers
            .iter()
            .map(|(k, v)| {
                let is_open = v.consecutive_failures >= self.threshold
                    && v.last_failure
                        .map(|t| t.elapsed().as_secs() < self.cooldown_secs)
                        .unwrap_or(false);
                (k.clone(), (v.consecutive_failures, is_open))
            })
            .collect()
    }
}

impl Default for PerActionCircuitBreaker {
    fn default() -> Self {
        Self::new()
    }
}

// ──────────────────────────────────────────────────────────────────────
// SAFE-03: Idempotency Tracker
// ──────────────────────────────────────────────────────────────────────

/// Default TTL for idempotency entries (10 minutes).
const IDEMPOTENCY_TTL_SECS: u64 = 600;
/// Cleanup stale entries when map exceeds this size.
const IDEMPOTENCY_CLEANUP_THRESHOLD: usize = 500;

/// Tracks recently applied fixes to prevent duplicate application.
///
/// Key = `{node_id}:{rule_version}:{incident_fingerprint}`
/// Value = `Instant` when the fix was first applied.
///
/// Entries expire after `IDEMPOTENCY_TTL_SECS` and are cleaned up
/// periodically when the map exceeds `IDEMPOTENCY_CLEANUP_THRESHOLD`.
pub struct IdempotencyTracker {
    seen: Mutex<HashMap<String, Instant>>,
    ttl_secs: u64,
}

impl IdempotencyTracker {
    pub fn new() -> Self {
        Self {
            seen: Mutex::new(HashMap::new()),
            ttl_secs: IDEMPOTENCY_TTL_SECS,
        }
    }

    /// Create with custom TTL.
    pub fn with_ttl(ttl_secs: u64) -> Self {
        Self {
            seen: Mutex::new(HashMap::new()),
            ttl_secs,
        }
    }

    /// Build an idempotency key from components.
    pub fn make_key(node_id: &str, rule_version: &str, incident_fingerprint: &str) -> String {
        format!("{}:{}:{}", node_id, rule_version, incident_fingerprint)
    }

    /// Check if a fix has already been applied within the TTL window.
    /// Returns `true` if this is a duplicate (should skip), `false` if novel.
    ///
    /// If novel, records it immediately so subsequent calls return `true`.
    pub fn check_and_record(&self, key: &str) -> bool {
        let mut seen = match self.seen.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        };

        let now = Instant::now();

        // Check if already seen and not expired
        if let Some(recorded_at) = seen.get(key) {
            if now.duration_since(*recorded_at).as_secs() < self.ttl_secs {
                tracing::debug!(
                    target: LOG_TARGET,
                    key = %key,
                    age_secs = now.duration_since(*recorded_at).as_secs(),
                    ttl_secs = self.ttl_secs,
                    "Idempotency check: DUPLICATE — skipping"
                );
                return true; // Duplicate
            }
            // Expired — fall through to re-record
        }

        // Record and cleanup if needed
        seen.insert(key.to_string(), now);

        if seen.len() > IDEMPOTENCY_CLEANUP_THRESHOLD {
            let ttl = self.ttl_secs;
            seen.retain(|_, v| now.duration_since(*v).as_secs() < ttl);
            tracing::debug!(
                target: LOG_TARGET,
                remaining = seen.len(),
                "Idempotency tracker: cleaned up expired entries"
            );
        }

        false // Novel — proceed with fix
    }

    /// Check without recording (peek). Returns `true` if duplicate.
    pub fn is_duplicate(&self, key: &str) -> bool {
        let seen = match self.seen.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        };
        let now = Instant::now();
        seen.get(key)
            .map(|recorded_at| now.duration_since(*recorded_at).as_secs() < self.ttl_secs)
            .unwrap_or(false)
    }

    /// Returns the number of active (non-expired) entries.
    pub fn active_count(&self) -> usize {
        let seen = match self.seen.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        };
        let now = Instant::now();
        seen.values()
            .filter(|v| now.duration_since(**v).as_secs() < self.ttl_secs)
            .count()
    }
}

impl Default for IdempotencyTracker {
    fn default() -> Self {
        Self::new()
    }
}

// ──────────────────────────────────────────────────────────────────────
// SafetyGuardrails: Combined facade
// ──────────────────────────────────────────────────────────────────────

/// Combined safety guardrails: blast radius + circuit breaker + idempotency.
///
/// This is the single entry point for tier_engine.rs to check all safety
/// conditions before applying a fix.
pub struct SafetyGuardrails {
    pub blast_radius: BlastRadiusLimiter,
    pub circuit_breaker: PerActionCircuitBreaker,
    pub idempotency: IdempotencyTracker,
}

impl SafetyGuardrails {
    pub fn new() -> Self {
        Self {
            blast_radius: BlastRadiusLimiter::new(),
            circuit_breaker: PerActionCircuitBreaker::new(),
            idempotency: IdempotencyTracker::new(),
        }
    }

    /// Pre-flight safety check before applying a fix.
    ///
    /// Returns `Err(reason)` if the fix should be skipped, or
    /// `Ok(FixGuard)` if all checks pass (guard auto-releases on drop).
    pub fn pre_check(
        &self,
        fix_id: &str,
        action_type: &str,
        target: &str,
        node_id: &str,
        rule_version: &str,
        incident_fingerprint: &str,
    ) -> Result<FixGuard<'_>, String> {
        // 1. Circuit breaker check
        if self.circuit_breaker.is_open(action_type) {
            return Err(format!(
                "Circuit breaker OPEN for action type '{}'",
                action_type
            ));
        }

        // 2. Idempotency check
        let idem_key =
            IdempotencyTracker::make_key(node_id, rule_version, incident_fingerprint);
        if self.idempotency.check_and_record(&idem_key) {
            return Err(format!(
                "Duplicate fix detected (idempotency key: {})",
                idem_key
            ));
        }

        // 3. Blast radius check
        match self.blast_radius.try_acquire(
            fix_id.to_string(),
            action_type.to_string(),
            target.to_string(),
        ) {
            Some(guard) => Ok(guard),
            None => Err(format!(
                "Blast radius limit exceeded (action: {}, target: {})",
                action_type, target
            )),
        }
    }
}

impl Default for SafetyGuardrails {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blast_radius_basic() {
        let limiter = BlastRadiusLimiter::new();

        // First fix should succeed
        let guard1 = limiter
            .try_acquire("fix-1".into(), "kill_process".into(), "pod-1".into());
        assert!(guard1.is_some());
        assert_eq!(limiter.active_count(), 1);

        // Same action type should be rejected (per-action limit = 1)
        let guard2 = limiter
            .try_acquire("fix-2".into(), "kill_process".into(), "pod-2".into());
        assert!(guard2.is_none());

        // Different action type should succeed (global limit = 2)
        let guard3 = limiter
            .try_acquire("fix-3".into(), "clear_sentinel".into(), "pod-1".into());
        assert!(guard3.is_some());
        assert_eq!(limiter.active_count(), 2);

        // Third concurrent fix should be rejected (global limit = 2)
        let guard4 = limiter
            .try_acquire("fix-4".into(), "restart_service".into(), "pod-3".into());
        assert!(guard4.is_none());

        // Drop first guard — should free a slot
        drop(guard1);
        assert_eq!(limiter.active_count(), 1);

        // Now a new fix should succeed
        let guard5 = limiter
            .try_acquire("fix-5".into(), "restart_service".into(), "pod-3".into());
        assert!(guard5.is_some());
    }

    #[test]
    fn blast_radius_duplicate_id() {
        let limiter = BlastRadiusLimiter::new();
        let _guard = limiter
            .try_acquire("fix-1".into(), "kill_process".into(), "pod-1".into());
        assert!(_guard.is_some());

        // Same fix_id should be rejected
        let dup = limiter
            .try_acquire("fix-1".into(), "different_action".into(), "pod-2".into());
        assert!(dup.is_none());
    }

    #[test]
    fn circuit_breaker_per_action() {
        let cb = PerActionCircuitBreaker::with_config(2, 300);

        assert!(!cb.is_open("kill_process"));

        // One failure — still closed
        cb.record_failure("kill_process");
        assert!(!cb.is_open("kill_process"));

        // Second failure — opens
        cb.record_failure("kill_process");
        assert!(cb.is_open("kill_process"));

        // Different action type should be unaffected
        assert!(!cb.is_open("clear_sentinel"));

        // Success resets
        cb.record_success("kill_process");
        assert!(!cb.is_open("kill_process"));
    }

    #[test]
    fn idempotency_basic() {
        let tracker = IdempotencyTracker::new();
        let key = IdempotencyTracker::make_key("pod-1", "v1", "crash-abc123");

        // First check — novel
        assert!(!tracker.check_and_record(&key));

        // Second check — duplicate
        assert!(tracker.check_and_record(&key));

        // Different key — novel
        let key2 = IdempotencyTracker::make_key("pod-2", "v1", "crash-abc123");
        assert!(!tracker.check_and_record(&key2));
    }

    #[test]
    fn idempotency_peek() {
        let tracker = IdempotencyTracker::new();
        let key = "test-key";

        // Peek before recording — not duplicate
        assert!(!tracker.is_duplicate(key));

        // Record it
        assert!(!tracker.check_and_record(key));

        // Peek after recording — duplicate
        assert!(tracker.is_duplicate(key));
    }

    #[test]
    fn safety_guardrails_combined() {
        let safety = SafetyGuardrails::new();

        // First fix — all checks pass
        let result = safety.pre_check(
            "fix-1",
            "kill_process",
            "pod-1",
            "pod-1",
            "v1",
            "crash-abc",
        );
        assert!(result.is_ok());

        // Same incident fingerprint — idempotency blocks
        let result2 = safety.pre_check(
            "fix-2",
            "kill_process",
            "pod-1",
            "pod-1",
            "v1",
            "crash-abc",
        );
        assert!(result2.is_err());
        assert!(result2
            .unwrap_err()
            .contains("Duplicate fix"));
    }
}
