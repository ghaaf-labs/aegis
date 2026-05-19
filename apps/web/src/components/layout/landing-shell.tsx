import Link from "next/link";
import { Shield } from "lucide-react";
import { BrutalButton } from "@aegis/ui";
import { PRICING_UI_ENABLED } from "@/lib/flags";

export function LandingShell({ children }: { children: React.ReactNode }) {
  return (
    <div className="min-h-screen bg-bg text-text-hi flex flex-col">
      <nav className="flex items-center justify-between px-6 py-5 max-w-5xl mx-auto border-b border-border-default">
        <Link href="/" className="flex items-center gap-2 group">
          <div className="w-8 h-8 rounded-sharp bg-accent-agent flex items-center justify-center border-brutal border-black">
            <Shield className="w-4 h-4 text-black" />
          </div>
          <span className="font-bold text-lg tracking-tight text-text-hi font-mono group-hover:text-accent-agent transition-colors">
            Aegis
          </span>
        </Link>
        <div className="flex items-center gap-4">
          {PRICING_UI_ENABLED && (
            <Link
              href="/pricing"
              className="hidden md:block text-xs font-mono text-text-lo hover:text-text-hi transition-colors"
            >
              Pricing
            </Link>
          )}
          <Link
            href="/strategies"
            className="hidden md:block text-xs font-mono text-text-lo hover:text-text-hi transition-colors"
          >
            Strategies
          </Link>
          <Link
            href="/leaderboard"
            className="hidden md:block text-xs font-mono text-text-lo hover:text-text-hi transition-colors"
          >
            Leaderboard
          </Link>
          <Link href="/signup">
            <BrutalButton variant="pnl">Get started</BrutalButton>
          </Link>
        </div>
      </nav>

      <main className="flex-1 max-w-4xl mx-auto w-full px-6 pb-20 pt-10">
        {children}
      </main>

      <footer className="border-t border-border-default bg-surface px-6 py-8">
        <div className="max-w-5xl mx-auto flex flex-col md:flex-row items-start md:items-center justify-between gap-6">
          <div className="flex items-center gap-2">
            <div className="w-6 h-6 rounded-sharp bg-accent-agent flex items-center justify-center border-brutal border-black">
              <Shield className="w-3 h-3 text-black" />
            </div>
            <span className="font-bold text-sm font-mono text-text-hi">
              Aegis
            </span>
          </div>
          <div className="flex flex-wrap gap-x-6 gap-y-2 text-xs font-mono text-text-mut">
            <Link href="/" className="hover:text-text-hi transition-colors">
              Home
            </Link>
            <Link
              href="/explore"
              className="hover:text-text-hi transition-colors"
            >
              Explore demo
            </Link>
            <Link
              href="/strategies"
              className="hover:text-text-hi transition-colors"
            >
              Strategies
            </Link>
            <Link
              href="/leaderboard"
              className="hover:text-text-hi transition-colors"
            >
              Leaderboard
            </Link>
            <Link
              href="/about/constitution"
              className="hover:text-text-hi transition-colors"
            >
              Constitution
            </Link>
            <Link
              href="/about/regime"
              className="hover:text-text-hi transition-colors"
            >
              Regime model
            </Link>
            <Link
              href="/policy"
              className="hover:text-text-hi transition-colors"
            >
              Policy
            </Link>
          </div>
        </div>
      </footer>
    </div>
  );
}
