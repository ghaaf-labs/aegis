import { ErrorBoundary } from "@/components/error-boundary";

export default function PublicLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return <ErrorBoundary>{children}</ErrorBoundary>;
}
