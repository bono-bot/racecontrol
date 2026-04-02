import type { DrivingState } from './pod';

/**
 * Maps to Rust BillingSessionStatus enum in crates/rc-common/src/types.rs
 * 10 variants — must stay in sync with Rust source.
 *
 * Drift detection: run `node scripts/check-billing-status-parity.js`
 */
export type BillingSessionStatus =
  | "pending"              // Session created, game not yet launched
  | "waiting_for_game"     // Game launched, waiting for AC STATUS=LIVE before billing starts
  | "active"               // Billing running (game live, customer on track)
  | "paused_manual"        // Staff manually paused the session
  | "paused_disconnect"    // Paused because pod disconnected from server
  | "paused_game_pause"    // Paused because AC STATUS=PAUSE (customer hit ESC)
  | "completed"            // Session ended normally (time elapsed)
  | "ended_early"          // Session ended before allocated time (staff action)
  | "cancelled"            // Session cancelled before starting
  | "cancelled_no_playable" // Session ended before PlayableSignal — customer charged nothing (BILL-06)
  | "paused_crash_recovery"; // Game crashed mid-session, auto-recovery in progress (v31.0)

/** Maps to Rust BillingSessionInfo in crates/rc-common/src/types.rs */
export interface BillingSession {
  id: string;
  driver_id: string;
  driver_name: string;
  pod_id: string;
  pricing_tier_name: string;
  allocated_seconds: number;
  driving_seconds: number;
  remaining_seconds: number;
  status: BillingSessionStatus;
  driving_state: DrivingState;
  started_at?: string;
  split_count: number;
  split_duration_minutes?: number;
  current_split_number: number;
  elapsed_seconds?: number;
  cost_paise?: number;
  rate_per_min_paise?: number;
}

export interface PricingTier {
  id: string;
  name: string;
  duration_minutes: number;
  price_paise: number;
  is_trial: boolean;
  is_active: boolean;
  /** Display ordering — returned by kiosk API, not in core Rust struct */
  sort_order?: number;
}
