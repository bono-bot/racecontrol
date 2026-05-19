/**
 * requireF6.ts — JWT parsing + §B-16 no-phone-claim assertion + claim validation tests.
 *
 * R-41 SEAM #1 dev stub. Production wire-in is ES256+JWKS via `jose` per the throw-on-prod
 * guard at module load.
 *
 * Coverage:
 *  - Authorization header shape (missing / non-Bearer / case-insensitive)
 *  - JWT structure (3 parts / decodable payload)
 *  - §B-16 invariant (phone / phone_e164 / phone_hash claims rejected)
 *  - Required claims (sub / iss / aud / exp / iat / tier)
 *  - Enum validation (iss ∈ {bono, james} · tier ∈ {customer, staff, shift_lead, captain})
 *  - Expiry (exp * 1000 < now → 401 token_expired)
 *  - Optional claim propagation (household_id / profile_id / staff_id / pod_id)
 */

import { describe, it, expect } from "vitest";
import { requireF6 } from "./requireF6";
import { HandlerError } from "../errors";

function encodePayload(payload: Record<string, unknown>): string {
  const header = Buffer.from(JSON.stringify({ alg: "ES256", typ: "JWT" })).toString("base64url");
  const body = Buffer.from(JSON.stringify(payload)).toString("base64url");
  const sig = "DEV_STUB_SIG"; // dev stub does not verify
  return `${header}.${body}.${sig}`;
}

function makeRequest(authValue?: string, headerName: string = "authorization"): Request {
  const headers = new Headers();
  if (authValue !== undefined) headers.set(headerName, authValue);
  return new Request("http://localhost/test", { headers });
}

function validPayload(overrides: Record<string, unknown> = {}): Record<string, unknown> {
  return {
    sub: "household-abc",
    iss: "bono",
    aud: "v2.racingpoint.cloud",
    exp: Math.floor(Date.now() / 1000) + 3600,
    iat: Math.floor(Date.now() / 1000),
    tier: "customer",
    ...overrides,
  };
}

describe("requireF6 · Authorization header shape", () => {
  it("throws 401 unauthorized when header is missing", async () => {
    await expect(requireF6(makeRequest())).rejects.toMatchObject({
      status: 401,
      body: { error: "unauthorized" },
    });
  });

  it("throws 401 unauthorized when header is not Bearer scheme", async () => {
    await expect(requireF6(makeRequest("Basic dXNlcjpwYXNz"))).rejects.toMatchObject({
      status: 401,
      body: { error: "unauthorized" },
    });
  });

  it("accepts capital-A 'Authorization' header (case-insensitive lookup)", async () => {
    const jwt = encodePayload(validPayload());
    const token = await requireF6(makeRequest(`Bearer ${jwt}`, "Authorization"));
    expect(token.sub).toBe("household-abc");
  });
});

describe("requireF6 · JWT structure", () => {
  it("throws 401 unauthorized on malformed JWT (not 3 parts)", async () => {
    await expect(requireF6(makeRequest("Bearer notajwt"))).rejects.toMatchObject({
      status: 401,
      body: { error: "unauthorized", staff_detail: "malformed JWT structure" },
    });
  });

  it("throws 401 unauthorized when payload is not valid base64-JSON", async () => {
    const jwt = "header.@@@notbase64@@@.sig";
    await expect(requireF6(makeRequest(`Bearer ${jwt}`))).rejects.toMatchObject({
      status: 401,
      body: { error: "unauthorized" },
    });
  });
});

describe("requireF6 · §B-16 no-phone-claim invariant", () => {
  it("rejects token carrying `phone` claim with invalid_token", async () => {
    const jwt = encodePayload(validPayload({ phone: "+919999999999" }));
    await expect(requireF6(makeRequest(`Bearer ${jwt}`))).rejects.toMatchObject({
      status: 401,
      body: { error: "invalid_token", staff_detail: expect.stringContaining("§B-16") },
    });
  });

  it("rejects token carrying `phone_e164` claim", async () => {
    const jwt = encodePayload(validPayload({ phone_e164: "+919999999999" }));
    await expect(requireF6(makeRequest(`Bearer ${jwt}`))).rejects.toMatchObject({
      status: 401,
      body: { error: "invalid_token" },
    });
  });

  it("rejects token carrying `phone_hash` claim", async () => {
    const jwt = encodePayload(validPayload({ phone_hash: "sha256:abcd1234" }));
    await expect(requireF6(makeRequest(`Bearer ${jwt}`))).rejects.toMatchObject({
      status: 401,
      body: { error: "invalid_token" },
    });
  });
});

