import type { Metadata } from "next";

export const metadata: Metadata = {
  title: "Aegis · Tax Center",
  description:
    "Download settled activity reports and create temporary accountant links.",
};

export default function TaxCenterLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return children;
}
