"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import { Shield } from "lucide-react";
import { PRICING_UI_ENABLED } from "@/lib/flags";

export function LandingHeader() {
  const pathname = usePathname();

  return (
    <header className="sticky top-0 z-50 border-b border-border-default bg-bg/95 backdrop-blur-sm">
      <nav
        aria-label="Site navigation"
        className="flex items-center justify-between px-6 py-4 max-w-7xl mx-auto"
      >
        <Link href="/" className="flex min-h-9 items-center gap-2 group">
          <div className="w-8 h-8 rounded-sharp bg-accent-agent flex items-center justify-center border-brutal border-black shrink-0">
            <Shield className="w-4 h-4 text-black" />
          </div>
          <span className="font-bold text-lg tracking-tight text-text-hi font-mono group-hover:text-accent-agent transition-colors">
            Aegis
          </span>
        </Link>

        {/* Desktop nav links — hidden on mobile via `hidden md:flex`. */}
        <div className="hidden md:flex items-center gap-4">
          {PRICING_UI_ENABLED && (
            <Link
              href="/pricing"
              aria-current={pathname === "/pricing" ? "page" : undefined}
              className="text-xs font-mono text-text-lo hover:text-text-hi transition-colors"
            >
              Pricing
            </Link>
          )}
          <Link
            href="/explore"
            aria-current={
              pathname === "/explore" || pathname.startsWith("/explore/")
                ? "page"
                : undefined
            }
            className="text-xs font-mono text-text-lo hover:text-text-hi transition-colors"
          >
            Explore demo
          </Link>
          <Link
            href="/leaderboard"
            aria-current={pathname === "/leaderboard" ? "page" : undefined}
            className="text-xs font-mono text-text-lo hover:text-text-hi transition-colors"
          >
            Leaderboard
          </Link>
          <Link
            href="/about"
            aria-current={
              pathname === "/about" || pathname.startsWith("/about/")
                ? "page"
                : undefined
            }
            className="text-xs font-mono text-text-lo hover:text-text-hi transition-colors"
          >
            About
          </Link>
          {/* Single semantic anchor — no nested button. */}
          <Link
            href="/login"
            className="inline-flex items-center justify-center gap-2 px-3 py-2 text-sm font-semibold border-brutal border-black rounded-sharp bg-accent-pnl text-black transition-[box-shadow] hover:shadow-brutal-sm active:translate-y-px"
          >
            Get started
          </Link>
        </div>

        {/* Mobile CTA — only rendered in the mobile breakpoint range.
            `md:hidden` removes it from the DOM (and the a11y tree) on desktop,
            avoiding the duplicate focusable control that existed when both the
            desktop and mobile CTAs were present simultaneously. */}
        <div className="md:hidden">
          <Link
            href="/login"
            className="inline-flex items-center justify-center gap-2 px-3 py-2 text-sm font-semibold border-brutal border-black rounded-sharp bg-accent-pnl text-black transition-[box-shadow] hover:shadow-brutal-sm active:translate-y-px"
          >
            Get started
          </Link>
        </div>
      </nav>
    </header>
  );
}
