"use client";

import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { ReactQueryDevtools } from "@tanstack/react-query-devtools";
import { useEffect, useState } from "react";
import { usePathname } from "next/navigation";
import { usePortfolioStore } from "@/stores/portfolio";
import { RealtimeBridge } from "@/components/realtime-bridge";
import {
  MOCK_PORTFOLIO,
  MOCK_AGENT_DECISIONS,
  MOCK_MARKET_SNAPSHOT,
} from "@/lib/mock-data";

// Only seed Zustand with mock data on the public /explore demo path. Authed
// routes must hydrate from real server data so brand-new users see their own
// (empty) portfolio, not the demo's $48k seed.
function MockDataInitializer() {
  const pathname = usePathname();
  const setPortfolios = usePortfolioStore((s) => s.setPortfolios);
  const setDecisions = usePortfolioStore((s) => s.setDecisions);
  const setMarketSnapshot = usePortfolioStore((s) => s.setMarketSnapshot);
  const isExplore = pathname?.startsWith("/explore") ?? false;
  useEffect(() => {
    if (!isExplore) return;
    setPortfolios([MOCK_PORTFOLIO]);
    setDecisions(MOCK_AGENT_DECISIONS);
    setMarketSnapshot(MOCK_MARKET_SNAPSHOT);
  }, [isExplore, setPortfolios, setDecisions, setMarketSnapshot]);
  return null;
}

export function Providers({ children }: { children: React.ReactNode }) {
  const [queryClient] = useState(
    () =>
      new QueryClient({
        defaultOptions: {
          queries: {
            staleTime: 30_000,
            retry: 1,
          },
        },
      }),
  );

  return (
    <QueryClientProvider client={queryClient}>
      <MockDataInitializer />
      <RealtimeBridge />
      {children}
      {process.env.NODE_ENV === "development" && (
        <ReactQueryDevtools initialIsOpen={false} />
      )}
    </QueryClientProvider>
  );
}
