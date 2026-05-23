"use client";

import { useEffect, useState } from "react";
import { usePathname, useRouter } from "next/navigation";
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
  const sessionResolved = usePortfolioStore((s) => s.sessionResolved);
  const sessionActive = usePortfolioStore((s) => s.sessionActive);
  const wallet = usePortfolioStore((s) => s.wallet);
  const portfoliosLoaded = usePortfolioStore((s) => s.portfoliosLoaded);
  const portfoliosError = usePortfolioStore((s) => s.portfoliosError);
  const portfolioCount = usePortfolioStore((s) => s.portfolios.length);

  useEffect(() => {
    if (!sessionResolved) {
      setAuthState({ kind: "checking" });
      return;
    }

    if (!sessionActive) {
      setAuthState({ kind: "signed_out" });
      return;
    }

    if (!wallet) {
      setAuthState({
        kind: "wallet_pending",
        email: localStorage.getItem("aegis_email") ?? "",
      });
      return;
    }

    setAuthState({ kind: "ready" });
  }, [sessionActive, sessionResolved, wallet]);

  useEffect(() => {
    if (isPublic(pathname) || authState.kind === "checking") return;

    if (authState.kind === "signed_out") {
      router.replace(authHref("/login", pathname, "session_required"));
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
      !portfoliosError &&
      requiresPortfolio(pathname)
    ) {
      router.replace("/onboarding");
      return;
    }

    if (authState.kind === "ready" && !sessionActive) {
      router.replace(authHref("/login", pathname, "session_required"));
    }
  }, [
    authState.kind,
    pathname,
    portfolioCount,
    portfoliosLoaded,
    portfoliosError,
    router,
    sessionActive,
  ]);

  // Public pages always show content.
  if (isPublic(pathname)) return <>{children}</>;

  // Hard navigations are already protected by middleware. Rendering while the
  // client probe settles keeps SSR usable if hydration is delayed.
  if (authState.kind === "checking") return <>{children}</>;

  if (authState.kind === "ready" && sessionActive) {
    if (requiresPortfolio(pathname) && portfoliosError) {
      return <PortfolioLoadError />;
    }
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

function PortfolioLoadError() {
  return (
    <div className="flex min-h-[50vh] flex-col items-center justify-center px-6 text-center">
      <p className="font-mono text-[11px] uppercase tracking-widest text-warn">
        Connection issue
      </p>
      <h1 className="mt-2 font-mono text-xl font-semibold text-text-hi">
        Couldn&apos;t load your portfolios
      </h1>
      <p className="mt-2 max-w-sm font-mono text-xs leading-relaxed text-text-lo">
        Your account is fine — Aegis just couldn&apos;t reach your portfolio
        data. Try again in a moment.
      </p>
      <button
        type="button"
        onClick={() => window.location.reload()}
        className="mt-5 inline-flex min-h-11 items-center justify-center rounded-sharp border-brutal border-black bg-accent-agent px-5 font-mono font-semibold text-black shadow-brutal-sm hover:shadow-brutal"
      >
        Try again
      </button>
    </div>
  );
}

function isWalletRecoveryPath(pathname: string) {
  return WALLET_RECOVERY_PATHS.has(pathname);
}

function requiresPortfolio(pathname: string) {
  return PORTFOLIO_REQUIRED_PREFIXES.some(
    (p) => pathname === p || pathname.startsWith(`${p}/`),
  );
}

function authHref(path: "/login", next: string, reason: string) {
  const params = new URLSearchParams();
  const safeNext = safeNextPath(next);
  if (safeNext) params.set("next", safeNext);
  params.set("reason", reason);
  const query = params.toString();
  return query ? `${path}?${query}` : path;
}
