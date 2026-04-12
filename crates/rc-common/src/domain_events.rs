//! Domain Events — typed business events for the RaceControl event bus.
//!
//! Every department publishes typed events; other departments subscribe.
//! Events are immutable facts. Game-related events reference launch_contract types.
//! All events are Serialize + Deserialize for persistence and WS transport.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::launch_contract::GameType;

/// The master event envelope. Every domain event is wrapped in this.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainEvent {
    /// Unique event ID (generated at publish time).
    pub id: Uuid,
    /// When the event occurred (UTC).
    pub timestamp: DateTime<Utc>,
    /// Which department produced this event.
    pub department: Department,
    /// The typed event payload.
    pub event: Event,
    /// Optional correlation ID to link related events across departments.
    /// E.g. a billing start, game launch, and lap completed can all share one.
    pub correlation_id: Option<Uuid>,
}

impl DomainEvent {
    /// Create a new DomainEvent with a fresh UUID and current timestamp.
    pub fn new(department: Department, event: Event) -> Self {
        Self {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            department,
            event,
            correlation_id: None,
        }
    }

    /// Create a new DomainEvent with a correlation ID for request tracing.
    pub fn with_correlation(department: Department, event: Event, correlation_id: Uuid) -> Self {
        Self {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            department,
            event,
            correlation_id: Some(correlation_id),
        }
    }
}

/// The departments in the RaceControl platform (event routing + filtering).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Department {
    Wallet,
    GameLaunch,
    Racing,
    Billing,
    Cafe,
    Community,
    Marketing,
    Staff,
    Pods,
    Infrastructure,
}

impl std::fmt::Display for Department {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Wallet => write!(f, "Wallet"),
            Self::GameLaunch => write!(f, "Game Launch"),
            Self::Racing => write!(f, "Racing"),
            Self::Billing => write!(f, "Billing"),
            Self::Cafe => write!(f, "Cafe"),
            Self::Community => write!(f, "Community"),
            Self::Marketing => write!(f, "Marketing"),
            Self::Staff => write!(f, "Staff"),
            Self::Pods => write!(f, "Pods"),
            Self::Infrastructure => write!(f, "Infrastructure"),
        }
    }
}

