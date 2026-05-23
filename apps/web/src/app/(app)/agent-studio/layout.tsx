import type { Metadata } from "next";

export const metadata: Metadata = {
  title: "Aegis · Agent Studio",
  description:
    "Ask for a recommendation, pause automatic checks, and review account inputs before approving a move.",
};

export default function AgentStudioLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return children;
}
