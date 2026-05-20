import type { NextConfig } from "next";

const nextConfig: NextConfig = {
  output: "standalone",
  // The bottom-left "N" dev indicator overlapped the sidebar's
  // "AGENT ACTIVE" status pill, leaving "...ENT ACTIVE" visible during
  // local dev. Hidden so the sidebar reads cleanly.
  devIndicators: false,
  transpilePackages: ["@aegis/ui", "@aegis/shared"],
  images: {
    remotePatterns: [
      {
        protocol: "https",
        hostname: "assets.coingecko.com",
      },
      {
        protocol: "https",
        hostname: "coin-images.coingecko.com",
      },
    ],
  },
  async rewrites() {
    return [
      {
        source: "/api/backend/:path*",
        destination: `${process.env.NEXT_PUBLIC_API_URL ?? "http://localhost:8080"}/:path*`,
      },
    ];
  },
};

export default nextConfig;
