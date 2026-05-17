import { AppShell } from "@/components/layout/app-shell";
import { PortfolioLoader } from "@/components/providers/portfolio-loader";

export default function AppLayout({ children }: { children: React.ReactNode }) {
  return (
    <AppShell>
      <PortfolioLoader />
      {children}
    </AppShell>
  );
}
