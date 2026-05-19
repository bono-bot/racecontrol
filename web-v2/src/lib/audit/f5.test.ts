/**
 * audit/f5.ts — F5 audit writer contract tests.
 *
 * R-41 SEAM #2 dev stub. Production wire-in is Postgres `audit_log` append-only table.
 *
 * Coverage:
 *  - hashBody determinism (same input → same hash)
 *  - hashBody content sensitivity (different input → different hash)
 *  - hashBody accepts string OR object input
 *  - writeAudit returns undefined and does not throw on valid F5AuditRow
 *  - writeAuditOrFail wraps internal failure into mutation-abort error
 *  - Contract surface matches R-42 §B-16 privacy posture (body_hash for customer payloads)
 */

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { hashBody, writeAudit, writeAuditOrFail, type F5AuditRow } from "./f5";

function validRow(overrides: Partial<F5AuditRow> = {}): F5AuditRow {
  return {
    ts: new Date().toISOString(),
    actor_id: "staff-001",
    actor_tier: "staff",
    action: "wallet.topup",
    target_type: "household",
    target_id: "hh-abc",
    outcome: "success",
    request_id: "req-xyz",
    ...overrides,
  };
}

describe("hashBody · determinism", () => {
  it("returns identical sha256 hex for the same string input", () => {
    const a = hashBody("payload-1");
    const b = hashBody("payload-1");
    expect(a).toBe(b);
    expect(a).toMatch(/^[a-f0-9]{64}$/);
  });

  it("returns identical hash for the same object input (stable JSON.stringify)", () => {
    const obj = { amount: 100, currency: "INR", customer: "hh-1" };
    const a = hashBody(obj);
    const b = hashBody({ amount: 100, currency: "INR", customer: "hh-1" });
    expect(a).toBe(b);
  });

  it("differs when content differs", () => {
    expect(hashBody("a")).not.toBe(hashBody("b"));
    expect(hashBody({ x: 1 })).not.toBe(hashBody({ x: 2 }));
  });

  it("produces a 64-char lowercase hex string", () => {
    expect(hashBody("test")).toMatch(/^[a-f0-9]{64}$/);
  });
});

describe("writeAudit · contract", () => {
  let logSpy: ReturnType<typeof vi.spyOn>;

  beforeEach(() => {
    logSpy = vi.spyOn(console, "log").mockImplementation(() => {});
  });

  afterEach(() => {
    logSpy.mockRestore();
  });

  it("resolves without throwing on a complete F5AuditRow", async () => {
    await expect(writeAudit(validRow())).resolves.toBeUndefined();
  });

  it("resolves without throwing when optional fields are omitted", async () => {
    const minimal = validRow({ request_id: undefined, operation_id: undefined, body_hash: undefined });
    await expect(writeAudit(minimal)).resolves.toBeUndefined();
  });

  it("accepts all 4 outcome enum values (success / failure / throttled / conflict)", async () => {
    for (const outcome of ["success", "failure", "throttled", "conflict"] as const) {
      await expect(writeAudit(validRow({ outcome }))).resolves.toBeUndefined();
    }
  });

  it("accepts all 4 actor_tier enum values", async () => {
    for (const tier of ["customer", "staff", "shift_lead", "captain"] as const) {
      await expect(writeAudit(validRow({ actor_tier: tier }))).resolves.toBeUndefined();
    }
  });

  it("propagates the row to the dev-stub log channel ([f5 audit] tag)", async () => {
    const row = validRow({ action: "test.dev.stub" });
    await writeAudit(row);
    expect(logSpy).toHaveBeenCalledWith("[f5 audit]", expect.stringContaining("test.dev.stub"));
  });
});

describe("writeAuditOrFail · mutation-abort wrapping", () => {
  let logSpy: ReturnType<typeof vi.spyOn>;

  beforeEach(() => {
    logSpy = vi.spyOn(console, "log").mockImplementation(() => {});
  });

  afterEach(() => {
    logSpy.mockRestore();
  });

  it("resolves silently when underlying writeAudit succeeds", async () => {
    await expect(writeAuditOrFail(validRow())).resolves.toBeUndefined();
  });

  it("wraps any internal failure into a 'mutation must abort' Error", async () => {
    logSpy.mockImplementation(() => {
      throw new Error("simulated audit channel failure");
    });
    await expect(writeAuditOrFail(validRow())).rejects.toThrow(/F5 audit write failed/);
    await expect(writeAuditOrFail(validRow())).rejects.toThrow(/mutation must abort/);
  });

  it("includes the underlying error message in the wrapped failure", async () => {
    logSpy.mockImplementation(() => {
      throw new Error("specific channel diagnostic");
    });
    await expect(writeAuditOrFail(validRow())).rejects.toThrow(/specific channel diagnostic/);
  });
});

describe("F5AuditRow · §B-16 privacy posture surface", () => {
  it("permits body_hash on customer-tier rows (no raw body)", async () => {
    const row = validRow({
      actor_tier: "customer",
      body_hash: hashBody({ phone: "+919999999999", amount: 100 }),
    });
    await expect(writeAudit(row)).resolves.toBeUndefined();
  });

  it("permits full detail on staff-tier rows (staff actions auditable in full)", async () => {
    const row = validRow({
      actor_tier: "staff",
      detail: { reason: "apology_grant", credits: 500, override_reason: "system_outage" },
    });
    await expect(writeAudit(row)).resolves.toBeUndefined();
  });
});
