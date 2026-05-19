import { LandingHeader } from "@/components/layout/landing-header";
import { LandingFooter } from "@/components/layout/landing-footer";

export function LandingShell({ children }: { children: React.ReactNode }) {
  return (
    <div className="min-h-screen bg-bg text-text-hi">
      <LandingHeader />
      <main className="max-w-4xl mx-auto px-6 pb-20 pt-10">{children}</main>
      <LandingFooter />
    </div>
  );
}
