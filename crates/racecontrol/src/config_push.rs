/// v22.0 Phase 177: Config push handler — validation, queuing, delivery, and audit logging.
/// Phase 296: Full config push — store/retrieve/push full AgentConfig per pod.
///
/// Endpoints:
///   POST /api/v1/config/push           — validate, queue, and deliver field-level config to pods
///   GET  /api/v1/config/push/queue     — view the per-pod delivery queue
///   GET  /api/v1/config/audit          — view the audit log of all config push events
///   POST /api/v1/config/pod/{pod_id}   — store full AgentConfig for a pod (Phase 296)
///   GET  /api/v1/config/pod/{pod_id}   — retrieve stored AgentConfig for a pod (Phase 296)

#[path = "config_push_types.rs"]
pub mod config_push_types;

#[path = "config_push_handlers.rs"]
pub mod config_push_handlers;

#[path = "config_push_full.rs"]
pub mod config_push_full;

#[path = "config_push_replay.rs"]
pub mod config_push_replay;

#[path = "config_push_tests.rs"]
mod config_push_tests;

// Re-export public API for existing callers (crate::config_push::X)
pub use config_push_types::{AuditLogEntry, ConfigQueueEntry, PushConfigRequest, validate_config_push};
pub use config_push_handlers::{push_config, get_queue, get_audit_log};
pub use config_push_full::{
    compute_config_hash, store_pod_config, get_pod_config,
    push_full_config_to_pod, set_pod_config, get_pod_config_handler,
};
pub use config_push_replay::replay_pending_config_pushes;
