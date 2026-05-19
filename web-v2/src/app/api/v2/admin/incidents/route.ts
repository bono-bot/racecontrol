/*
 * /api/v2/admin/incidents — Captain console incident list endpoint.
 *
 * GET handler returns F5 audit_log rows where action LIKE 'incident_%'.
 *
 * Phase B-1 dev-stub (this impl):
 *   - No auth enforcement (dev mode — requireF6/requireTier dev-stubs)
 *   - Reads from listIncidents mock data
 *
 * Phase B-4 wire-in (per ARCH-FOLLOWUP-1 scope-map):
 *   - Read Authorization: Bearer JWT
 *   - Call requireF6(token) to verify ES256+JWKS
 *   - Call requireTier(token, "captain") to enforce captain-tier
 *   - On failure: 401 / 403 via standard error envelope
 *
 * Per §S-404 Axis .3 Bono admin proxy trust model:
 *   - Bono edge-validates F6 JWT + tier
 *   - This endpoint is READ-ONLY; no writes ever leave bono surface
 *   - James re-validates on any downstream canonical-state call (none here — pure read)
 *
 * Per CONSTITUTION §5: bono-owned Admin scope.
 */

import { NextRequest, NextResponse } from "next/server";
import { listIncidents, type ListIncidentsParams } from "../../../../../lib/audit/queries";

export const dynamic = "force-dynamic";

export async function GET(req: NextRequest): Promise<NextResponse> {
  const sp = req.nextUrl.searchParams;

  const statusParam = sp.get("status");
  const validStatuses = ["active", "resolved", "all"] as const;
  const status: ListIncidentsParams["status"] =
    statusParam && (validStatuses as readonly string[]).includes(statusParam)
      ? (statusParam as ListIncidentsParams["status"])
      : "active";

  const since = sp.get("since") ?? undefined;
  const classPattern = sp.get("class") ?? undefined;
  const limitRaw = sp.get("limit");
  const limit = limitRaw ? Math.min(Math.max(parseInt(limitRaw, 10) || 100, 1), 1000) : 100;

  try {
    const r = await listIncidents({ status, since, classPattern, limit });
    return NextResponse.json(r, { status: 200 });
  } catch (e) {
    return NextResponse.json(
      {
        error: "internal_error",
        message: "Failed to list incidents.",
        staff_detail: e instanceof Error ? e.message : String(e),
      },
      { status: 500 },
    );
  }
}
