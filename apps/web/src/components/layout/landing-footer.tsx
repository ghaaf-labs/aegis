import Link from "next/link";
import { Shield } from "lucide-react";
import { PRICING_UI_ENABLED } from "@/lib/flags";

export function LandingFooter() {
  return (
    <footer className="border-t border-border-default bg-surface px-6 py-10">
      <div className="max-w-7xl mx-auto">
        <div className="flex items-center gap-2 mb-8">
          <div className="w-7 h-7 rounded-sharp bg-accent-agent flex items-center justify-center border-brutal border-black">
            <Shield className="w-3.5 h-3.5 text-black" />
          </div>
          <span className="font-bold text-text-hi font-mono">Aegis</span>
          <span className="text-xs font-mono text-text-mut ml-2">
            Built for Agora Agents Hackathon · RFB 04
          </span>
        </div>

        <div className="grid grid-cols-2 md:grid-cols-4 gap-8 mb-8">
          <div className="space-y-3">
            <p className="text-xs font-mono text-text-lo uppercase tracking-widest">
              Product
            </p>
            {[
              { href: "/strategies", label: "Strategies" },
              { href: "/leaderboard", label: "Leaderboard" },
              { href: "/explore", label: "Demo portfolios" },
              ...(PRICING_UI_ENABLED
                ? [{ href: "/pricing", label: "Pricing" }]
                : []),
            ].map((link) => (
              <Link
                key={link.href}
                href={link.href}
                className="block text-xs font-mono text-text-mut hover:text-text-hi transition-colors"
              >
                {link.label}
              </Link>
            ))}
          </div>
          <div className="space-y-3">
            <p className="text-xs font-mono text-text-lo uppercase tracking-widest">
              Transparency
            </p>
            {[
              { href: "/about/constitution", label: "Agent constitution" },
              { href: "/about/regime", label: "Regime model" },
              { href: "/policy", label: "Policy" },
            ].map((link) => (
              <Link
                key={link.href}
                href={link.href}
                className="block text-xs font-mono text-text-mut hover:text-text-hi transition-colors"
              >
                {link.label}
              </Link>
            ))}
          </div>
          <div className="space-y-3">
            <p className="text-xs font-mono text-text-lo uppercase tracking-widest">
              Get started
            </p>
            {[
              { href: "/signup", label: "Create wallet" },
              { href: "/login", label: "Sign in" },
              { href: "/onboarding", label: "Build portfolio" },
            ].map((link) => (
              <Link
                key={link.href}
                href={link.href}
                className="block text-xs font-mono text-text-mut hover:text-text-hi transition-colors"
              >
                {link.label}
              </Link>
            ))}
          </div>
          <div className="space-y-3">
            <p className="text-xs font-mono text-text-lo uppercase tracking-widest">
              Circle stack
            </p>
            {[
              "Wallets",
              "Gateway",
              "CCTP V2",
              "USYC",
              "Paymaster",
              "StableFX",
              "Nanopayments",
            ].map((api) => (
              <span key={api} className="block text-xs font-mono text-text-mut">
                {api}
              </span>
            ))}
          </div>
        </div>

        <div className="border-t border-border-default pt-6 text-xs font-mono text-text-mut text-center">
          Aegis · AI-powered stablecoin portfolio management · Arc + Base ·
          Circle APIs
        </div>
      </div>
    </footer>
  );
}
