import type { Metadata } from "next";
import { notFound } from "next/navigation";
import { PRICING_UI_ENABLED } from "@/lib/flags";
import { BillingSettingsClient } from "./billing-client";

export const metadata: Metadata = {
  title: "Billing — Aegis",
};

export default function BillingSettingsPage() {
  if (!PRICING_UI_ENABLED) {
    notFound();
  }
  return <BillingSettingsClient />;
}
