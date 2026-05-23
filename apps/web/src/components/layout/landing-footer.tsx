import Link from "next/link";
import { Shield } from "lucide-react";
import { PRICING_UI_ENABLED } from "@/lib/flags";

const CIRCLE_LINKS = [
  {
    label: "Wallets",
    href: "https://developers.circle.com/w3s/docs/programmable-wallets-overview",
  },
  {
    label: "Gateway",
    href: "https://developers.circle.com/w3s/docs/circle-gateway",
  },
  {
    label: "CCTP V2",
    href: "https://developers.circle.com/stablecoins/docs/cctp-getting-started",
  },
  {
    label: "USYC",
    href: "https://developers.circle.com/stablecoins/docs/usyc",
  },
  {
    label: "Paymaster",
    href: "https://developers.circle.com/w3s/docs/gas-station",
  },
  {
    label: "StableFX",
    href: "https://developers.circle.com/w3s/docs/stablefx",
  },
  {
    label: "Nanopayments",
    href: "https://developers.circle.com/payments/docs/nanopayments",
  },
];

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

        <div className="grid grid-cols-2 md:grid-cols-3 gap-8 mb-8">
          <div className="space-y-2">
            <p className="text-xs font-mono text-text-lo uppercase tracking-widest py-1">
              Product
            </p>
            {[
              { href: "/leaderboard", label: "Leaderboard" },
              { href: "/explore", label: "Demo portfolios" },
              ...(PRICING_UI_ENABLED
                ? [{ href: "/pricing", label: "Pricing" }]
                : []),
            ].map((link) => (
              <Link
                key={link.href}
                href={link.href}
                className="flex items-center py-1 text-xs font-mono text-text-mut hover:text-text-hi transition-colors"
              >
                {link.label}
              </Link>
            ))}
          </div>

          <div className="space-y-2">
            <p className="text-xs font-mono text-text-lo uppercase tracking-widest py-1">
              Transparency
            </p>
            {[
              { href: "/about", label: "About us" },
              { href: "/about/regime", label: "Regime model" },
              { href: "/policy", label: "Policy" },
            ].map((link) => (
              <Link
                key={link.href}
                href={link.href}
                className="flex items-center py-1 text-xs font-mono text-text-mut hover:text-text-hi transition-colors"
              >
                {link.label}
              </Link>
            ))}
          </div>

          <div className="space-y-2">
            <p className="text-xs font-mono text-text-lo uppercase tracking-widest py-1">
              Circle stack
            </p>
            {CIRCLE_LINKS.map((link) => (
              <Link
                key={link.label}
                href={link.href}
                target="_blank"
                rel="noopener noreferrer"
                className="flex items-center py-1 text-xs font-mono text-text-mut hover:text-text-hi transition-colors"
              >
                {link.label}
              </Link>
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
