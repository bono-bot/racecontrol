import type { DrivingState } from './pod';

export type BillingSessionStatus =
  | "pending"
  | "active"
  | "paused_idle"
  | "paused_manual"
  | "completed"
  | "ended_early"
  | "cancelled"
  | "expired";

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
