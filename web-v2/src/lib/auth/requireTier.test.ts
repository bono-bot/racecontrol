/**
 * requireTier.ts — tier ordering + self-attestation invariant tests.
 *
 * R-41 invariant #2: customer < staff < shift_lead < captain.
 */

import { describe, it, expect } from "vitest";
import type { F6Token, Tier } from "./requireF6";
import { tierRank, requireTier, requireTierWithSelfAttestation } from "./requireTier";
import { HandlerError } from "../errors";

function makeToken(tier: Tier, subOverride?: string): F6Token {
  return {
    sub: subOverride ?? "test-sub",
    iss: "bono",
    aud: "v2.racingpoint.cloud",
    exp: Math.floor(Date.now() / 1000) + 3600,
    iat: Math.floor(Date.now() / 1000),
    tier,
  };
}

describe("requireTier · ordering", () => {
  it("ranks customer < staff < shift_lead < captain", () => {
    expect(tierRank("customer")).toBe(0);
    expect(tierRank("staff")).toBe(1);
    expect(tierRank("shift_lead")).toBe(2);
    expect(tierRank("captain")).toBe(3);
  });
});

describe("requireTier · authorization", () => {
  it("allows when actor tier == required tier", () => {
    expect(() => requireTier(makeToken("staff"), "staff")).not.toThrow();
  });

  it("allows when actor tier > required tier", () => {
    expect(() => requireTier(makeToken("captain"), "staff")).not.toThrow();
    expect(() => requireTier(makeToken("shift_lead"), "staff")).not.toThrow();
  });

  it("throws HandlerError(403) when actor tier < required tier", () => {
    try {
      requireTier(makeToken("customer"), "staff");
      throw new Error("expected throw");
    } catch (e) {
      expect(e).toBeInstanceOf(HandlerError);
      expect((e as HandlerError).status).toBe(403);
      expect((e as HandlerError).body.error).toBe("insufficient_tier");
      expect((e as HandlerError).body.staff_detail).toContain("required=staff");
      expect((e as HandlerError).body.staff_detail).toContain("actor=customer");
    }
  });
});

describe("requireTier · self-attestation", () => {
  it("permits one-tier-down when actor's subject matches target's subject", () => {
    // staff acting on their OWN scope can do shift_lead-level if self-attested
    const token = makeToken("staff", "self-actor-1");
    expect(() =>
      requireTierWithSelfAttestation(token, "shift_lead", {
        actorSubject: "self-actor-1",
        targetSubject: "self-actor-1",
      })
    ).not.toThrow();
  });

  it("does NOT permit one-tier-down when actor differs from target", () => {
    const token = makeToken("staff", "actor-A");
    expect(() =>
      requireTierWithSelfAttestation(token, "shift_lead", {
        actorSubject: "actor-A",
        targetSubject: "target-B",
      })
    ).toThrow(HandlerError);
  });

  it("falls through to requireTier when no self-attestation context is provided", () => {
    const token = makeToken("customer");
    expect(() => requireTierWithSelfAttestation(token, "staff")).toThrow(HandlerError);
  });

  it("does NOT permit two-tier-down even with self-attestation", () => {
    const token = makeToken("customer", "self");
    // customer self-attesting at shift_lead (skip staff) — not permitted
    expect(() =>
      requireTierWithSelfAttestation(token, "shift_lead", {
        actorSubject: "self",
        targetSubject: "self",
      })
    ).toThrow(HandlerError);
  });
});
