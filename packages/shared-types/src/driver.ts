/** Maps to Rust Driver struct in crates/rc-common/src/types.rs */
export interface Driver {
  id: string;
  name: string;
  email?: string;
  phone?: string;
  steam_guid?: string;
  iracing_id?: string;
  total_laps: number;
  total_time_ms: number;
  created_at?: string;
  /** Computed field returned by the kiosk API — not in Rust struct */
  has_used_trial?: boolean;
}
