import Link from "next/link";
import { Shield } from "lucide-react";
import { BrutalButton } from "@aegis/ui";
import { PRICING_UI_ENABLED } from "@/lib/flags";

export function LandingHeader() {
  return (
    <header className="sticky top-0 z-50 border-b border-border-default bg-bg/95 backdrop-blur-sm">
      <nav className="flex items-center justify-between px-6 py-4 max-w-7xl mx-auto">
        <Link href="/" className="flex items-center gap-2 group">
          <div className="w-8 h-8 rounded-sharp bg-accent-agent flex items-center justify-center border-brutal border-black shrink-0">
            <Shield className="w-4 h-4 text-black" />
          </div>
          <span className="font-bold text-lg tracking-tight text-text-hi font-mono group-hover:text-accent-agent transition-colors">
            Aegis
          </span>
        </Link>
        <div className="hidden md:flex items-center gap-4">
          {PRICING_UI_ENABLED && (
            <Link
              href="/pricing"
              className="text-xs font-mono text-text-lo hover:text-text-hi transition-colors"
            >
              Pricing
            </Link>
          )}
          <Link
            href="/explore"
            className="text-xs font-mono text-text-lo hover:text-text-hi transition-colors"
          >
            Explore demo
          </Link>
          <Link
            href="/strategies"
            className="text-xs font-mono text-text-lo hover:text-text-hi transition-colors"
          >
            Strategies
          </Link>
          <Link
            href="/leaderboard"
            className="text-xs font-mono text-text-lo hover:text-text-hi transition-colors"
          >
            Leaderboard
          </Link>
          <Link href="/signup">
            <BrutalButton variant="pnl">Get started</BrutalButton>
          </Link>
        </div>
        <div className="md:hidden">
          <Link href="/signup">
            <BrutalButton variant="pnl">Get started</BrutalButton>
          </Link>
        </div>
      </nav>
    </header>
  );
}
