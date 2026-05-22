import type { Metadata } from "next";
import type { ReactNode } from "react";

export const metadata: Metadata = {
  title: "Aegis · Analytics",
  description:
    "Track portfolio value, wallet cash, targets, market context, and decision quality.",
};

export default function AnalyticsLayout({ children }: { children: ReactNode }) {
  return children;
}
