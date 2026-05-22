import { AppShell } from "@/components/layout/app-shell";
import { PortfolioLoader } from "@/components/providers/portfolio-loader";
import { SessionBootstrap } from "@/components/providers/session-bootstrap";
import { AuthGate } from "@/components/layout/auth-gate";

export default function AppLayout({ children }: { children: React.ReactNode }) {
  return (
    <>
      <SessionBootstrap />
      <AppShell>
        <PortfolioLoader />
        <AuthGate>{children}</AuthGate>
      </AppShell>
    </>
  );
}
