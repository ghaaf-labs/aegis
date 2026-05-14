"use client";

import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { ReactQueryDevtools } from "@tanstack/react-query-devtools";
import { useEffect, useState } from "react";
import { usePortfolioStore } from "@/stores/portfolio";
import { RealtimeBridge } from "@/components/realtime-bridge";

function MockDataInitializer() {
  const initMockData = usePortfolioStore((s) => s.initMockData);
  useEffect(() => {
    initMockData();
  }, [initMockData]);
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
