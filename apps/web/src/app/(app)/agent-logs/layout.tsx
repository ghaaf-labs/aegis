import type { Metadata } from "next";

export const metadata: Metadata = {
  title: "Aegis · Agent Logs",
  description:
    "Review past recommendations, confidence, safety notes, and whether they still match your current account.",
};

export default function AgentLogsLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return children;
}
