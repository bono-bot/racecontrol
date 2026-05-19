import { describe, it, expect } from "vitest";
import { listIncidents, getIncidentById } from "./queries";

describe("audit/queries — listIncidents", () => {
  it("returns active incidents by default", async () => {
    const r = await listIncidents();
    expect(r.incidents.length).toBeGreaterThan(0);
    for (const inc of r.incidents) {
      expect(inc.action.startsWith("incident_")).toBe(true);
      expect(inc.operation_id).not.toBe("incident_resolve");
    }
  });

  it("returns resolved incidents when status=resolved", async () => {
    const r = await listIncidents({ status: "resolved" });
    for (const inc of r.incidents) {
      expect(inc.operation_id).toBe("incident_resolve");
      expect(inc.outcome).toBe("success");
    }
  });

  it("returns all incidents when status=all", async () => {
    const active = await listIncidents({ status: "active" });
    const resolved = await listIncidents({ status: "resolved" });
    const all = await listIncidents({ status: "all" });
    expect(all.incidents.length).toBe(active.incidents.length + resolved.incidents.length);
  });

  it("filters by classPattern substring", async () => {
    const r = await listIncidents({ status: "all", classPattern: "wallet" });
    for (const inc of r.incidents) {
      expect(inc.action.includes("wallet")).toBe(true);
    }
  });

  it("respects limit parameter", async () => {
    const r = await listIncidents({ status: "all", limit: 1 });
    expect(r.incidents.length).toBeLessThanOrEqual(1);
  });

  it("caps limit at 1000", async () => {
    const r = await listIncidents({ status: "all", limit: 10_000 });
    expect(r.incidents.length).toBeLessThanOrEqual(1000);
  });

  it("filters by since timestamp", async () => {
    const future = new Date(Date.now() + 24 * 60 * 60 * 1000).toISOString();
    const r = await listIncidents({ status: "all", since: future });
    expect(r.incidents.length).toBe(0);
  });

  it("returns counts independent of pagination slice", async () => {
    const limited = await listIncidents({ status: "active", limit: 1 });
    const full = await listIncidents({ status: "active" });
    expect(limited.active_count).toBe(full.active_count);
  });

  it("sorts by ts DESC (newest first)", async () => {
    const r = await listIncidents({ status: "all" });
    for (let i = 1; i < r.incidents.length; i++) {
      expect(r.incidents[i - 1].ts >= r.incidents[i].ts).toBe(true);
    }
  });
});

describe("audit/queries — getIncidentById", () => {
  it("returns row for known id", async () => {
    const r = await getIncidentById("1001");
    expect(r).not.toBeNull();
    expect(r?.action).toBe("incident_bono_egress_down");
  });

  it("returns null for unknown id", async () => {
    const r = await getIncidentById("does-not-exist");
    expect(r).toBeNull();
  });
});

describe("audit/queries — privacy posture (§S-404 + §B-16)", () => {
  it("never returns phone in actor_id or detail", async () => {
    const r = await listIncidents({ status: "all" });
    const phoneRe = /\+?\d{10,15}/;
    for (const inc of r.incidents) {
      expect(phoneRe.test(inc.actor_id)).toBe(false);
      if (inc.detail) {
        const json = JSON.stringify(inc.detail);
        expect(phoneRe.test(json)).toBe(false);
      }
    }
  });
});
