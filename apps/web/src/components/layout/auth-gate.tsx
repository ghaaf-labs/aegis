"use client";

import { useEffect, useState } from "react";
import { usePathname } from "next/navigation";
import Link from "next/link";
import { Shield } from "lucide-react";

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

  useEffect(() => {
    setAuthed(!!localStorage.getItem("aegis.jwt"));
  }, [pathname]);

  // Public pages always show content.
  if (isPublic(pathname)) return <>{children}</>;

  // Avoid a flash by rendering nothing until localStorage is read.
  if (authed === null) return null;

  if (authed) return <>{children}</>;

  return (
    <div className="max-w-[1400px] mx-auto flex items-center justify-center min-h-[60vh]">
      <div className="border-brutal border-border-default bg-raised p-8 max-w-sm w-full text-center space-y-4">
        <div className="flex justify-center">
          <div className="w-10 h-10 rounded-sharp bg-accent-agent flex items-center justify-center border-brutal border-black">
            <Shield className="w-5 h-5 text-black" />
          </div>
        </div>
        <div>
          <h2 className="text-base font-semibold text-text-hi font-mono">
            Create a wallet to continue
          </h2>
          <p className="text-xs text-text-lo font-mono mt-1 leading-relaxed">
            This page requires a Circle Wallet. Sign up in under a minute — no
            KYC, no credit card.
          </p>
        </div>
        <div className="flex flex-col gap-2">
          <Link
            href="/signup"
            className="inline-flex items-center justify-center px-4 py-2 bg-accent-pnl text-black font-mono font-semibold rounded-sharp border-brutal border-black shadow-brutal-sm hover:shadow-brutal transition-shadow"
          >
            Create wallet
          </Link>
          <Link
            href="/login"
            className="inline-flex items-center justify-center px-4 py-2 border-brutal border-border-default rounded-sharp text-sm font-mono text-text-lo hover:text-text-hi hover:border-border-hi transition-colors"
          >
            Sign in
          </Link>
        </div>
      </div>
    </div>
  );
}
