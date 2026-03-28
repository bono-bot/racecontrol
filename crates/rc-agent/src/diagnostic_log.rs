//! Shared diagnostic event log — ring buffer of recent tier engine results.
//!
//! Written by tier_engine after each diagnosis. Read by:
//! - remote_ops `/events/recent` endpoint (Phase 1 — kiosk fetches recent events)
//! - ws_handler for DiagnosticResult responses (Phase 2 — staff-triggered diagnosis)
//!
//! Uses parking_lot-style poison recovery: on PoisonError, logs a warning and
//! recovers the inner data via into_inner() rather than silently dropping events.
//! (MMA OpenRouter fix: std::sync::RwLock poisoning made push() a permanent no-op)
//!
//! v27.0: Staff Diagnostic Bridge

use std::collections::VecDeque;
use std::sync::{Arc, RwLock};
use serde::Serialize;

/// Maximum events retained in the ring buffer
const MAX_EVENTS: usize = 50;

/// A single diagnostic event log entry
#[derive(Debug, Clone, Serialize)]
pub struct DiagnosticLogEntry {
    /// ISO-8601 IST timestamp
    pub timestamp: String,
    /// Trigger type (e.g. "GameLaunchFail", "ProcessCrash", "StaffRequest")
    pub trigger: String,
    /// Which tier resolved it (1-5, or 0 if not applicable)
    pub tier: u8,
    /// "fixed", "failed_to_fix", "not_applicable", "stub"
    pub outcome: String,
    /// Action taken (e.g. "cleared MAINTENANCE_MODE", "KB solution applied")
    pub action: String,
    /// Root cause (from KB or model, empty for Tier 1)
    pub root_cause: String,
    /// Fix type classification
    pub fix_type: String,
    /// Confidence score (0.0-1.0, 0 for deterministic)
    pub confidence: f64,
    /// Whether the fix was actually applied
    pub fix_applied: bool,
    /// Problem hash for KB cross-reference
    pub problem_hash: String,
    /// Optional correlation_id for staff-triggered requests
    pub correlation_id: Option<String>,
    /// "autonomous" or "staff"
    pub source: String,
}

/// Thread-safe ring buffer of diagnostic events.
/// Uses std::sync::RwLock with poison recovery for resilience.
#[derive(Clone)]
pub struct DiagnosticLog {
    entries: Arc<RwLock<VecDeque<DiagnosticLogEntry>>>,
}

impl DiagnosticLog {
    pub fn new() -> Self {
        Self {
            entries: Arc::new(RwLock::new(VecDeque::with_capacity(MAX_EVENTS))),
        }
    }

    /// Append a new entry; drops oldest if buffer is full.
    /// Recovers from RwLock poisoning rather than silently dropping events.
    pub async fn push(&self, entry: DiagnosticLogEntry) {
        let mut guard = match self.entries.write() {
            Ok(g) => g,
            Err(poisoned) => {
                tracing::warn!(target: "diagnostic-log", "RwLock was poisoned — recovering inner data");
                poisoned.into_inner()
            }
        };
        if guard.len() >= MAX_EVENTS {
            guard.pop_front();
        }
        guard.push_back(entry);
    }

    /// Get the N most recent entries (newest first).
    /// Recovers from RwLock poisoning rather than returning empty.
    pub async fn recent(&self, limit: usize) -> Vec<DiagnosticLogEntry> {
        let guard = match self.entries.read() {
            Ok(g) => g,
            Err(poisoned) => {
                tracing::warn!(target: "diagnostic-log", "RwLock was poisoned on read — recovering");
                poisoned.into_inner()
            }
        };
        guard.iter().rev().take(limit).cloned().collect()
    }
}
