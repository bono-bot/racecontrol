/*
 * /admin/incidents — Incident pane.
 *
 * Server Component renders the incident table from listIncidents().
 * Phase B-1 dev-stub: filters via URL search params; mock data.
 * Real data wire-in lands in B-3.
 */

import { listIncidents } from "../../../lib/audit/queries";

export const dynamic = "force-dynamic";

interface PageProps {
  searchParams: Promise<{ status?: string; class?: string; limit?: string }>;
}

export default async function IncidentsPage({ searchParams }: PageProps) {
  const sp = await searchParams;
  const status = (sp.status as "active" | "resolved" | "all" | undefined) ?? "active";
  const classPattern = sp.class;
  const limit = sp.limit ? Math.min(parseInt(sp.limit, 10), 1000) : 100;

  const r = await listIncidents({ status, classPattern, limit });

  return (
    <div>
      <div style={{ display: "flex", alignItems: "baseline", gap: 16, marginBottom: 16 }}>
        <h1 style={{ fontFamily: "Orbitron, sans-serif", fontSize: 24, color: "#E10600", margin: 0 }}>
          Incidents
        </h1>
        <span style={{ fontSize: 12, color: "#5A5A5A" }}>
          Active: {r.active_count} · Resolved 24h: {r.resolved_24h}
        </span>
      </div>

      <nav style={{ display: "flex", gap: 12, marginBottom: 16, fontSize: 13 }}>
        <a
          href="/admin/incidents?status=active"
          style={{
            color: status === "active" ? "#E10600" : "#E5E5E5",
            textDecoration: "none",
            padding: "4px 8px",
            border: `1px solid ${status === "active" ? "#E10600" : "#333333"}`,
            borderRadius: 4,
          }}
        >
          Active
        </a>
        <a
          href="/admin/incidents?status=resolved"
          style={{
            color: status === "resolved" ? "#E10600" : "#E5E5E5",
            textDecoration: "none",
            padding: "4px 8px",
            border: `1px solid ${status === "resolved" ? "#E10600" : "#333333"}`,
            borderRadius: 4,
          }}
        >
          Resolved
        </a>
        <a
          href="/admin/incidents?status=all"
          style={{
            color: status === "all" ? "#E10600" : "#E5E5E5",
            textDecoration: "none",
            padding: "4px 8px",
            border: `1px solid ${status === "all" ? "#E10600" : "#333333"}`,
            borderRadius: 4,
          }}
        >
          All
        </a>
      </nav>

      <table style={{ width: "100%", borderCollapse: "collapse", fontSize: 13, background: "#222222", border: "1px solid #333333" }}>
        <thead>
          <tr style={{ background: "#2A2A2A", textAlign: "left" }}>
            <th style={{ padding: "8px 12px", borderBottom: "1px solid #333333" }}>Class</th>
            <th style={{ padding: "8px 12px", borderBottom: "1px solid #333333" }}>Outcome</th>
            <th style={{ padding: "8px 12px", borderBottom: "1px solid #333333" }}>Time</th>
            <th style={{ padding: "8px 12px", borderBottom: "1px solid #333333" }}>Target</th>
            <th style={{ padding: "8px 12px", borderBottom: "1px solid #333333" }}>Detail</th>
          </tr>
        </thead>
        <tbody>
          {r.incidents.length === 0 ? (
            <tr>
              <td colSpan={5} style={{ padding: "24px", textAlign: "center", color: "#5A5A5A" }}>
                No incidents matching filter.
              </td>
            </tr>
          ) : (
            r.incidents.map((inc) => (
              <tr key={inc.id} style={{ borderBottom: "1px solid #333333" }}>
                <td style={{ padding: "8px 12px", fontFamily: "Orbitron, monospace", fontSize: 12 }}>
                  {inc.action.replace(/^incident_/, "")}
                </td>
                <td style={{ padding: "8px 12px" }}>
                  <span
                    style={{
                      padding: "2px 6px",
                      borderRadius: 3,
                      fontSize: 11,
                      background:
                        inc.outcome === "failure"
                          ? "#E10600"
                          : inc.outcome === "throttled"
                          ? "#FF4400"
                          : inc.outcome === "success"
                          ? "#1A8A00"
                          : "#5A5A5A",
                      color: "white",
                    }}
                  >
                    {inc.outcome}
                  </span>
                </td>
                <td style={{ padding: "8px 12px", color: "#5A5A5A", fontSize: 12 }}>{inc.ts}</td>
                <td style={{ padding: "8px 12px", fontSize: 12 }}>
                  {inc.target_type}:{inc.target_id}
                </td>
                <td style={{ padding: "8px 12px", fontSize: 12, color: "#5A5A5A", maxWidth: 320, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                  {inc.detail ? JSON.stringify(inc.detail) : "—"}
                </td>
              </tr>
            ))
          )}
        </tbody>
      </table>

      <p style={{ marginTop: 24, fontSize: 12, color: "#5A5A5A" }}>
        Phase B-1 dev-stub · mock data via lib/audit/queries.ts · real Postgres read wire-in lands in B-3.
        Per §S-404 Axis .3, this surface is READ-ONLY; acknowledgment writes route through james-canonical.
      </p>
    </div>
  );
}
