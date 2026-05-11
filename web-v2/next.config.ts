import path from "node:path";
import type { NextConfig } from "next";

const config: NextConfig = {
  basePath: "/v2",
  output: "standalone",
  outputFileTracingRoot: path.join(__dirname),
  reactStrictMode: true,
  poweredByHeader: false,
};

export default config;
