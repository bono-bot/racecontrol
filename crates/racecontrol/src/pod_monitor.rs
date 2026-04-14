//! Pod Monitor: Heartbeat detector for pod liveness.
//!
//! Runs as a background task on racecontrol. Checks all known pods every N seconds,
//! marks them Offline if heartbeat is stale, and resets state on natural recovery.
//!
//! Recovery actions (WoL, rc-agent restart, AI escalation, staff alert) are handled
//! exclusively by pod_healer's graduated recovery tracker. pod_monitor is a pure detector.
//!
//! WatchdogState is reset to Healthy on natural recovery (fresh heartbeat arrives).
//! The WatchdogState::Restarting / Verifying skip guard is kept so pod_healer's
//! in-progress recovery cycle is not interrupted.
//!
//! Key invariants:
//! - Pod in Restarting or Verifying state is NEVER double-triggered
//! - Pod with active billing is NEVER flagged for restart
//! - Natural recovery (fresh heartbeat while attempt > 0) resets WatchdogState to Healthy
//! - ALL repair actions (WoL, exec, alert) are delegated to pod_healer

#[path = "pod_monitor_check.rs"]
mod pod_monitor_check;

#[path = "pod_monitor_helpers.rs"]
pub mod pod_monitor_helpers;

#[path = "pod_monitor_tests.rs"]
mod pod_monitor_tests;

// Re-export public API
pub use pod_monitor_check::spawn;
pub use pod_monitor_helpers::{determine_failure_reason, failure_type_from_reason};