/// All typed domain events. Each variant is a fact about something that happened.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event {
    // Wallet
    /// Credits were added to a customer's wallet.
    WalletTopUp {
        driver_id: i64,
        /// Amount in credits (1 credit = 1 rupee).
        amount: i64,
        /// How the top-up was funded.
        credit_type: CreditType,
    },

    /// Credits were debited from a customer's wallet.
    WalletDebit {
        driver_id: i64,
        /// Amount in credits.
        amount: i64,
        /// Why the debit occurred.
        reason: DebitReason,
    },

    /// Credits were refunded to a customer's wallet.
    WalletRefund {
        driver_id: i64,
        /// Amount in credits.
        amount: i64,
        /// Which session or order triggered the refund, if any.
        reference_id: Option<String>,
    },

    // Game Launch
    /// A game process was started on a pod.
    GameStarted {
        pod_id: u8,
        game: GameType,
        driver_id: i64,
    },

    /// A game process crashed or terminated unexpectedly.
    GameCrashed {
        pod_id: u8,
        game: GameType,
        reason: String,
    },

    /// A game ended normally (customer exited or session expired).
    GameEnded {
        pod_id: u8,
        game: GameType,
        duration_secs: u64,
    },

    // Racing
    /// A lap was completed (references launch_contract::LapCompleted fields).
    LapCompleted {
        driver_id: i64,
        pod_id: u8,
        game: GameType,
        track: String,
        car: String,
        lap_time_ms: u32,
        lap_number: u32,
        is_valid: bool,
        sector_times_ms: Vec<u32>,
    },

    /// A new personal best was set.
    PersonalBest {
        driver_id: i64,
        track: String,
        car: String,
        lap_time_ms: u32,
        previous_best_ms: Option<u32>,
    },

    // Billing
    /// A billing session was started (wallet debited, timer running).
    BillingStarted {
        session_id: i64,
        pod_id: u8,
        driver_id: i64,
        /// Credits debited at session start.
        credits: i64,
    },

    /// A billing session was paused (idle detection or staff override).
    BillingPaused {
        session_id: i64,
        reason: String,
    },

    /// A billing session was resumed after a pause.
    BillingResumed {
        session_id: i64,
    },

    /// A billing session ended (normal, early-end, or cancelled).
    BillingEnded {
        session_id: i64,
        duration_secs: u64,
        credits_used: i64,
        /// Credits refunded (if early-end).
        credits_refunded: i64,
    },

    // Cafe
    /// A cafe order was placed.
    CafeOrderPlaced {
        order_id: i64,
        driver_id: Option<i64>,
        /// List of (item_name, quantity, unit_price_credits).
        items: Vec<CafeOrderItem>,
        total_credits: i64,
    },

    /// A cafe order was completed (served to customer).
    CafeOrderCompleted {
        order_id: i64,
    },

    // Community
    /// A friend request was sent.
    FriendRequestSent {
        from_driver_id: i64,
        to_driver_id: i64,
    },

    /// A friend request was accepted.
    FriendRequestAccepted {
        from_driver_id: i64,
        to_driver_id: i64,
    },

    /// A multiplayer session was created.
    MultiplayerSessionCreated {
        session_id: i64,
        host_driver_id: i64,
        pod_ids: Vec<u8>,
        game: GameType,
    },

    // Marketing
    /// A deal/coupon was generated.
    DealGenerated {
        deal_id: i64,
        /// Why the deal was created (post-session, referral, campaign, etc).
        reason: String,
    },

    /// A deal/coupon was redeemed by a customer.
    DealRedeemed {
        deal_id: i64,
        driver_id: i64,
        discount_credits: i64,
    },

    /// A review nudge was scheduled for a customer.
    ReviewNudgeScheduled {
        driver_id: i64,
        session_id: i64,
        scheduled_at: DateTime<Utc>,
    },

    /// A referral code was redeemed.
    ReferralRedeemed {
        referrer_id: i64,
        referee_id: i64,
        referrer_credits: i64,
        referee_credits: i64,
    },

    // Staff
    /// A staff member clocked in.
    StaffClockIn {
        staff_id: String,
        timestamp: DateTime<Utc>,
    },

    /// A staff member clocked out.
    StaffClockOut {
        staff_id: String,
        timestamp: DateTime<Utc>,
    },

    // Pods
    /// A pod came online (agent connected to server).
    PodOnline {
        pod_id: u8,
    },

    /// A pod went offline.
    PodOffline {
        pod_id: u8,
        reason: String,
    },

    /// A pod's health score changed.
    PodHealthChanged {
        pod_id: u8,
        /// Composite health score 0-100 (from fleet intelligence).
        score: u8,
        previous_score: Option<u8>,
    },

    /// Content drift was detected on a pod.
    PodContentDrift {
        pod_id: u8,
        game_key: String,
        delta_type: String,
        item: String,
    },

    // Infrastructure
    /// Server started or restarted.
    ServerStarted {
        build_id: String,
        version: String,
    },

    /// Cloud sync completed a cycle.
    CloudSyncCompleted {
        direction: SyncDirection,
        tables_synced: u32,
        rows_transferred: u32,
    },

    // Customer
    /// A new customer registered.
    CustomerRegistered {
        driver_id: i64,
        name: String,
        phone: String,
    },

    /// A customer checked in at a pod (scanned QR, entered PIN, etc).
    CustomerCheckedIn {
        driver_id: i64,
        pod_id: u8,
    },

    /// A session report is ready for sharing.
    SessionReportReady {
        session_id: i64,
        driver_id: i64,
    },
}

/// How a wallet was topped up.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CreditType {
    Cash,
    Card,
    Upi,
    Referral,
    Refund,
    Promotional,
}

/// Why credits were debited.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DebitReason {
    Racing,
    Cafe,
    Merchandise,
    Membership,
    Package,
}

/// An item in a cafe order.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CafeOrderItem {
    pub item_name: String,
    pub quantity: u32,
    pub unit_price_credits: i64,
}

/// Direction of cloud sync.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncDirection {
    /// Cloud is authoritative (drivers, pricing).
    CloudToVenue,
    /// Venue is authoritative (billing, laps, game state).
    VenueToCloud,
    /// Bidirectional merge.
    Bidirectional,
}

