"use client";

import { useEffect } from "react";
import { usePathname } from "next/navigation";
import { walletApi } from "@/lib/api";
import { usePortfolioStore } from "@/stores/portfolio";

/**
 * Single session probe for app chrome. Runs once per navigation so AuthGate,
 * AppShell, and Sidebar share the same resolved state without duplicate fetches
 * or signed-out flashes while the cookie is still valid.
 */
export function SessionBootstrap() {
  const pathname = usePathname();
  const setSessionActive = usePortfolioStore((s) => s.setSessionActive);
  const setWallet = usePortfolioStore((s) => s.setWallet);
  const setSessionResolved = usePortfolioStore((s) => s.setSessionResolved);
  const resetSession = usePortfolioStore((s) => s.resetSession);

  useEffect(() => {
    let cancelled = false;
    walletApi
      .session()
      .then((session) => {
        if (cancelled) return;
        localStorage.setItem("aegis_email", session.user.email);
        setSessionActive(true);
        setWallet(session.wallet);
      })
      .catch(() => {
        if (!cancelled) resetSession();
      })
      .finally(() => {
        if (!cancelled) setSessionResolved(true);
      });
    return () => {
      cancelled = true;
    };
  }, [pathname, resetSession, setSessionActive, setSessionResolved, setWallet]);

  return null;
}
