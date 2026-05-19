/*
 * /admin layout — Captain console auth gate shell.
 *
 * Per §S-404 Axis .3 Bono admin proxy trust model:
 *   - Bono edge-validates F6 JWT + tier
 *   - James re-validates on any downstream canonical-state call
 *   - "Bono validation is NEVER sufficient alone"
 *
 * Per CONSTITUTION §5: Captain console is bono-owned Admin surface.
 * Current dev-stub: layout is rendered without F6/tier check (stubs live
 * in lib/auth/requireF6.ts + requireTier.ts and throw-on-prod).
 *
 * B-4 wire-in (per ARCH-FOLLOWUP-1 scope-map):
 *   - Read `Authorization: Bearer <jwt>` from request headers
 *   - Call requireF6() to verify ES256+JWKS
 *   - Call requireTier(token, "captain") to enforce captain-tier
 *   - On failure: redirect to /login (when V1 admin login flow is V2-migrated)
 *
 * Phase B-1 (this implementation):
 *   - Pure UI shell with nav links
 *   - No auth enforcement (dev-stub UI for design review)
 */

import type { Metadata } from "next";

export const metadata: Metadata = {
  title: "Captain Console — Racing Point",
};

export default function AdminLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <div style={{ minHeight: "100vh", background: "#1A1A1A", color: "#E5E5E5", fontFamily: "Montserrat, sans-serif" }}>
      <header
        style={{
          background: "#222222",
          borderBottom: "1px solid #333333",
          padding: "12px 24px",
          display: "flex",
          alignItems: "center",
          gap: 24,
        }}
      >
        <span style={{ fontWeight: 700, fontSize: 18, color: "#E10600", fontFamily: "Orbitron, sans-serif", letterSpacing: 1 }}>
          Captain Console
        </span>
        <nav style={{ display: "flex", gap: 16, fontSize: 14 }}>
          <a href="/admin" style={{ color: "#E5E5E5", textDecoration: "none" }}>
            Home
          </a>
          <a href="/admin/incidents" style={{ color: "#E5E5E5", textDecoration: "none" }}>
            Incidents
          </a>
        </nav>
        <span style={{ marginLeft: "auto", fontSize: 12, color: "#5A5A5A" }}>
          ARCH-FOLLOWUP-1 · Phase B-1 dev-stub
        </span>
      </header>
      <main style={{ padding: "24px" }}>{children}</main>
    </div>
  );
}
