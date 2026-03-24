/** Status field in redeem-pin error responses — reliable state indicator */
export type RedeemPinStatus = 'lockout' | 'invalid_pin' | 'pending_debit' | 'error';

/**
 * Response from POST /api/v1/kiosk/redeem-pin
 *
 * Maps to Rust reservation::redeem_pin() OK/Err in crates/racecontrol/src/reservation.rs
 * and the kiosk_redeem_pin() handler in crates/racecontrol/src/api/routes.rs.
 *
 * On success: pod_number, pod_id, driver_name, experience_name, tier_name,
 *             allocated_seconds, billing_session_id are present; error is absent.
 * On error:   error and status are present; remaining_attempts or
 *             lockout_remaining_seconds may be present depending on status.
 */
export interface RedeemPinResponse {
  // ── Success fields ──
  pod_number?: number;
  pod_id?: string;
  driver_name?: string;
  experience_name?: string;
  tier_name?: string;
  allocated_seconds?: number;
  billing_session_id?: string;

  // ── Error fields ──
  error?: string;
  remaining_attempts?: number;
  lockout_remaining_seconds?: number;
  status?: RedeemPinStatus;
}
