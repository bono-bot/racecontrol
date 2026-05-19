import { HandlerError } from "../errors";

/*
 * BONO-AI proactive seed per R-41 §"The four SEAMs Bono-AI must wire" + R-42 CD-6 doctrine.
 *
 * Production wire-in (R-41 SEAM #1):
 *   - ES256 + JWKS verification via `jose` (issuer-specific JWKS endpoints for iss=bono | iss=james)
 *   - §B-16 assertion: customer tokens MUST NOT carry a `phone` claim (no-phone-as-FK)
 *   - CD-6 chain: F6 JWT forwarded verbatim Bono Admin proxy → James Admin proxy → heart;
 *     both proxies validate independently (defence-in-depth)
 *
 * Dev stub:
 *   - Accepts `Authorization: Bearer <jwt>` with a parseable payload
 *   - Base64-decodes payload (NO signature verify in dev)
 *   - Throws on production via NODE_ENV check at module load so accidental prod-deploy fails fast
 */

if (process.env.NODE_ENV === "production") {
  throw new Error(
    "requireF6 dev-stub reached in production — wire ES256+JWKS via `jose` before deploy"
  );
}

export type Tier = "customer" | "staff" | "shift_lead" | "captain";
export type Issuer = "bono" | "james";

export interface F6Token {
  sub: string;
  iss: Issuer;
  aud: string;
  exp: number;
  iat: number;
  tier: Tier;
  household_id?: string;
  profile_id?: string;
  staff_id?: string;
  pod_id?: string;
}

function decodeJwtPayload(jwt: string): Record<string, unknown> {
  const parts = jwt.split(".");
  if (parts.length !== 3) {
    throw new HandlerError(401, {
      error: "unauthorized",
      message: "Authentication required.",
      staff_detail: "malformed JWT structure",
    });
  }
  try {
    const payloadB64 = parts[1].replace(/-/g, "+").replace(/_/g, "/");
    const padding = "=".repeat((4 - (payloadB64.length % 4)) % 4);
    const json = Buffer.from(payloadB64 + padding, "base64").toString("utf-8");
    return JSON.parse(json) as Record<string, unknown>;
  } catch (e) {
    throw new HandlerError(401, {
      error: "unauthorized",
      message: "Authentication required.",
      staff_detail: `JWT payload decode failed: ${e instanceof Error ? e.message : String(e)}`,
    });
  }
}

function assertNoPhoneClaim(payload: Record<string, unknown>): void {
  if ("phone" in payload || "phone_e164" in payload || "phone_hash" in payload) {
    throw new HandlerError(401, {
      error: "invalid_token",
      message: "Authentication failed.",
      staff_detail: "§B-16 violation — F6 token carries a phone claim (must use household_id)",
    });
  }
}

export async function requireF6(req: Request): Promise<F6Token> {
  const header = req.headers.get("authorization") ?? req.headers.get("Authorization");
  if (!header || !header.startsWith("Bearer ")) {
    throw new HandlerError(401, {
      error: "unauthorized",
      message: "Authentication required.",
      staff_detail: "Authorization header missing or not Bearer scheme",
    });
  }
  const jwt = header.slice("Bearer ".length).trim();
  const payload = decodeJwtPayload(jwt);
  assertNoPhoneClaim(payload);

  const sub = typeof payload.sub === "string" ? payload.sub : null;
  const iss = payload.iss === "bono" || payload.iss === "james" ? payload.iss : null;
  const aud = typeof payload.aud === "string" ? payload.aud : null;
  const exp = typeof payload.exp === "number" ? payload.exp : null;
  const iat = typeof payload.iat === "number" ? payload.iat : null;
  const tier =
    payload.tier === "customer" ||
    payload.tier === "staff" ||
    payload.tier === "shift_lead" ||
    payload.tier === "captain"
      ? payload.tier
      : null;

  if (!sub || !iss || !aud || !exp || !iat || !tier) {
    throw new HandlerError(401, {
      error: "invalid_token",
      message: "Authentication failed.",
      staff_detail: "F6 token missing required claims (sub/iss/aud/exp/iat/tier)",
    });
  }
  if (exp * 1000 < Date.now()) {
    throw new HandlerError(401, {
      error: "token_expired",
      message: "Session expired. Please sign in again.",
    });
  }

  return {
    sub,
    iss,
    aud,
    exp,
    iat,
    tier,
    household_id: typeof payload.household_id === "string" ? payload.household_id : undefined,
    profile_id: typeof payload.profile_id === "string" ? payload.profile_id : undefined,
    staff_id: typeof payload.staff_id === "string" ? payload.staff_id : undefined,
    pod_id: typeof payload.pod_id === "string" ? payload.pod_id : undefined,
  };
}
