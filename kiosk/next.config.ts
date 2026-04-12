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
    // Proxy API calls to racecontrol server so kiosk at :3300 works standalone.
    // basePath: false on each rule is REQUIRED — without it, Next.js auto-prefixes
    // the source with basePath ("/kiosk"), so the rewrite only matches
    // /kiosk/api/* but NOT bare /api/*. The browser's fetchApi() sends
    // fetch("http://host:3300/api/v1/...") (no /kiosk prefix), which means
    // the rewrite never fires and the request hits Next.js as a 404 page route.
    // This broke staff PIN login, experiences fetch, and ALL API calls on port 3300.
    const apiDest = process.env.NEXT_PUBLIC_API_URL || "http://192.168.31.23:8080";
    return {
      // beforeFiles rewrites are checked before pages — API calls never reach the page router
      beforeFiles: [
        {
          source: "/api/:path*",
          destination: `${apiDest}/api/:path*`,
          basePath: false,
        },
      ],
    };
  },
};

export default nextConfig;
