import Link from "next/link";
import { ExternalLink, Shield } from "lucide-react";
import { PRICING_UI_ENABLED } from "@/lib/flags";

const CIRCLE_LINKS: {
  label: string;
  href: string;
  ariaLabel: string;
  comingSoon?: true;
}[] = [
  {
    label: "Wallets",
    href: "https://developers.circle.com/wallets",
    ariaLabel: "Circle Wallets documentation (opens in new tab)",
  },
  {
    label: "Gateway",
    href: "https://developers.circle.com/gateway",
    ariaLabel: "Circle Gateway documentation (opens in new tab)",
  },
  {
    label: "CCTP V2",
    href: "https://developers.circle.com/cctp",
    ariaLabel: "Circle CCTP V2 documentation (opens in new tab)",
  },
  {
    label: "USYC",
    href: "https://usyc.docs.hashnote.com",
    ariaLabel:
      "USYC by Hashnote documentation (opens in new tab) — coming soon",
    comingSoon: true,
  },
  {
    label: "Paymaster",
    href: "https://developers.circle.com/paymaster",
    ariaLabel: "Circle Paymaster documentation (opens in new tab)",
  },
  {
    label: "StableFX",
    href: "https://developers.circle.com/stablefx",
    ariaLabel: "Circle StableFX documentation (opens in new tab) — coming soon",
    comingSoon: true,
  },
  {
    label: "Nanopayments",
    href: "https://developers.circle.com/gateway/nanopayments",
    ariaLabel: "Circle Nanopayments documentation (opens in new tab)",
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
                className="touch-target flex items-center py-1 text-xs font-mono text-text-mut hover:text-text-hi transition-colors"
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
              { href: "/about/regime", label: "Regime classifier" },
              { href: "/about/regime/backtest", label: "Backtest evidence" },
              { href: "/policy", label: "Policy" },
            ].map((link) => (
              <Link
                key={link.href}
                href={link.href}
                className="touch-target flex items-center py-1 text-xs font-mono text-text-mut hover:text-text-hi transition-colors"
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
              <a
                key={link.label}
                href={link.href}
                target="_blank"
                rel="noopener noreferrer"
                aria-label={link.ariaLabel}
                className="touch-target flex items-center gap-1.5 py-1 text-xs font-mono text-text-mut hover:text-text-hi transition-colors"
              >
                {link.label}
                {link.comingSoon && (
                  <span className="text-[9px] font-mono uppercase tracking-widest text-text-mut border border-border-default px-1 py-px leading-none">
                    soon
                  </span>
                )}
                <ExternalLink
                  className="w-2.5 h-2.5 shrink-0 opacity-50"
                  aria-hidden="true"
                />
              </a>
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
