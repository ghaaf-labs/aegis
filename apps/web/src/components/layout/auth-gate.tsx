"use client";

import { useEffect, useState } from "react";
import { usePathname } from "next/navigation";
import Link from "next/link";
import { CircleAlert, Shield, ShieldCheck, WalletCards } from "lucide-react";
import { walletApi, type WalletAuthReadinessResponse } from "@/lib/api";
import { usePortfolioStore } from "@/stores/portfolio";

// Paths that are publicly viewable without a wallet.
const PUBLIC_PREFIXES = ["/leaderboard", "/explore", "/strategies", "/help"];
const WALLET_RECOVERY_PATHS = new Set(["/wallet", "/wallets", "/settings"]);

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
  const [authState, setAuthState] = useState<AuthState>({ kind: "checking" });
  const [readiness, setReadiness] =
    useState<WalletAuthReadinessResponse | null>(null);
  const [readinessChecked, setReadinessChecked] = useState(false);
  const setWallet = usePortfolioStore((s) => s.setWallet);
  const setSessionActive = usePortfolioStore((s) => s.setSessionActive);
  const sessionActive = usePortfolioStore((s) => s.sessionActive);
  const resetSession = usePortfolioStore((s) => s.resetSession);

  useEffect(() => {
    let cancelled = false;
    walletApi
      .readiness()
      .then((nextReadiness) => {
        if (!cancelled) setReadiness(nextReadiness);
      })
      .catch(() => {
        if (!cancelled) setReadiness(null);
      })
      .finally(() => {
        if (!cancelled) setReadinessChecked(true);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    let cancelled = false;
    setAuthState({ kind: "checking" });
    walletApi
      .me()
      .then(async (user) => {
        if (cancelled) return;
        localStorage.setItem("aegis_email", user.email);
        setSessionActive(true);
        const status = await walletApi.status().catch(() => ({ wallet: null }));
        if (cancelled) return;
        if (status.wallet) {
          setWallet(status.wallet);
          setAuthState({ kind: "ready" });
        } else {
          setWallet(null);
          setAuthState({ kind: "wallet_pending", email: user.email });
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

  // Public pages always show content.
  if (isPublic(pathname)) return <>{children}</>;

  // Avoid flashing the signed-out prompt until the server session check resolves.
  if (authState.kind === "checking") return null;

  if (authState.kind === "ready" && sessionActive) return <>{children}</>;

  if (
    authState.kind === "wallet_pending" &&
    sessionActive &&
    isWalletRecoveryPath(pathname)
  ) {
    return <>{children}</>;
  }

  if (authState.kind === "wallet_pending" && sessionActive) {
    return (
      <div className="mx-auto flex min-h-[60vh] max-w-[1400px] items-center justify-center">
        <div
          data-testid="wallet-pending-gate-message"
          className="w-full max-w-md space-y-4 border-brutal border-warn/40 bg-raised p-8 text-center"
        >
          <div className="flex justify-center">
            <div className="flex h-10 w-10 items-center justify-center rounded-sharp border-brutal border-black bg-warn">
              <WalletCards className="h-5 w-5 text-black" />
            </div>
          </div>
          <div>
            <h2 className="font-mono text-base font-semibold text-text-hi">
              Finish wallet setup
            </h2>
            <p className="mt-1 font-mono text-xs leading-relaxed text-text-lo">
              {authState.email} is signed in, but Aegis has not received real
              Arc + Base Circle wallet addresses yet. Portfolio actions stay
              blocked until wallet setup is complete.
            </p>
          </div>
          <div className="flex flex-col gap-2">
            <Link
              href={authHref("/login", pathname, authState.email)}
              className="inline-flex items-center justify-center rounded-sharp border-brutal border-black bg-accent-agent px-4 py-2 font-mono font-semibold text-black shadow-brutal-sm transition-shadow hover:shadow-brutal"
            >
              Resume wallet setup
            </Link>
            <Link
              href="/wallets"
              className="inline-flex items-center justify-center rounded-sharp border-brutal border-border-default px-4 py-2 font-mono text-sm text-text-lo transition-colors hover:border-border-hi hover:text-text-hi"
            >
              Open wallet page
            </Link>
          </div>
        </div>
      </div>
    );
  }

  const authLocked =
    (readinessChecked && !readiness) ||
    (!!readiness &&
      !readiness.emailDeliveryConfigured &&
      !readiness.devCodesEnabled);
  const authReadinessFailed = readinessChecked && !readiness;

  return (
    <div className="max-w-[1400px] mx-auto flex min-h-[60vh] items-center justify-center">
      <div
        data-testid="auth-gate-message"
        className="grid w-full max-w-4xl gap-0 overflow-hidden border-brutal border-border-default bg-raised lg:grid-cols-[1.05fr_0.95fr]"
      >
        <div className="space-y-5 p-6 md:p-8">
          <div className="flex flex-wrap items-center gap-2">
            <div
              className={`flex h-9 w-9 items-center justify-center rounded-sharp border-brutal border-black ${
                authLocked ? "bg-warn" : "bg-accent-agent"
              }`}
            >
              {authLocked ? (
                <CircleAlert className="h-4 w-4 text-black" />
              ) : (
                <Shield className="h-4 w-4 text-black" />
              )}
            </div>
            <span
              className={`border px-2 py-1 font-mono text-[10px] uppercase tracking-widest ${
                authLocked
                  ? "border-warn/50 bg-warn/10 text-warn"
                  : "border-accent-agent/40 bg-accent-agent/10 text-accent-agent"
              }`}
            >
              {authReadinessFailed
                ? "Auth check failed"
                : authLocked
                  ? "Real auth locked"
                  : "Session required"}
            </span>
          </div>
          <div>
            <h2 className="font-mono text-lg font-semibold text-text-hi">
              {authReadinessFailed
                ? "Login cannot verify this backend"
                : authLocked
                  ? "Login is waiting on email delivery"
                  : "Sign in to continue"}
            </h2>
            <p className="mt-2 max-w-xl font-mono text-xs leading-relaxed text-text-lo">
              {authReadinessFailed
                ? "Aegis could not confirm whether this backend can issue one-time codes. Protected pages stay closed instead of opening from stale browser state."
                : authLocked
                  ? "This real backend cannot send one-time codes right now. Aegis will not trust a remembered email, stale browser state, or old client token to open the dashboard."
                  : "This page needs a server-verified Aegis wallet session. Use the same email you registered with, then verify the one-time code before portfolio actions unlock."}
            </p>
          </div>
          {authLocked && !authReadinessFailed && (
            <div className="border border-warn/40 bg-warn/5 px-3 py-2 font-mono text-[11px] leading-relaxed text-warn">
              Set `RESEND_API_KEY` on the API to unlock real login/signup email
              codes. Mock dev codes stay off while real Circle mode is active.
            </div>
          )}
          {authReadinessFailed && (
            <div className="border border-risk/40 bg-risk/5 px-3 py-2 font-mono text-[11px] leading-relaxed text-risk">
              Start or reconnect the API, then reload this page. Until the
              readiness check succeeds, Aegis will not offer login or signup
              actions.
            </div>
          )}
          <div className="grid gap-2 sm:grid-cols-3">
            <AuthFact label="Email" value="not enough" />
            <AuthFact label="Code" value="required" />
            <AuthFact label="Session" value="server checked" />
          </div>
          <div className="flex flex-col gap-2 sm:flex-row">
            <Link
              href={authHref("/login", pathname)}
              className="inline-flex min-h-10 flex-1 items-center justify-center rounded-sharp border-brutal border-black bg-accent-agent px-4 py-2 font-mono font-semibold text-black shadow-brutal-sm transition-shadow hover:shadow-brutal"
            >
              {authLocked ? "Open sign-in status" : "Sign in"}
            </Link>
            <Link
              href={authHref("/signup", pathname)}
              className="inline-flex min-h-10 flex-1 items-center justify-center rounded-sharp border-brutal border-border-default px-4 py-2 font-mono text-sm text-text-lo transition-colors hover:border-border-hi hover:text-text-hi"
            >
              {authLocked ? "Open signup status" : "Create wallet"}
            </Link>
          </div>
        </div>
        <div className="border-t border-border-default bg-bg p-5 lg:border-l lg:border-t-0">
          <AuthFlowIllustration locked={authLocked} />
        </div>
      </div>
    </div>
  );
}

function AuthFact({ label, value }: { label: string; value: string }) {
  return (
    <div className="border border-border-default bg-bg px-3 py-2 font-mono">
      <p className="text-[10px] uppercase tracking-widest text-text-mut">
        {label}
      </p>
      <p className="mt-1 text-xs text-text-hi">{value}</p>
    </div>
  );
}

function AuthFlowIllustration({ locked }: { locked: boolean }) {
  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between gap-3">
        <div>
          <p className="font-mono text-[10px] uppercase tracking-widest text-text-mut">
            Wallet access path
          </p>
          <p className="mt-1 font-mono text-xs text-text-lo">
            Browser → email code → HttpOnly session → Arc + Base wallet
          </p>
        </div>
        <ShieldCheck
          className={`h-4 w-4 ${locked ? "text-warn" : "text-accent-agent"}`}
        />
      </div>
      <svg
        viewBox="0 0 440 250"
        role="img"
        aria-label={
          locked
            ? "Authentication flow blocked before email code delivery"
            : "Authentication flow from browser to wallet session"
        }
        className="h-auto w-full overflow-visible"
      >
        <defs>
          <linearGradient id="authFlowAgent" x1="0" x2="1" y1="0" y2="1">
            <stop offset="0%" stopColor="#67e8f9" />
            <stop offset="100%" stopColor="#22d3ee" stopOpacity="0.25" />
          </linearGradient>
          <linearGradient id="authFlowMoney" x1="0" x2="1" y1="0" y2="1">
            <stop offset="0%" stopColor="#86efac" />
            <stop offset="100%" stopColor="#4ade80" stopOpacity="0.2" />
          </linearGradient>
          <filter
            id="authFlowGlow"
            x="-30%"
            y="-30%"
            width="160%"
            height="160%"
          >
            <feGaussianBlur stdDeviation="3" result="blur" />
            <feMerge>
              <feMergeNode in="blur" />
              <feMergeNode in="SourceGraphic" />
            </feMerge>
          </filter>
        </defs>
        <rect
          x="1"
          y="1"
          width="438"
          height="248"
          fill="#0b0b0b"
          stroke="#2a2a2a"
          strokeWidth="2"
        />
        <g fill="none" stroke="#2a2a2a" strokeWidth="1">
          {Array.from({ length: 10 }).map((_, i) => (
            <path key={`h-${i}`} d={`M 0 ${25 + i * 22} H 440`} />
          ))}
          {Array.from({ length: 9 }).map((_, i) => (
            <path key={`v-${i}`} d={`M ${25 + i * 48} 0 V 250`} />
          ))}
        </g>
        <path
          d="M92 124 H175 H245 H330"
          fill="none"
          stroke={locked ? "#f59e0b" : "#67e8f9"}
          strokeDasharray="8 8"
          strokeWidth="3"
          filter="url(#authFlowGlow)"
        >
          <animate
            attributeName="stroke-dashoffset"
            dur="2.4s"
            from="32"
            repeatCount="indefinite"
            to="0"
          />
        </path>
        <FlowNode
          x={42}
          y={83}
          label="Browser"
          sublabel="no JS token"
          tone="agent"
        />
        <FlowNode
          x={151}
          y={83}
          label="Email code"
          sublabel={locked ? "sender missing" : "one-time"}
          tone={locked ? "warn" : "agent"}
        />
        <FlowNode
          x={260}
          y={83}
          label="Session"
          sublabel="HttpOnly"
          tone="agent"
        />
        <FlowNode
          x={330}
          y={164}
          label="Wallet"
          sublabel="Arc + Base"
          tone="pnl"
        />
        <path
          d="M306 146 L337 164"
          fill="none"
          stroke="#86efac"
          strokeDasharray="6 6"
          strokeWidth="3"
        >
          <animate
            attributeName="stroke-dashoffset"
            dur="2.8s"
            from="24"
            repeatCount="indefinite"
            to="0"
          />
        </path>
        {locked && (
          <g>
            <circle cx="203" cy="124" r="22" fill="#f59e0b" opacity="0.16" />
            <path
              d="M193 114 L213 134 M213 114 L193 134"
              stroke="#f59e0b"
              strokeLinecap="square"
              strokeWidth="5"
            />
          </g>
        )}
      </svg>
    </div>
  );
}

function FlowNode({
  x,
  y,
  label,
  sublabel,
  tone,
}: {
  x: number;
  y: number;
  label: string;
  sublabel: string;
  tone: "agent" | "pnl" | "warn";
}) {
  const fill =
    tone === "pnl"
      ? "url(#authFlowMoney)"
      : tone === "warn"
        ? "#2a1d06"
        : "url(#authFlowAgent)";
  const stroke =
    tone === "pnl" ? "#86efac" : tone === "warn" ? "#f59e0b" : "#67e8f9";
  return (
    <g>
      <rect
        x={x}
        y={y}
        width="86"
        height="74"
        fill={fill}
        stroke={stroke}
        strokeWidth="2"
      />
      <rect x={x + 10} y={y + 13} width="66" height="10" fill={stroke} />
      <text
        x={x + 43}
        y={y + 46}
        fill="#f5f5f5"
        fontFamily="monospace"
        fontSize="11"
        fontWeight="700"
        textAnchor="middle"
      >
        {label}
      </text>
      <text
        x={x + 43}
        y={y + 62}
        fill="#a3a3a3"
        fontFamily="monospace"
        fontSize="9"
        textAnchor="middle"
      >
        {sublabel}
      </text>
    </g>
  );
}

function isWalletRecoveryPath(pathname: string) {
  return WALLET_RECOVERY_PATHS.has(pathname);
}

function authHref(path: "/login" | "/signup", next: string, email?: string) {
  const params = new URLSearchParams();
  const safeNext = safeNextPath(next);
  if (safeNext) params.set("next", safeNext);
  if (email) params.set("email", email);
  const query = params.toString();
  return query ? `${path}?${query}` : path;
}

function safeNextPath(path: string | null | undefined) {
  if (!path || !path.startsWith("/") || path.startsWith("//")) return null;
  if (path.startsWith("/login") || path.startsWith("/signup")) return null;
  return path;
}
