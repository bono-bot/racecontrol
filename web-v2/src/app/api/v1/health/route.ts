import { NextResponse } from "next/server";

/**
 * web-v2 health probe — `GET /v2/api/v1/health`
 *
 * Phase 0.1 substrate — minimal probe shape. The activation-phase
 * shape decision (V1-compatible vs V2-native) is bono-AMPLIFIER ask
 * Q4 in PACT-20260503-001 and may be refined post-AMPLIFIER.
 *
 * V2-namespaced URL (/v2/api/v1/health) avoids collision with V1
 * web app health probe at /api/v1/health on :3200.
 */

export const dynamic = "force-dynamic";
export const revalidate = 0;

type HealthBody = {
  status: "ok";
  service: "web-v2";
  version: string;
  pact: "PACT-20260503-001";
  phase: "0.1-substrate";
  timestamp_iso: string;
};

export function GET(): NextResponse<HealthBody> {
  return NextResponse.json(
    {
      status: "ok",
      service: "web-v2",
      version: "0.1.0",
      pact: "PACT-20260503-001",
      phase: "0.1-substrate",
      timestamp_iso: new Date().toISOString(),
    },
    { status: 200 },
  );
}
