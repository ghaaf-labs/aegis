import type { Metadata } from "next";
import { redirect } from "next/navigation";
import { PRICING_UI_ENABLED } from "@/lib/flags";
import { PricingPageClient } from "./pricing-client";

export const metadata: Metadata = {
  title: "Pricing — Aegis",
  description:
    "Free, Pro ($19/mo), and Business ($199/mo). Stablecoin-native portfolio agent — pay in USDC via Circle Nanopayments. No hidden swap spread. No charging on failed execution.",
  openGraph: {
    title: "Pricing — Aegis",
    description: "Three tiers. USDC-native billing. No hidden fees.",
    type: "website",
  },
};

export default function PricingPage() {
  if (!PRICING_UI_ENABLED) {
    redirect("/settings/billing");
  }
  return <PricingPageClient />;
}
