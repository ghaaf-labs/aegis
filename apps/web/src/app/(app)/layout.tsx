import { AppShell } from "@/components/layout/app-shell";
import { PortfolioLoader } from "@/components/providers/portfolio-loader";
import { AuthGate } from "@/components/layout/auth-gate";

export default function AppLayout({ children }: { children: React.ReactNode }) {
  return (
    <AppShell>
      <PortfolioLoader />
      <AuthGate>{children}</AuthGate>
    </AppShell>
  );
}
