import { NextResponse } from "next/server";
import { readFileSync } from "node:fs";
import { join } from "node:path";

/**
 * web-v2 health probe — `GET /v2/api/v1/health`
 *
 * Phase 0.1 substrate — minimal probe shape. The activation-phase
 * shape decision (V1-compatible vs V2-native) is bono-AMPLIFIER ask
 * Q4 in PACT-20260503-001 and may be refined post-AMPLIFIER.
 *
 * V2-namespaced URL (/v2/api/v1/health) avoids collision with V1
 * web app health probe at /api/v1/health on :3200.
 *
 * §S-209 NEW-1 close: build_id field reads .next/BUILD_ID at module-load
 * (cached for the life of the Node process) so deploy-parity checks can
 * distinguish stale-vs-fresh pm2 restarts. Sibling of SWAPLOG rule.
 */

export const dynamic = "force-dynamic";
export const revalidate = 0;

const BUILD_ID: string = (() => {
  try {
    return readFileSync(join(process.cwd(), ".next", "BUILD_ID"), "utf-8").trim();
  } catch {
    return "unknown";
  }
})();

type HealthBody = {
  status: "ok";
  service: "web-v2";
  version: string;
  pact: "PACT-20260503-001";
  phase: "0.1-substrate";
  build_id: string;
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
      build_id: BUILD_ID,
      timestamp_iso: new Date().toISOString(),
    },
    { status: 200 },
  );
}
