/*
 * /api/v2/health — Bono Admin proxy V2 liveness probe.
 *
 * Public unauthenticated endpoint that proves the bono V2 substrate is
 * receiving and serving requests via the nginx → web-v2 chain. CD-6
 * proxy-to-proxy wire-tests use this as the deterministic ping target.
 *
 * Behavior:
 *   200 OK with JSON body containing:
 *     status: "ok"
 *     service: "bono-admin-proxy-v2"
 *     ts: ISO timestamp at request time
 *     basePath: "/v2" (informational — proves nginx URI rewrite is in chain)
 *
 * No auth required (this is the health probe used to verify the proxy
 * infrastructure itself is alive, BEFORE any auth flow has been wired).
 */

import { NextResponse } from "next/server";

export const dynamic = "force-dynamic";

export async function GET(): Promise<NextResponse> {
  return NextResponse.json(
    {
      status: "ok",
      service: "bono-admin-proxy-v2",
      ts: new Date().toISOString(),
      basePath: "/v2",
    },
    { status: 200 },
  );
}
