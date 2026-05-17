import type { Metadata } from "next";
import { BillingSettingsClient } from "./billing-client";

export const metadata: Metadata = {
  title: "Billing — Aegis",
};

export default function BillingSettingsPage() {
  return <BillingSettingsClient />;
}
