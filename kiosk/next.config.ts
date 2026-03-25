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
};

export default nextConfig;
