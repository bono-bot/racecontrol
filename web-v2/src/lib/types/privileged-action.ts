/**
 * TypeScript mirror of Rust PrivilegedAction enum at
 * crates/racecontrol/src/auth/privileged_actions.rs.
 *
 * PACT-20260506-001 §AMEND-1.E + Phase 1 wire-up Session 5 (manager-pill UI).
 *
 * Wire-format invariants (DO NOT change without coordinating Rust side):
 *   - 12 variants × 4 categories per §AMEND-1.E (Rust test
 *     `all_returns_twelve_variants_per_amend_1_e` enforces 12).
 *   - snake_case identifiers match `serde(rename_all = "snake_case")` +
 *     `PrivilegedAction::as_str` (Rust test
 *     `as_str_matches_serde_snake_case_rendering` enforces parity).
 *   - Category distribution: Wallet 4 / Identity 3 / SessionTerminal 3 /
 *     ManagerEscalation 2 (Rust test
 *     `category_distribution_matches_amend_1_e_spec` enforces).
 *   - `ApproveRefundOverThreshold` is `manager_escalation`, NOT wallet
 *     (Rust test `approve_refund_over_threshold_is_manager_escalation_not_wallet`).
 */

export type PrivilegedActionCategory =
  | "wallet_billing_irreversible"
  | "identity_record"
  | "session_terminal"
  | "manager_escalation";

export type PrivilegedAction =
  // Wallet/billing irreversibles (4)
  | "apply_apology_credit"
  | "issue_refund"
  | "adjust_wallet_balance"
  | "override_discount_ineligible"
  // Identity/customer record changes (3)
  | "merge_customer_profiles"
  | "promote_walk_in_to_registered"
  | "override_identity_binding"
  // Session/billing terminals (3)
  | "end_session_early"
  | "mark_paid_out_of_band"
  | "comp_session"
  // Manager-role escalations (2)
  | "enter_manager_mode"
  | "approve_refund_over_threshold";

/**
 * Full enumeration mirroring Rust `PrivilegedAction::all()`.
 * Order matches §AMEND-1.E spec; categories grouped contiguously.
 */
export const PRIVILEGED_ACTIONS: readonly PrivilegedAction[] = [
  "apply_apology_credit",
  "issue_refund",
  "adjust_wallet_balance",
  "override_discount_ineligible",
  "merge_customer_profiles",
  "promote_walk_in_to_registered",
  "override_identity_binding",
  "end_session_early",
  "mark_paid_out_of_band",
  "comp_session",
  "enter_manager_mode",
  "approve_refund_over_threshold",
] as const;

const CATEGORY_OF: Readonly<Record<PrivilegedAction, PrivilegedActionCategory>> = {
  apply_apology_credit: "wallet_billing_irreversible",
  issue_refund: "wallet_billing_irreversible",
  adjust_wallet_balance: "wallet_billing_irreversible",
  override_discount_ineligible: "wallet_billing_irreversible",
  merge_customer_profiles: "identity_record",
  promote_walk_in_to_registered: "identity_record",
  override_identity_binding: "identity_record",
  end_session_early: "session_terminal",
  mark_paid_out_of_band: "session_terminal",
  comp_session: "session_terminal",
  enter_manager_mode: "manager_escalation",
  approve_refund_over_threshold: "manager_escalation",
};

/**
 * Mirror of Rust `PrivilegedAction::category()`.
 */
export function categoryOf(action: PrivilegedAction): PrivilegedActionCategory {
  return CATEGORY_OF[action];
}

/**
 * Mirror of Rust `PrivilegedAction::requires_pin()` — by construction, every
 * PrivilegedAction value requires PIN re-entry. Provided for handler-side
 * parity with the Rust contract; UI render-condition for manager-pill is
 * `if PRIVILEGED_ACTIONS.includes(action) { manager-pill + PIN-prompt }`.
 */
export function requiresPin(action: PrivilegedAction): boolean {
  return CATEGORY_OF[action] !== undefined;
}

/**
 * Human-readable labels for UI rendering. Rust side has no display labels —
 * audit logs use snake_case `as_str()`. UI strings live here.
 *
 * Per Captain customer-satisfaction-first / minimal-compromise (2026-05-07
 * ~03:05 IST): labels are staff-facing (POS terminal), kept concise + plain
 * English, no jargon.
 */
export const ACTION_LABEL: Readonly<Record<PrivilegedAction, string>> = {
  apply_apology_credit: "Apply apology credit",
  issue_refund: "Issue refund",
  adjust_wallet_balance: "Adjust wallet balance",
  override_discount_ineligible: "Override discount-ineligible flag",
  merge_customer_profiles: "Merge customer profiles",
  promote_walk_in_to_registered: "Promote walk-in to registered",
  override_identity_binding: "Override identity binding",
  end_session_early: "End session early",
  mark_paid_out_of_band: "Mark paid (out of band)",
  comp_session: "Comp session",
  enter_manager_mode: "Enter manager mode",
  approve_refund_over_threshold: "Approve refund over threshold",
};

export const CATEGORY_LABEL: Readonly<Record<PrivilegedActionCategory, string>> = {
  wallet_billing_irreversible: "Wallet & billing",
  identity_record: "Customer record",
  session_terminal: "Session end",
  manager_escalation: "Manager mode",
};