describe("requireF6 · required claims", () => {
  it.each([
    ["sub", validPayload({ sub: undefined })],
    ["iss", validPayload({ iss: undefined })],
    ["aud", validPayload({ aud: undefined })],
    ["exp", validPayload({ exp: undefined })],
    ["iat", validPayload({ iat: undefined })],
    ["tier", validPayload({ tier: undefined })],
  ])("throws 401 invalid_token when %s is missing", async (_field, payload) => {
    const jwt = encodePayload(payload);
    await expect(requireF6(makeRequest(`Bearer ${jwt}`))).rejects.toMatchObject({
      status: 401,
      body: { error: "invalid_token" },
    });
  });

  it("throws 401 invalid_token when iss is neither bono nor james", async () => {
    const jwt = encodePayload(validPayload({ iss: "other" }));
    await expect(requireF6(makeRequest(`Bearer ${jwt}`))).rejects.toMatchObject({
      status: 401,
      body: { error: "invalid_token" },
    });
  });

  it("throws 401 invalid_token when tier is not in enum", async () => {
    const jwt = encodePayload(validPayload({ tier: "admin" }));
    await expect(requireF6(makeRequest(`Bearer ${jwt}`))).rejects.toMatchObject({
      status: 401,
      body: { error: "invalid_token" },
    });
  });
});

describe("requireF6 · expiry", () => {
  it("throws 401 token_expired when exp is in the past", async () => {
    const jwt = encodePayload(validPayload({ exp: Math.floor(Date.now() / 1000) - 10 }));
    await expect(requireF6(makeRequest(`Bearer ${jwt}`))).rejects.toMatchObject({
      status: 401,
      body: { error: "token_expired" },
    });
  });

  it("accepts token with exp in the future", async () => {
    const jwt = encodePayload(validPayload({ exp: Math.floor(Date.now() / 1000) + 60 }));
    const token = await requireF6(makeRequest(`Bearer ${jwt}`));
    expect(token.sub).toBe("household-abc");
  });
});

describe("requireF6 · happy path + optional claim propagation", () => {
  it("returns F6Token with all required claims for bono customer", async () => {
    const jwt = encodePayload(validPayload());
    const token = await requireF6(makeRequest(`Bearer ${jwt}`));
    expect(token).toMatchObject({
      sub: "household-abc",
      iss: "bono",
      aud: "v2.racingpoint.cloud",
      tier: "customer",
    });
    expect(typeof token.exp).toBe("number");
    expect(typeof token.iat).toBe("number");
  });

  it("accepts iss=james tokens", async () => {
    const jwt = encodePayload(validPayload({ iss: "james", tier: "staff" }));
    const token = await requireF6(makeRequest(`Bearer ${jwt}`));
    expect(token.iss).toBe("james");
    expect(token.tier).toBe("staff");
  });

  it("propagates optional claims (household_id / profile_id / staff_id / pod_id)", async () => {
    const jwt = encodePayload(
      validPayload({
        household_id: "hh-123",
        profile_id: "profile-456",
        staff_id: "staff-789",
        pod_id: "pod-2",
      }),
    );
    const token = await requireF6(makeRequest(`Bearer ${jwt}`));
    expect(token.household_id).toBe("hh-123");
    expect(token.profile_id).toBe("profile-456");
    expect(token.staff_id).toBe("staff-789");
    expect(token.pod_id).toBe("pod-2");
  });

  it("leaves optional claims undefined when absent", async () => {
    const jwt = encodePayload(validPayload());
    const token = await requireF6(makeRequest(`Bearer ${jwt}`));
    expect(token.household_id).toBeUndefined();
    expect(token.profile_id).toBeUndefined();
    expect(token.staff_id).toBeUndefined();
    expect(token.pod_id).toBeUndefined();
  });

  it("accepts all 4 tier enum values", async () => {
    for (const tier of ["customer", "staff", "shift_lead", "captain"] as const) {
      const jwt = encodePayload(validPayload({ tier }));
      const token = await requireF6(makeRequest(`Bearer ${jwt}`));
      expect(token.tier).toBe(tier);
    }
  });
});

describe("requireF6 · error type contract", () => {
  it("throws HandlerError instances (not raw Error)", async () => {
    try {
      await requireF6(makeRequest());
      throw new Error("expected throw");
    } catch (e) {
      expect(e).toBeInstanceOf(HandlerError);
      expect((e as HandlerError).status).toBe(401);
      expect((e as HandlerError).body.message).toBeTruthy();
    }
  });
});
