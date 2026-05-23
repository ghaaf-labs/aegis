import type { Metadata } from "next";
import { PricingPageClient } from "./pricing-client";
import { pageMetadata } from "@/lib/seo";

export const metadata: Metadata = pageMetadata({
  title: "Pricing — Aegis",
  description:
    "Free, Pro ($19/mo), and Business ($199/mo). Stablecoin-native portfolio agent — pay in USDC via Circle Nanopayments. No hidden swap spread. No charging on failed execution.",
  path: "/pricing",
});

export default function PricingPage() {
  return (
    <main>
      <PricingPageClient />
    </main>
  );
}
