//! Meshed Intelligence debug state — stores recent triggers and last diagnosis
//! for the /mi/debug HTTP endpoint (Phase 322, MIG-01).
//!
//! Thread-safe static storage using OnceLock<Arc<Mutex<...>>>.
//! Pure std — no async, no tokio.

use rc_common::diagnostic_types::{DiagnosticTrigger, TierDiagnosis};
use serde::Serialize;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex, OnceLock};

const MAX_RECENT_TRIGGERS: usize = 20;

/// Internal mutable state for MI debug info.
pub struct MiDebugState {
    recent_triggers: VecDeque<(String, DiagnosticTrigger)>,
    last_diagnosis: Option<TierDiagnosis>,
}

/// Serializable snapshot returned by the /mi/debug endpoint.
#[derive(Debug, Clone, Serialize)]
pub struct MiDebugSnapshot {
    pub recent_triggers: Vec<(String, DiagnosticTrigger)>,
    pub last_diagnosis: Option<TierDiagnosis>,
}

static MI_DEBUG: OnceLock<Arc<Mutex<MiDebugState>>> = OnceLock::new();

/// Initialize the debug state. Call once at startup.
pub fn init() -> Arc<Mutex<MiDebugState>> {
    let state = Arc::new(Mutex::new(MiDebugState {
        recent_triggers: VecDeque::with_capacity(MAX_RECENT_TRIGGERS + 1),
        last_diagnosis: None,
    }));
    let _ = MI_DEBUG.set(state.clone());
    state
}

/// Get a reference to the debug state. Panics if init() was never called,
/// but callers should always call init() at startup.
pub fn get() -> &'static Arc<Mutex<MiDebugState>> {
    MI_DEBUG.get().expect("MiDebugState not initialized — call mi_debug_state::init() at startup")
}

/// Record a diagnostic trigger with IST timestamp.
pub fn record_trigger(trigger: &DiagnosticTrigger) {
    let timestamp = ist_now();
    if let Some(state) = MI_DEBUG.get() {
        if let Ok(mut guard) = state.lock() {
            guard.recent_triggers.push_back((timestamp, trigger.clone()));
            while guard.recent_triggers.len() > MAX_RECENT_TRIGGERS {
                guard.recent_triggers.pop_front();
            }
        }
    }
}

/// Record the latest tier diagnosis result.
pub fn record_diagnosis(diagnosis: &TierDiagnosis) {
    if let Some(state) = MI_DEBUG.get() {
        if let Ok(mut guard) = state.lock() {
            guard.last_diagnosis = Some(diagnosis.clone());
        }
    }
}

/// Take a snapshot of current debug state for serialization.
pub fn snapshot() -> MiDebugSnapshot {
    match MI_DEBUG.get() {
        Some(state) => match state.lock() {
            Ok(guard) => MiDebugSnapshot {
                recent_triggers: guard.recent_triggers.iter().cloned().collect(),
                last_diagnosis: guard.last_diagnosis.clone(),
            },
            Err(_) => MiDebugSnapshot {
                recent_triggers: vec![],
                last_diagnosis: None,
            },
        },
        None => MiDebugSnapshot {
            recent_triggers: vec![],
            last_diagnosis: None,
        },
    }
}

/// Get current time as IST ISO-8601 string.
fn ist_now() -> String {
    let now = chrono::Utc::now();
    let ist = now + chrono::Duration::hours(5) + chrono::Duration::minutes(30);
    ist.format("%Y-%m-%dT%H:%M:%S+05:30").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rc_common::diagnostic_types::DiagnosticTrigger;

    #[test]
    fn test_record_and_snapshot() {
        // Initialize state (OnceLock -- only first call succeeds in process, but that's fine)
        init();

        let snap_before = snapshot();
        let count_before = snap_before.recent_triggers.len();

        // Record some triggers
        record_trigger(&DiagnosticTrigger::Periodic);
        record_trigger(&DiagnosticTrigger::HealthCheckFail);
        record_trigger(&DiagnosticTrigger::ProcessCrash {
            process_name: "WerFault.exe".to_string(),
        });

        let snap = snapshot();
        // We should have added at least our triggers (may be capped at 20 if other tests ran)
        let new_count = snap.recent_triggers.len();
        assert!(new_count >= count_before.min(17) + 3 || new_count == MAX_RECENT_TRIGGERS,
            "Expected growth or cap, before={} after={}", count_before, new_count);

        // Verify our triggers exist somewhere in the list
        assert!(snap.recent_triggers.iter().any(|(_, t)| matches!(t, DiagnosticTrigger::Periodic)),
            "Periodic trigger should be in snapshot");
        assert!(snap.recent_triggers.iter().any(|(_, t)| matches!(t, DiagnosticTrigger::HealthCheckFail)),
            "HealthCheckFail trigger should be in snapshot");

        // Record a diagnosis
        let diag = rc_common::diagnostic_types::TierDiagnosis {
            trigger: DiagnosticTrigger::Periodic,
            tier: 1,
            outcome: "fixed".to_string(),
            action: "cleared maintenance mode".to_string(),
            root_cause: "stale sentinel".to_string(),
            fix_type: "deterministic".to_string(),
            confidence: 1.0,
            fix_applied: true,
            problem_hash: "test123".to_string(),
            timestamp: "2026-04-06T14:00:00+05:30".to_string(),
        };
        record_diagnosis(&diag);

        let snap2 = snapshot();
        assert!(snap2.last_diagnosis.is_some());
        assert_eq!(snap2.last_diagnosis.as_ref().map(|d| d.tier), Some(1));
    }

    #[test]
    fn test_trigger_trimming() {
        init();
        // Record 25 triggers -- should trim to max 20
        for i in 0..25 {
            record_trigger(&DiagnosticTrigger::ErrorSpike { errors_per_min: i });
        }
        let snap = snapshot();
        assert!(snap.recent_triggers.len() <= MAX_RECENT_TRIGGERS,
            "Expected at most {} triggers, got {}", MAX_RECENT_TRIGGERS, snap.recent_triggers.len());
    }
}
