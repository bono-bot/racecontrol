/**
 * TypeScript types mirroring the Rust cirs_lookup HTTP DTOs at
 * crates/racecontrol/src/api/cirs_lookup.rs.
 *
 * PACT-20260506-001 Phase 1 wire-up Session 4 — POS UI.
 *
 * Wire-format invariants (DO NOT change without coordinating Rust side):
 *   - Discriminated union via `method` field (per §AMEND-1.E + Q1-A)
 *   - `balance_credits` field name (per §AMEND-1.A NF-bono-1 absorption)
 *   - LookupInput shape matches Rust v2_db::cirs::LookupInput re-exported
 *     as cirs_lookup::CirsLookupRequest.
 *
 * Composes-with cirs_lookup.rs serde tests (14/14 PASS commit c0078935).
 */

export type CirsLookupRequest =
  | { method: "phone"; phone: string }
  | { method: "qr_payload"; qr_payload: string }
  | { method: "nfc_tag_id"; nfc_tag_id: string }
  | { method: "walk_in_guest_id"; walk_in_guest_id: 1 | 2 };

export interface ProfileSummary {
  profile_id: string;
  full_name: string;
  is_default: boolean;
  discount_ineligible: boolean;
}

export interface ProfilePreview {
  customer_id: string;
  full_name: string;
  balance_credits: number;
  arrival_count_30d: number;
  profiles: ProfileSummary[];
}

export type CirsLookupResponse =
  | { status: "found"; profile: ProfilePreview }
  | { status: "not_found"; reason: string }
  | { status: "error"; code: string; message: string };

/**
 * Indian-mobile-prefix WARN gate (NF-james-B per §AMEND-1.B).
 * Returns `true` when first digit is in {6, 7, 8, 9} (Indian mobile range).
 * Used as WARN-only at UI input layer; substrate stays permissive for
 * cross-tz / E.164 customers.
 */
export function isIndianMobilePrefix(phone: string): boolean {
  if (phone.length === 0) return true;
  const first = phone[0];
  return first === "6" || first === "7" || first === "8" || first === "9";
}
