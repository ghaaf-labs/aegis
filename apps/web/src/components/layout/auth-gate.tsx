"use client";

import { useEffect, useState } from "react";
import { usePathname, useRouter } from "next/navigation";
import { walletApi } from "@/lib/api";
import { safeNextPath } from "@/lib/auth-routing";
import { usePortfolioStore } from "@/stores/portfolio";

// Paths that are publicly viewable without a wallet.
const PUBLIC_PREFIXES = ["/leaderboard", "/explore", "/strategies", "/help"];
const WALLET_RECOVERY_PATHS = new Set(["/wallet", "/wallets", "/settings"]);
const PORTFOLIO_REQUIRED_PREFIXES = [
  "/dashboard",
  "/portfolio",
  "/transactions",
  "/analytics",
  "/agent-logs",
  "/agent-studio",
  "/tax-center",
  "/rebalance",
];

type AuthState =
  | { kind: "checking" }
  | { kind: "signed_out" }
  | { kind: "ready" }
  | { kind: "wallet_pending"; email: string };

function isPublic(pathname: string) {
  return PUBLIC_PREFIXES.some(
    (p) => pathname === p || pathname.startsWith(p + "/"),
  );
}

export function AuthGate({ children }: { children: React.ReactNode }) {
  const pathname = usePathname();
  const router = useRouter();
  const [authState, setAuthState] = useState<AuthState>({ kind: "checking" });
  const setWallet = usePortfolioStore((s) => s.setWallet);
  const setSessionActive = usePortfolioStore((s) => s.setSessionActive);
  const sessionActive = usePortfolioStore((s) => s.sessionActive);
  const portfoliosLoaded = usePortfolioStore((s) => s.portfoliosLoaded);
  const portfolioCount = usePortfolioStore((s) => s.portfolios.length);
  const resetSession = usePortfolioStore((s) => s.resetSession);

  useEffect(() => {
    let cancelled = false;
    setAuthState({ kind: "checking" });
    walletApi
      .session()
      .then((session) => {
        if (cancelled) return;
        localStorage.setItem("aegis_email", session.user.email);
        setSessionActive(true);
        if (session.wallet) {
          setWallet(session.wallet);
          setAuthState({ kind: "ready" });
        } else {
          setWallet(null);
          setAuthState({ kind: "wallet_pending", email: session.user.email });
        }
      })
      .catch(() => {
        if (!cancelled) {
          resetSession();
          setAuthState({ kind: "signed_out" });
        }
      });
    return () => {
      cancelled = true;
    };
  }, [pathname, resetSession, setSessionActive, setWallet]);

  useEffect(() => {
    if (isPublic(pathname) || authState.kind === "checking") return;

    if (authState.kind === "signed_out") {
      router.replace(authHref("/login", pathname));
      return;
    }

    if (
      authState.kind === "wallet_pending" &&
      !isWalletRecoveryPath(pathname)
    ) {
      router.replace("/onboarding");
      return;
    }

    if (
      authState.kind === "ready" &&
      sessionActive &&
      portfoliosLoaded &&
      portfolioCount === 0 &&
      requiresPortfolio(pathname)
    ) {
      router.replace("/onboarding");
      return;
    }

    if (authState.kind === "ready" && !sessionActive) {
      router.replace(authHref("/login", pathname));
    }
  }, [
    authState.kind,
    pathname,
    portfolioCount,
    portfoliosLoaded,
    router,
    sessionActive,
  ]);

  // Public pages always show content.
  if (isPublic(pathname)) return <>{children}</>;

  // Avoid flashing the signed-out prompt until the server session check resolves.
  if (authState.kind === "checking") return null;

  if (authState.kind === "ready" && sessionActive) {
    if (
      requiresPortfolio(pathname) &&
      (!portfoliosLoaded || portfolioCount === 0)
    ) {
      return null;
    }
    return <>{children}</>;
  }

  if (
    authState.kind === "wallet_pending" &&
    sessionActive &&
    isWalletRecoveryPath(pathname)
  ) {
    return <>{children}</>;
  }

  return null;
}

function isWalletRecoveryPath(pathname: string) {
  return WALLET_RECOVERY_PATHS.has(pathname);
}

function requiresPortfolio(pathname: string) {
  return PORTFOLIO_REQUIRED_PREFIXES.some(
    (p) => pathname === p || pathname.startsWith(`${p}/`),
  );
}

function authHref(path: "/login", next: string) {
  const params = new URLSearchParams();
  const safeNext = safeNextPath(next);
  if (safeNext) params.set("next", safeNext);
  const query = params.toString();
  return query ? `${path}?${query}` : path;
}
