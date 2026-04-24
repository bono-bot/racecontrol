import type { NextConfig } from "next";
import path from "path";

// Admin gateway base URL — server-side env, not NEXT_PUBLIC_* (not in browser bundle).
// Used by the /api/customer/:path* rewrite to proxy PWA requests through the admin
// spinal cord instead of directly to RC. Ratified as PACT-20260424-001 Option B
// (gradual proxy, keep fallback). Falls back to admin cloud URL when unset in prod.
//
// - Venue:   ADMIN_GATEWAY_URL=http://192.168.31.23:3201
// - Cloud:   ADMIN_GATEWAY_URL=https://admin.racingpoint.cloud
// - Local dev: defaults to localhost:3201 if admin runs on same host
const ADMIN_GATEWAY_URL =
  process.env.ADMIN_GATEWAY_URL || "http://localhost:3201";

const nextConfig: NextConfig = {
  output: "standalone",
  // Pin outputFileTracingRoot to app dir so standalone build has flat structure.
  // Without this, Next.js auto-detects monorepo root and embeds build-machine
  // absolute paths — causing static file 404s when deployed elsewhere.
  outputFileTracingRoot: path.join(__dirname),

  // Spinal-cord proxy (PACT-20260424-001). Client calls /api/customer/<path>,
  // Next.js server forwards to admin gateway /api/rc/<path>, admin forwards to
  // RC /api/v1/<path>. Three-hop chain: browser -> PWA (Next) -> admin -> RC.
  // Headers (Authorization, Cookie, X-Service-Key, x-kiosk-pin) pass through
  // untouched. Admin classifies auth class per its gateway contract.
  async rewrites() {
    return [
      {
        source: "/api/customer/:path*",
        destination: `${ADMIN_GATEWAY_URL}/api/rc/:path*`,
      },
    ];
  },
};

export default nextConfig;