impl From<crate::launch_contract::GameEvent> for Event {
    fn from(ge: crate::launch_contract::GameEvent) -> Self {
        use crate::launch_contract::GameEvent as GE;
        match ge {
            GE::Started { pod_id, game, pid: _ } => {
                // driver_id not available in GameEvent — caller must enrich.
                Event::GameStarted { pod_id, game, driver_id: 0 }
            }
            GE::Crashed { pod_id, game, reason } => Event::GameCrashed { pod_id, game, reason },
            GE::Ended { pod_id, game, duration_secs } => Event::GameEnded { pod_id, game, duration_secs },
        }
    }
}

impl From<crate::launch_contract::LapCompleted> for Event {
    fn from(lc: crate::launch_contract::LapCompleted) -> Self {
        Event::LapCompleted {
            driver_id: lc.driver_id,
            pod_id: lc.pod_id,
            game: lc.game,
            track: lc.track,
            car: lc.car,
            lap_time_ms: lc.lap_time_ms,
            lap_number: lc.lap_number,
            is_valid: lc.is_valid,
            sector_times_ms: lc.sector_times_ms,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn domain_event_new_has_unique_id() {
        let e1 = DomainEvent::new(Department::Wallet, Event::WalletTopUp {
            driver_id: 1, amount: 700, credit_type: CreditType::Cash,
        });
        let e2 = DomainEvent::new(Department::Wallet, Event::WalletTopUp {
            driver_id: 1, amount: 700, credit_type: CreditType::Cash,
        });
        assert_ne!(e1.id, e2.id);
    }

    #[test]
    fn domain_event_with_correlation() {
        let cid = Uuid::new_v4();
        let e = DomainEvent::with_correlation(
            Department::Billing,
            Event::BillingStarted { session_id: 42, pod_id: 3, driver_id: 1, credits: 700 },
            cid,
        );
        assert_eq!(e.correlation_id, Some(cid));
        assert_eq!(e.department, Department::Billing);
    }

    #[test]
    fn event_serializes_as_tagged_json() {
        let event = Event::PodOnline { pod_id: 5 };
        let json = serde_json::to_string(&event).expect("serialize");
        assert!(json.contains("\"type\":\"pod_online\""));
        assert!(json.contains("\"pod_id\":5"));
    }

    #[test]
    fn domain_event_roundtrip_serde() {
        let original = DomainEvent::new(Department::Racing, Event::PersonalBest {
            driver_id: 42, track: "imola".into(), car: "f40".into(),
            lap_time_ms: 98765, previous_best_ms: Some(99000),
        });
        let json = serde_json::to_string(&original).expect("serialize");
        let restored: DomainEvent = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(original.id, restored.id);
        assert_eq!(original.department, restored.department);
    }

    #[test]
    fn game_event_converts_to_domain_event() {
        let ge = crate::launch_contract::GameEvent::Crashed {
            pod_id: 2, game: GameType::AssettCorsa, reason: "out of memory".into(),
        };
        let event: Event = ge.into();
        match event {
            Event::GameCrashed { pod_id, reason, .. } => {
                assert_eq!(pod_id, 2);
                assert_eq!(reason, "out of memory");
            }
            _ => panic!("expected GameCrashed"),
        }
    }

    #[test]
    fn lap_completed_converts_to_domain_event() {
        let lc = crate::launch_contract::LapCompleted {
            pod_id: 4, driver_id: 10, game: GameType::F125,
            track: "monza".into(), car: "ferrari".into(),
            lap_time_ms: 85432, lap_number: 3, is_valid: true,
            sector_times_ms: vec![28000, 29000, 28432],
        };
        let event: Event = lc.into();
        match event {
            Event::LapCompleted { driver_id, track, is_valid, .. } => {
                assert_eq!(driver_id, 10);
                assert_eq!(track, "monza");
                assert!(is_valid);
            }
            _ => panic!("expected LapCompleted"),
        }
    }

    #[test]
    fn department_display() {
        assert_eq!(Department::GameLaunch.to_string(), "Game Launch");
        assert_eq!(Department::Infrastructure.to_string(), "Infrastructure");
    }
}
