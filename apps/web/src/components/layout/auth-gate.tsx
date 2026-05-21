"use client";

import { useEffect, useState } from "react";
import { usePathname } from "next/navigation";
import Link from "next/link";
import { Shield } from "lucide-react";
import { walletApi } from "@/lib/api";
import { usePortfolioStore } from "@/stores/portfolio";

// Paths that are publicly viewable without a wallet.
const PUBLIC_PREFIXES = ["/leaderboard", "/explore", "/strategies"];

function isPublic(pathname: string) {
  return PUBLIC_PREFIXES.some(
    (p) => pathname === p || pathname.startsWith(p + "/"),
  );
}

export function AuthGate({ children }: { children: React.ReactNode }) {
  const pathname = usePathname();
  const [authed, setAuthed] = useState<boolean | null>(null);
  const setSessionActive = usePortfolioStore((s) => s.setSessionActive);

  useEffect(() => {
    let cancelled = false;
    setAuthed(null);
    walletApi
      .me()
      .then((user) => {
        if (cancelled) return;
        localStorage.setItem("aegis_email", user.email);
        setSessionActive(true);
        setAuthed(true);
      })
      .catch(() => {
        if (!cancelled) {
          setSessionActive(false);
          setAuthed(false);
        }
      });
    return () => {
      cancelled = true;
    };
  }, [pathname, setSessionActive]);

  // Public pages always show content.
  if (isPublic(pathname)) return <>{children}</>;

  // Avoid flashing the signed-out prompt until the server session check resolves.
  if (authed === null) return null;

  if (authed) return <>{children}</>;

  return (
    <div className="max-w-[1400px] mx-auto flex items-center justify-center min-h-[60vh]">
      <div
        data-testid="auth-gate-message"
        className="border-brutal border-border-default bg-raised p-8 max-w-sm w-full text-center space-y-4"
      >
        <div className="flex justify-center">
          <div className="w-10 h-10 rounded-sharp bg-accent-agent flex items-center justify-center border-brutal border-black">
            <Shield className="w-5 h-5 text-black" />
          </div>
        </div>
        <div>
          <h2 className="text-base font-semibold text-text-hi font-mono">
            Sign in to continue
          </h2>
          <p className="text-xs text-text-lo font-mono mt-1 leading-relaxed">
            This page needs your Aegis wallet session. Use the same email you
            registered with, or create a wallet if this is your first visit.
          </p>
        </div>
        <div className="flex flex-col gap-2">
          <Link
            href="/login"
            className="inline-flex items-center justify-center px-4 py-2 bg-accent-agent text-black font-mono font-semibold rounded-sharp border-brutal border-black shadow-brutal-sm hover:shadow-brutal transition-shadow"
          >
            Sign in
          </Link>
          <Link
            href="/signup"
            className="inline-flex items-center justify-center px-4 py-2 border-brutal border-border-default rounded-sharp text-sm font-mono text-text-lo hover:text-text-hi hover:border-border-hi transition-colors"
          >
            Create wallet
          </Link>
        </div>
      </div>
    </div>
  );
}
