import type { NextConfig } from "next";
import path from "node:path";

const nextConfig: NextConfig = {
  reactStrictMode: true,
  outputFileTracingRoot: path.join(process.cwd(), ".."),
  output: process.env.HOWLLO_DOCKER_BUILD === "true" ? "standalone" : undefined,
  experimental: { externalDir: true },
};

export default nextConfig;
