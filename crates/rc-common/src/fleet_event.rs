//! FleetEvent types and broadcast event bus for the Meshed Intelligence pipeline.
//!
//! Phase 273-01: Replaces the single-consumer mpsc model with a broadcast-based
//! event bus that fans out to multiple subscribers (tier engine, experience scorer,
//! fleet coordinator) and emits events the MOMENT a threshold is crossed.
//!
//! All types are in rc-common so both rc-agent and racecontrol can share them.
//! Types use String fields (not rc-agent internal enums) to avoid cross-crate deps.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Core event type broadcast through the fleet event bus.
///
/// Each variant represents a lifecycle moment in the Meshed Intelligence pipeline:
/// anomaly detection, predictive alerting, fix application, fix failure, and escalation.
///
/// Clone is required for `tokio::sync::broadcast` (each subscriber gets a clone).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FleetEvent {
    /// Diagnostic engine detected an anomaly crossing a threshold.
    /// `trigger` and `severity` are stringified from rc-agent's DiagnosticTrigger
    /// because rc-common cannot depend on rc-agent types.
    AnomalyDetected {
        trigger: String,
        severity: String,
        node_id: String,
        timestamp: DateTime<Utc>,
        /// JSON-serialized pod state snapshot for downstream consumers
        pod_state_snapshot: String,
    },

    /// Predictive maintenance detected degradation before failure.
    PredictiveAlert {
        alert_type: String,
        severity: String,
        message: String,
        metric_value: f64,
        threshold: f64,
        node_id: String,
        timestamp: DateTime<Utc>,
    },

    /// A fix was successfully applied by the tier engine.
    FixApplied {
        node_id: String,
        tier: u8,
        action: String,
        trigger: String,
        timestamp: DateTime<Utc>,
    },

    /// A fix attempt failed.
    FixFailed {
        node_id: String,
        tier: u8,
        reason: String,
        trigger: String,
        timestamp: DateTime<Utc>,
    },

    /// Issue escalated to a higher tier or to staff.
    Escalated {
        node_id: String,
        tier: u8,
        reason: String,
        timestamp: DateTime<Utc>,
    },

    // ─── Phase 0 shared variants for parallel phases 275/276/277 ──────

    /// Phase 275: Game launch retry completed (success or exhausted).
    GameLaunchRetryResult {
        node_id: String,
        attempt: u32,
        success: bool,
        cause: String,
        fix_applied: Option<String>,
        timestamp: DateTime<Utc>,
    },

    /// Phase 276: Per-pod experience score updated.
    ExperienceScoreUpdate {
        node_id: String,
        score: f64,
        status: String,
        timestamp: DateTime<Utc>,
    },

    /// Phase 277: Revenue anomaly detected (game without billing, billing without game).
    RevenueAnomaly {
        anomaly_type: String,
        detail: String,
        node_id: String,
        timestamp: DateTime<Utc>,
    },

    /// Phase 277: Model reputation changed (demoted or promoted).
    ModelReputationChange {
        model_id: String,
        old_accuracy: f64,
        new_accuracy: f64,
        action: String,
        timestamp: DateTime<Utc>,
    },
}

/// An Incident is the work unit sent via mpsc to the tier engine.
/// Wraps a FleetEvent with tracking metadata (ID, idempotency).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Incident {
    /// Unique incident ID (UUID v4)
    pub id: String,
    /// The source event that created this incident
    pub source_event: FleetEvent,
    /// When this incident was created
    pub created_at: DateTime<Utc>,
    /// Idempotency key for deduplication (populated by Plan 03)
    pub idempotency_key: Option<String>,
}

impl Incident {
    /// Create a new Incident from a FleetEvent with a fresh UUID.
    pub fn new(source_event: FleetEvent) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            source_event,
            created_at: Utc::now(),
            idempotency_key: None,
        }
    }
}

/// Broadcast event bus for FleetEvents.
///
/// Wraps `tokio::sync::broadcast::Sender<FleetEvent>` with a constructor and
/// subscribe method. Each subscriber gets an independent receiver that sees
/// all events from the point of subscription onward.
///
/// Capacity: 256 events. If a slow subscriber falls behind, it receives a
/// `RecvError::Lagged(n)` indicating how many events were missed.
#[cfg(feature = "tokio")]
pub struct FleetEventBus {
    sender: tokio::sync::broadcast::Sender<FleetEvent>,
}

#[cfg(feature = "tokio")]
impl FleetEventBus {
    /// Create a new broadcast event bus with the given capacity.
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = tokio::sync::broadcast::channel(capacity);
        Self { sender }
    }

    /// Get a clone of the sender for emitting events.
    pub fn sender(&self) -> tokio::sync::broadcast::Sender<FleetEvent> {
        self.sender.clone()
    }

    /// Subscribe to receive all future events.
    pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<FleetEvent> {
        self.sender.subscribe()
    }
}
