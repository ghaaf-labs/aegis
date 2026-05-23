import type { Portfolio, PortfolioId } from "@/types";

export function dashboardDestination(
  portfolios: Portfolio[],
  activePortfolioId: PortfolioId | null,
) {
  return (
    portfolios.find((portfolio) => portfolio.id === activePortfolioId) ??
    portfolios[0] ??
    null
  );
}
