/*
 * /admin home — Captain console landing.
 *
 * Phase B-1 dev-stub: incident summary card + nav links.
 * Real data wire-in lands in B-3 (replace listIncidents mock returns with pg query).
 */

import { listIncidents } from "../../lib/audit/queries";

export const dynamic = "force-dynamic";

export default async function AdminHome() {
  const r = await listIncidents({ status: "active", limit: 5 });

  return (
    <div>
      <h1 style={{ fontFamily: "Orbitron, sans-serif", fontSize: 24, color: "#E10600", marginBottom: 16 }}>
        Status
      </h1>

      <div
        style={{
          background: "#2A2A2A",
          border: "1px solid #333333",
          borderRadius: 8,
          padding: 16,
          marginBottom: 24,
          maxWidth: 480,
        }}
      >
        <div style={{ fontSize: 12, color: "#5A5A5A", marginBottom: 8 }}>Incidents</div>
        <div style={{ display: "flex", gap: 24, alignItems: "baseline" }}>
          <div>
            <div style={{ fontSize: 32, fontWeight: 700, color: "#E10600" }}>{r.active_count}</div>
            <div style={{ fontSize: 12, color: "#5A5A5A" }}>Active</div>
          </div>
          <div>
            <div style={{ fontSize: 32, fontWeight: 700, color: "#5A5A5A" }}>{r.resolved_24h}</div>
            <div style={{ fontSize: 12, color: "#5A5A5A" }}>Resolved (24h)</div>
          </div>
        </div>
        <a
          href="/admin/incidents"
          style={{ display: "inline-block", marginTop: 12, fontSize: 14, color: "#E10600", textDecoration: "none" }}
        >
          View all →
        </a>
      </div>

      <p style={{ fontSize: 13, color: "#5A5A5A", maxWidth: 640 }}>
        Captain console READ-ONLY per §S-404 Axis .3 trust model. Resolution writes route through James-canonical
        (CD-7.1). No mutation endpoints exposed in this surface.
      </p>
    </div>
  );
}
