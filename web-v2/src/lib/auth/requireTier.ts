import { HandlerError } from "../errors";
import type { F6Token, Tier } from "./requireF6";

/*
 * BONO-AI proactive seed per R-41 §"The 8 pattern invariants" #2:
 *   "Tier second (`requireTier`) — match `x-rp-required-tier` on YAML op"
 *
 * Tier ordering (R-41 README): customer < staff < shift_lead < captain
 * Self-attestation: when a higher-tier action targets the actor's own scope
 * (e.g., staff topping up THEIR OWN wallet), the YAML op may declare
 * `x-rp-self-attestation: true` and `requireTier` permits one-tier-down callers.
 *
 * Pure logic; no external deps; no env gate (this is decidable at compile time).
 */

const TIER_ORDER: Record<Tier, number> = {
  customer: 0,
  staff: 1,
  shift_lead: 2,
  captain: 3,
};

export function tierRank(t: Tier): number {
  return TIER_ORDER[t];
}

export function requireTier(token: F6Token, required: Tier): void {
  if (tierRank(token.tier) < tierRank(required)) {
    throw new HandlerError(403, {
      error: "insufficient_tier",
      message: "You don't have permission for this action.",
      staff_detail: `required=${required}, actor=${token.tier}`,
    });
  }
}

export interface SelfAttestationContext {
  actorSubject: string;
  targetSubject: string;
}

export function requireTierWithSelfAttestation(
  token: F6Token,
  required: Tier,
  ctx?: SelfAttestationContext
): void {
  if (ctx && ctx.actorSubject === ctx.targetSubject) {
    const oneDown = tierRank(required) - 1;
    if (tierRank(token.tier) >= Math.max(oneDown, 0)) return;
  }
  requireTier(token, required);
}
