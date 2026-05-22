import type { Metadata } from "next";
import type { ReactNode } from "react";

export const metadata: Metadata = {
  title: "Aegis · Transactions",
  description:
    "Review approved moves, execution status, and completed portfolio activity.",
};

export default function TransactionsLayout({
  children,
}: {
  children: ReactNode;
}) {
  return children;
}
