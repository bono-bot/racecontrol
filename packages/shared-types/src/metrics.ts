/**
 * Metrics response types — maps to Rust structs in crates/racecontrol/src/api/metrics.rs
 * Used by admin dashboard and downstream consumers.
 */

/** A failure mode with its occurrence count — used in LaunchStatsResponse */
export interface FailureMode {
  mode: string;
  count: number;
}

/** Response for GET /api/v1/metrics/launch-stats */
export interface LaunchStatsResponse {
  success_rate: number;
  avg_time_to_track_ms: number | null;
  p95_time_to_track_ms: number | null;
  total_launches: number;
  common_failure_modes: FailureMode[];
  last_30d_trend: string;
}

/** Response for GET /api/v1/metrics/billing-accuracy */
export interface BillingAccuracyResponse {
  avg_delta_ms: number | null;
  max_delta_ms: number | null;
  sessions_with_zero_delta: number;
  sessions_where_billing_never_started: number;
  false_playable_signals: number;
}

/** A car+track combination with its launch success statistics — used in /games/alternatives */
export interface AlternativeCombo {
  car: string | null;
  track: string | null;
  success_rate: number;
  avg_time_ms: number | null;
  total_launches: number;
}

/** Per-pod launch matrix row — used in GET /api/v1/admin/launch-matrix */
export interface LaunchMatrixRow {
  pod_id: string;
  total_launches: number;
  success_rate: number;
  avg_time_ms: number | null;
  top_3_failure_modes: FailureMode[];
  flagged: boolean;
}
