import type { MetadataRoute } from "next";

const BASE_URL = process.env.NEXT_PUBLIC_APP_URL ?? "http://localhost:3000";

const STATIC_ROUTES: Array<{ path: string; priority: number }> = [
  { path: "/", priority: 1.0 },
  { path: "/explore", priority: 0.9 },
  { path: "/explore/conservative-retiree", priority: 0.9 },
  { path: "/explore/aggressive-builder", priority: 0.9 },
  { path: "/explore/operating-reserve", priority: 0.9 },
  { path: "/pricing", priority: 0.8 },
  { path: "/leaderboard", priority: 0.8 },
  { path: "/about", priority: 0.8 },
  { path: "/about/regime", priority: 0.7 },
  { path: "/help", priority: 0.7 },
  { path: "/policy", priority: 0.5 },
];

export default function sitemap(): MetadataRoute.Sitemap {
  return STATIC_ROUTES.map(({ path, priority }) => ({
    url: `${BASE_URL}${path}`,
    lastModified: new Date(),
    changeFrequency: path === "/" ? "daily" : "weekly",
    priority,
  }));
}
