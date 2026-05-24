import { LandingHeader } from "@/components/layout/landing-header";
import { LandingFooter } from "@/components/layout/landing-footer";

export function LandingShell({
  children,
  width = "default",
}: {
  children: React.ReactNode;
  width?: "default" | "wide";
}) {
  return (
    <div className="min-h-screen bg-bg text-text-hi flex flex-col">
      <LandingHeader />
      <main
        className={`flex-1 mx-auto w-full px-6 pb-20 pt-10 ${
          width === "wide" ? "max-w-6xl" : "max-w-4xl"
        }`}
      >
        {children}
      </main>
      <LandingFooter />
    </div>
  );
}
