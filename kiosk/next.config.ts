import type { NextConfig } from "next";

const nextConfig: NextConfig = {
  output: "standalone",
  basePath: "/kiosk",
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
