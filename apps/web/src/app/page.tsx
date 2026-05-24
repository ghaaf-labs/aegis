"use client";

import { useEffect, useState } from "react";
import {
  leaderboardApi,
  tractionApi,
  type LeaderboardEntry,
  type Traction,
} from "@/lib/api";
import { LandingHeader } from "@/components/layout/landing-header";
import { LandingFooter } from "@/components/layout/landing-footer";
import { LEADERBOARD_PREVIEW_COUNT } from "@/components/landing/landing-data";
import { AnnouncementBar } from "@/components/landing/announcement-bar";
import { Hero } from "@/components/landing/hero";
import { DashboardMockup } from "@/components/landing/dashboard-mockup";
import { HowItWorks } from "@/components/landing/how-it-works";
import { ReasoningShowcase } from "@/components/landing/reasoning-showcase";
import { ProductTour } from "@/components/landing/product-tour";
import { CircleStack } from "@/components/landing/circle-stack";
import { TrustStats } from "@/components/landing/trust-stats";
import { FeaturesGrid } from "@/components/landing/features-grid";
import { LeaderboardPreview } from "@/components/landing/leaderboard-preview";
import { Cta } from "@/components/landing/cta";

export default function LandingPage() {
  const [traction, setTraction] = useState<Traction | null>(null);
  const [topPerformers, setTopPerformers] = useState<LeaderboardEntry[]>([]);
  const [statsLoaded, setStatsLoaded] = useState(false);

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      const [t, board] = await Promise.allSettled([
        tractionApi.get(),
        leaderboardApi.list(LEADERBOARD_PREVIEW_COUNT),
      ]);
      if (cancelled) return;
      if (t.status === "fulfilled") setTraction(t.value);
      if (board.status === "fulfilled") setTopPerformers(board.value);
      setStatsLoaded(true);
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  return (
    <div className="min-h-screen bg-bg text-text-hi overflow-hidden">
      {/* Ambient glow — on-system tokens only */}
      <div className="fixed inset-0 pointer-events-none">
        <div className="absolute top-[-10%] right-[5%] w-[500px] h-[500px] bg-accent-agent/5 rounded-full blur-[120px]" />
        <div className="absolute bottom-[15%] left-[10%] w-[400px] h-[400px] bg-accent-pnl/5 rounded-full blur-[120px]" />
      </div>

      <AnnouncementBar />

      <LandingHeader />

      <main>
        <Hero />
        <DashboardMockup />
        <HowItWorks />
        <ReasoningShowcase />
        <ProductTour />
        <CircleStack />
        <TrustStats traction={traction} statsLoaded={statsLoaded} />
        <FeaturesGrid />
        <LeaderboardPreview
          topPerformers={topPerformers}
          statsLoaded={statsLoaded}
        />
        <Cta />
      </main>

      <LandingFooter />
    </div>
  );
}
