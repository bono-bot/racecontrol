/*
 * /api/v2/echo — CD-6 proxy-to-proxy round-trip wire-test endpoint.
 *
 * Public unauthenticated POST that echoes the request body back with
 * bono-side metadata (pid, uptime, timestamp, basePath). Enables james
 * Admin proxy to verify the cross-PC bridge by:
 *
 *   POST https://admin.racingpoint.cloud/api/v2/echo
 *   Content-Type: application/json
 *   Body: any JSON object
 *
 *   → 200 OK
 *   → { echoed: <body>, bono_received_at: ISO, bono_pid: N, bono_uptime_s: N, basePath: "/v2" }
 *
 * Non-JSON or malformed body returns 400 with {"error":"invalid_payload"}.
 *
 * This is a wire-test channel, NOT a production data path. CD-6 ratified
 * routes (wallet · registration · pricing) will replace it with real
 * handlers once Phase B mirror lands.
 */

import { NextRequest, NextResponse } from "next/server";

export const dynamic = "force-dynamic";

export async function POST(req: NextRequest): Promise<NextResponse> {
  let body: unknown;
  try {
    body = await req.json();
  } catch {
    return NextResponse.json(
      { error: "invalid_payload", message: "Body must be valid JSON." },
      { status: 400 },
    );
  }

  return NextResponse.json(
    {
      echoed: body,
      bono_received_at: new Date().toISOString(),
      bono_pid: process.pid,
      bono_uptime_s: Math.floor(process.uptime()),
      basePath: "/v2",
    },
    { status: 200 },
  );
}
