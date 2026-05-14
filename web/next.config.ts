import type { NextConfig } from "next";
import path from "path";

const nextConfig: NextConfig = {
  output: "standalone",
  // Pin outputFileTracingRoot to app dir so standalone build has flat structure.
  // Without this, Next.js auto-detects monorepo root and embeds build-machine
  // absolute paths — causing static file 404s when deployed elsewhere.
  outputFileTracingRoot: path.join(__dirname),
  // Same-origin /api/* proxy to racecontrol backend. Configurable via
  // API_PROXY_TARGET env var (default localhost:8080 — works for venue Server .23
  // and cloud Bono VPS where backend pm2 runs on same host as web server process).
  async rewrites() {
    return [
      {
        source: "/api/:path*",
        destination: `${process.env.API_PROXY_TARGET || "http://localhost:8080"}/api/:path*`,
      },
    ];
  },
};

export default nextConfig;
