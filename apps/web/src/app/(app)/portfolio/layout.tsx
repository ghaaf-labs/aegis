import type { Metadata } from "next";

export const metadata: Metadata = {
  title: "Aegis · Portfolio",
  description:
    "Review positions, targets, wallet cash, and rebalance readiness before approving a move.",
};

export default function PortfolioLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return children;
}
