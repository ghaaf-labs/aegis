import type { MetadataRoute } from "next";

const BASE_URL = process.env.NEXT_PUBLIC_APP_URL ?? "http://localhost:3000";

const isIndexable = process.env.NEXT_PUBLIC_SITE_INDEXABLE === "true";

export default function robots(): MetadataRoute.Robots {
  if (!isIndexable) {
    return {
      rules: { userAgent: "*", disallow: "/" },
    };
  }

  return {
    rules: [
      {
        userAgent: "*",
        allow: "/",
        disallow: [
          "/dashboard",
          "/wallets",
          "/settings",
          "/transactions",
          "/analytics",
          "/agent-logs",
          "/agent-studio",
          "/tax-center",
          "/portfolio",
          "/rebalance",
          "/onboarding",
        ],
      },
    ],
    sitemap: `${BASE_URL}/sitemap.xml`,
  };
}
