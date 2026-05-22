import type { Metadata } from "next";

export const metadata: Metadata = {
  title: "Aegis · Settings",
  description:
    "Manage account email, data export, account closure, notifications, and app settings.",
};

export default function SettingsLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return children;
}
