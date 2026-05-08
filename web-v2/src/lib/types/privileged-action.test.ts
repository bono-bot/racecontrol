/**
 * PrivilegedAction TS mirror invariant tests.
 *
 * Mirrors the Rust contract enforced at
 * crates/racecontrol/src/auth/privileged_actions.rs `mod tests`.
 *
 * Drift between this file and the Rust enum tests = cross-layer
 * silent disagreement, which is exactly the invariant the manager-pill
 * UI Q5-A enforcement substrate is supposed to make non-fuzzy. These
 * tests catch drift at TypeScript compile/test time.
 */

import { describe, it, expect } from "vitest";
import {
  type PrivilegedAction,
  type PrivilegedActionCategory,
  PRIVILEGED_ACTIONS,
  ACTION_LABEL,
  CATEGORY_LABEL,
  categoryOf,
  requiresPin,
} from "./privileged-action";

describe("PrivilegedAction TS mirror — §AMEND-1.E parity", () => {
  it("PRIVILEGED_ACTIONS has exactly 12 variants (matches Rust all_returns_twelve_variants_per_amend_1_e)", () => {
    expect(PRIVILEGED_ACTIONS).toHaveLength(12);
  });

  it("every variant requires PIN by construction (matches Rust every_variant_is_privileged_by_construction)", () => {
    for (const action of PRIVILEGED_ACTIONS) {
      expect(requiresPin(action)).toBe(true);
    }
  });

  it("category distribution is 4 wallet / 3 identity / 3 session / 2 manager (matches Rust category_distribution_matches_amend_1_e_spec)", () => {
    const counts: Record<PrivilegedActionCategory, number> = {
      wallet_billing_irreversible: 0,
      identity_record: 0,
      session_terminal: 0,
      manager_escalation: 0,
    };
    for (const action of PRIVILEGED_ACTIONS) {
      counts[categoryOf(action)]++;
    }
    expect(counts.wallet_billing_irreversible).toBe(4);
    expect(counts.identity_record).toBe(3);
    expect(counts.session_terminal).toBe(3);
    expect(counts.manager_escalation).toBe(2);
  });

  it("every action identifier is snake_case (matches Rust as_str_matches_serde_snake_case_rendering)", () => {
    const snakeCase = /^[a-z]+(_[a-z]+)*$/;
    for (const action of PRIVILEGED_ACTIONS) {
      expect(action).toMatch(snakeCase);
    }
  });

  it("audit identifiers are globally unique (matches Rust as_str_values_are_globally_unique — NF-bono-1)", () => {
    const set = new Set(PRIVILEGED_ACTIONS);
    expect(set.size).toBe(PRIVILEGED_ACTIONS.length);
  });

  it("approve_refund_over_threshold is manager_escalation NOT wallet (matches Rust approve_refund_over_threshold_is_manager_escalation_not_wallet)", () => {
    expect(categoryOf("approve_refund_over_threshold")).toBe("manager_escalation");
  });

  it("enter_manager_mode is privileged (matches Rust enter_manager_mode_is_distinct_from_individual_privileged_actions)", () => {
    expect(requiresPin("enter_manager_mode")).toBe(true);
    expect(categoryOf("enter_manager_mode")).toBe("manager_escalation");
  });

  it("ACTION_LABEL has an entry for every PrivilegedAction (UI render-completeness invariant)", () => {
    for (const action of PRIVILEGED_ACTIONS) {
      expect(ACTION_LABEL[action]).toBeTruthy();
      expect(typeof ACTION_LABEL[action]).toBe("string");
    }
  });

  it("CATEGORY_LABEL has an entry for every PrivilegedActionCategory (UI render-completeness invariant)", () => {
    const categories: PrivilegedActionCategory[] = [
      "wallet_billing_irreversible",
      "identity_record",
      "session_terminal",
      "manager_escalation",
    ];
    for (const category of categories) {
      expect(CATEGORY_LABEL[category]).toBeTruthy();
      expect(typeof CATEGORY_LABEL[category]).toBe("string");
    }
  });

  it("wallet_billing_irreversible category contains the 4 expected variants", () => {
    const wallet: PrivilegedAction[] = PRIVILEGED_ACTIONS.filter(
      (a) => categoryOf(a) === "wallet_billing_irreversible",
    );
    expect(wallet).toEqual(
      expect.arrayContaining([
        "apply_apology_credit",
        "issue_refund",
        "adjust_wallet_balance",
        "override_discount_ineligible",
      ]),
    );
    expect(wallet).toHaveLength(4);
  });

  it("identity_record category contains the 3 expected variants", () => {
    const identity = PRIVILEGED_ACTIONS.filter(
      (a) => categoryOf(a) === "identity_record",
    );
    expect(identity).toEqual(
      expect.arrayContaining([
        "merge_customer_profiles",
        "promote_walk_in_to_registered",
        "override_identity_binding",
      ]),
    );
    expect(identity).toHaveLength(3);
  });

  it("session_terminal category contains the 3 expected variants", () => {
    const session = PRIVILEGED_ACTIONS.filter(
      (a) => categoryOf(a) === "session_terminal",
    );
    expect(session).toEqual(
      expect.arrayContaining(["end_session_early", "mark_paid_out_of_band", "comp_session"]),
    );
    expect(session).toHaveLength(3);
  });

  it("manager_escalation category contains the 2 expected variants", () => {
    const manager = PRIVILEGED_ACTIONS.filter(
      (a) => categoryOf(a) === "manager_escalation",
    );
    expect(manager).toEqual(
      expect.arrayContaining(["enter_manager_mode", "approve_refund_over_threshold"]),
    );
    expect(manager).toHaveLength(2);
  });
});
