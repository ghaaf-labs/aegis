"use client";

import { useEffect, useState } from "react";
import Link from "next/link";
import { useRouter } from "next/navigation";
import {
  CircleAlert,
  Loader2,
  LogOut,
  Shield,
  WalletCards,
} from "lucide-react";
import { BrutalButton, BrutalPill } from "@aegis/ui";
import { GoalWizard } from "@/components/onboarding/goal-wizard";
import { walletApi } from "@/lib/api";
import { usePortfolioStore } from "@/stores/portfolio";

type OnboardingAuthState =
  | { kind: "checking" }
  | { kind: "signed_out" }
  | { kind: "wallet_pending"; email: string }
  | { kind: "ready"; email: string };

export default function OnboardingPage() {
  const router = useRouter();
  const resetSession = usePortfolioStore((s) => s.resetSession);
  const setSessionActive = usePortfolioStore((s) => s.setSessionActive);
  const setWallet = usePortfolioStore((s) => s.setWallet);
  const [authState, setAuthState] = useState<OnboardingAuthState>({
    kind: "checking",
  });
  const [logoutError, setLogoutError] = useState<string | null>(null);
  const [loggingOut, setLoggingOut] = useState(false);

  useEffect(() => {
    let cancelled = false;
    walletApi
      .me()
      .then(async (user) => {
        if (cancelled) return;
        setSessionActive(true);
        localStorage.setItem("aegis_email", user.email);
        const status = await walletApi.status().catch(() => ({ wallet: null }));
        if (cancelled) return;
        if (status.wallet) {
          setWallet(status.wallet);
          setAuthState({ kind: "ready", email: user.email });
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
  }, [resetSession, setSessionActive, setWallet]);

  const logout = async () => {
    setLoggingOut(true);
    setLogoutError(null);
    try {
      await walletApi.logout();
    } catch (e) {
      setLoggingOut(false);
      setLogoutError(logoutFailureMessage(e));
      return;
    }
    resetSession();
    router.replace("/login?signedOut=1");
  };

  return (
    <div className="min-h-screen bg-bg text-text-default flex items-start justify-center p-6 py-12">
      <div className="w-full max-w-2xl">
        <div className="mb-6 flex flex-wrap items-center justify-between gap-3">
          <div className="flex items-center gap-2">
            <div className="w-8 h-8 rounded-sharp bg-accent-agent flex items-center justify-center border-brutal border-black">
              <Shield className="w-4 h-4 text-black" />
            </div>
            <span className="font-semibold text-lg text-text-hi">Aegis</span>
          </div>
          {authState.kind === "ready" && (
            <div className="flex min-w-0 flex-wrap items-center gap-2">
              <BrutalPill tone="agent">SESSION VERIFIED</BrutalPill>
              <span className="max-w-[240px] truncate font-mono text-[11px] text-text-mut">
                {authState.email}
              </span>
              <button
                type="button"
                onClick={() => void logout()}
                disabled={loggingOut}
                className="inline-flex min-h-8 items-center justify-center gap-2 border border-border-default bg-bg px-2.5 font-mono text-[11px] text-text-lo hover:border-risk/50 hover:bg-risk/5 hover:text-risk disabled:opacity-50"
              >
                {loggingOut ? (
                  <Loader2 className="h-3.5 w-3.5 animate-spin" />
                ) : (
                  <LogOut className="h-3.5 w-3.5" />
                )}
                Log out
              </button>
            </div>
          )}
        </div>

        <div className="mb-8 text-center space-y-3">
          <h1 className="text-2xl font-semibold text-text-hi font-mono tracking-tight">
            Welcome to Aegis
          </h1>
          <p className="text-sm text-text-lo font-mono">
            Let&apos;s set your portfolio goal. The agent uses this every time
            it rebalances — you can update it later from Settings.
          </p>
          <p className="text-xs text-text-mut font-mono leading-relaxed">
            This creates a target plan only. It does not move funds; deployment
            and rebalance execution still require your approval.
          </p>
          <p className="text-xs font-mono">
            <span className="text-accent-agent">Set goal</span>
            <span className="text-text-mut mx-2">·</span>
            <span className="text-text-mut">Agent analyzes</span>
            <span className="text-text-mut mx-2">·</span>
            <span className="text-text-mut">You approve trades</span>
          </p>
        </div>

        {authState.kind === "checking" && <OnboardingChecking />}
        {authState.kind === "signed_out" && <OnboardingSignedOut />}
        {authState.kind === "wallet_pending" && (
          <OnboardingWalletPending
            email={authState.email}
            loggingOut={loggingOut}
            logoutError={logoutError}
            onLogout={() => void logout()}
          />
        )}
        {authState.kind === "ready" && (
          <>
            {logoutError && (
              <div
                role="alert"
                className="mb-4 border border-risk/40 bg-risk/5 px-3 py-2 font-mono text-[11px] text-risk"
              >
                {logoutError}
              </div>
            )}
            <GoalWizard />
          </>
        )}
      </div>
    </div>
  );
}

function OnboardingChecking() {
  return (
    <div
      role="status"
      className="mx-auto flex max-w-xl items-center gap-3 border-brutal border-border-default bg-raised p-5 font-mono text-sm text-text-lo"
    >
      <Loader2 className="h-4 w-4 animate-spin text-accent-agent" />
      Checking the server session before portfolio setup opens.
    </div>
  );
}

function OnboardingSignedOut() {
  return (
    <div className="mx-auto max-w-xl border-brutal border-warn/40 bg-raised p-5">
      <div className="mb-4 flex items-center gap-2">
        <CircleAlert className="h-4 w-4 text-warn" />
        <h2 className="font-mono text-base font-semibold text-text-hi">
          Sign in before portfolio setup
        </h2>
      </div>
      <p className="font-mono text-xs leading-relaxed text-text-lo">
        Aegis could not verify a server session for this browser. Portfolio
        setup stays closed until a one-time email code creates a fresh session.
      </p>
      <div className="mt-4 grid gap-2 sm:grid-cols-2">
        <Link
          href="/login?next=%2Fonboarding"
          className="inline-flex min-h-10 items-center justify-center border-brutal border-black bg-accent-agent px-4 font-mono text-sm font-semibold text-black shadow-brutal-sm hover:shadow-brutal"
        >
          Sign in
        </Link>
        <Link
          href="/signup?next=%2Fonboarding"
          className="inline-flex min-h-10 items-center justify-center border-brutal border-border-default px-4 font-mono text-sm text-text-lo hover:border-border-hi hover:text-text-hi"
        >
          Create wallet
        </Link>
      </div>
    </div>
  );
}

function OnboardingWalletPending({
  email,
  loggingOut,
  logoutError,
  onLogout,
}: {
  email: string;
  loggingOut: boolean;
  logoutError: string | null;
  onLogout: () => void;
}) {
  return (
    <div className="mx-auto max-w-xl border-brutal border-warn/40 bg-raised p-5">
      <div className="mb-4 flex items-center gap-2">
        <WalletCards className="h-4 w-4 text-warn" />
        <h2 className="font-mono text-base font-semibold text-text-hi">
          Finish wallet setup first
        </h2>
      </div>
      <p className="break-all font-mono text-sm text-text-hi">{email}</p>
      <p className="mt-2 font-mono text-xs leading-relaxed text-text-lo">
        This browser has a valid app session, but Aegis has not received real
        Arc + Base Circle wallet addresses yet. Goal setup stays paused until
        wallet recovery completes, so this screen cannot look like a fully
        logged-in account.
      </p>
      <div className="mt-4 grid gap-2 sm:grid-cols-2">
        <Link
          href={`/login?email=${encodeURIComponent(email)}&next=%2Fonboarding`}
          className="inline-flex min-h-10 items-center justify-center border-brutal border-black bg-accent-agent px-4 font-mono text-sm font-semibold text-black shadow-brutal-sm hover:shadow-brutal"
        >
          Resume wallet setup
        </Link>
        <BrutalButton
          type="button"
          variant="ghost"
          disabled={loggingOut}
          onClick={onLogout}
        >
          {loggingOut ? (
            <Loader2 className="h-4 w-4 animate-spin" />
          ) : (
            <LogOut className="h-4 w-4" />
          )}
          Log out
        </BrutalButton>
      </div>
      {logoutError && (
        <p role="alert" className="mt-3 font-mono text-[11px] text-risk">
          {logoutError}
        </p>
      )}
    </div>
  );
}

function logoutFailureMessage(error: unknown) {
  const message = (error as Error).message.toLowerCase();
  if (message.includes("still accepts")) {
    return "Logout was rejected because the server still accepts this browser session.";
  }
  if (message.includes("verification failed")) {
    return "Aegis could not verify sign-out with the API, so this session stays active.";
  }
  return "Logout did not reach the API. Your server session may still be active.";
}
