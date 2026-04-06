import type { NextConfig } from "next";
import path from "path";

const nextConfig: NextConfig = {
  output: "standalone",
  basePath: "/kiosk",
  // Pin outputFileTracingRoot to kiosk dir so standalone build has flat structure.
  // Without this, Next.js auto-detects the monorepo root (C:\Users\bono) and embeds
  // build-machine absolute paths in server.js + required-server-files.json — causing
  // static file 404s when deployed to a different machine/path.
  outputFileTracingRoot: path.join(__dirname),
  async redirects() {
    return [
      {
        source: "/",
        destination: "/kiosk",
        basePath: false,
        permanent: true,
      },
    ];
  },
  async rewrites() {
    // Proxy API calls to racecontrol server so kiosk at :3300 works standalone
    const apiDest = process.env.NEXT_PUBLIC_API_URL || "http://192.168.31.23:8080";
    return [
      {
        source: "/api/:path*",
        destination: `${apiDest}/api/:path*`,
      },
    ];
  },
};

export default nextConfig;
