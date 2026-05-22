"use client";

import { useCallback, useEffect, useState } from "react";
import Link from "next/link";
import { useRouter } from "next/navigation";
import { CircleAlert, Loader2, LogOut, RotateCw, Shield } from "lucide-react";
import { BrutalButton } from "@aegis/ui";
import { logoutFailureMessage } from "@/components/layout/logout-copy";
import { GoalWizard } from "@/components/onboarding/goal-wizard";
import { walletApi, type WalletSessionResponse } from "@/lib/api";
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
  const setSessionResolved = usePortfolioStore((s) => s.setSessionResolved);
  const setWallet = usePortfolioStore((s) => s.setWallet);
  const [authState, setAuthState] = useState<OnboardingAuthState>({
    kind: "checking",
  });
  const [logoutError, setLogoutError] = useState<string | null>(null);
  const [setupError, setSetupError] = useState<string | null>(null);
  const [loggingOut, setLoggingOut] = useState(false);
  const [refreshingAccount, setRefreshingAccount] = useState(false);

  const applySession = useCallback(
    (session: WalletSessionResponse) => {
      setSessionActive(true);
      setSessionResolved(true);
      setSetupError(null);
      localStorage.setItem("aegis_email", session.user.email);
      if (session.wallet) {
        setWallet(session.wallet);
        setAuthState({ kind: "ready", email: session.user.email });
      } else {
        setWallet(null);
        setAuthState({ kind: "wallet_pending", email: session.user.email });
      }
    },
    [setSessionActive, setSessionResolved, setWallet],
  );

  useEffect(() => {
    let cancelled = false;
    walletApi
      .session()
      .then((session) => {
        if (cancelled) return;
        applySession(session);
      })
      .catch(() => {
        if (!cancelled) {
          resetSession();
          setSessionResolved(true);
          setAuthState({ kind: "signed_out" });
        }
      });
    return () => {
      cancelled = true;
    };
  }, [applySession, resetSession, setSessionResolved]);

  const refreshAccount = async () => {
    setRefreshingAccount(true);
    setSetupError(null);
    try {
      const session = await walletApi.session();
      applySession(session);
    } catch (e) {
      if (isSessionExpired(e)) {
        resetSession();
        setSessionResolved(true);
        setAuthState({ kind: "signed_out" });
      }
      setSetupError("We could not finish account setup. Try again.");
    } finally {
      setRefreshingAccount(false);
    }
  };

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
    <main className="relative min-h-screen overflow-hidden bg-bg text-text-default">
      <div className="absolute inset-0 bg-[linear-gradient(rgba(255,255,255,0.035)_1px,transparent_1px),linear-gradient(90deg,rgba(255,255,255,0.035)_1px,transparent_1px)] bg-[size:48px_48px]" />
      <section className="relative mx-auto flex min-h-screen w-full max-w-lg flex-col justify-center px-5 py-8">
        <Link href="/" className="mx-auto mb-8 inline-flex items-center gap-2">
          <div className="flex h-9 w-9 items-center justify-center rounded-sharp border-brutal border-black bg-accent-agent">
            <Shield className="h-4 w-4 text-black" />
          </div>
          <span className="text-lg font-semibold text-text-hi">Aegis</span>
        </Link>

        <div className="mb-5 space-y-2 text-center">
          <h1 className="font-mono text-3xl font-semibold text-text-hi sm:text-4xl">
            Create your portfolio
          </h1>
          <p className="mx-auto max-w-md font-mono text-sm leading-relaxed text-text-lo">
            Set the target the agent should follow. Nothing moves without your
            approval.
          </p>
        </div>

        {authState.kind === "checking" && <OnboardingChecking />}
        {authState.kind === "signed_out" && <OnboardingSignedOut />}
        {authState.kind === "wallet_pending" && (
          <OnboardingAccountPending
            email={authState.email}
            refreshing={refreshingAccount}
            setupError={setupError}
            loggingOut={loggingOut}
            logoutError={logoutError}
            onRetry={() => void refreshAccount()}
            onLogout={() => void logout()}
          />
        )}
        {authState.kind === "ready" && (
          <>
            <div className="mb-3 flex min-w-0 items-center justify-between gap-3 border border-border-default bg-surface px-3 py-2 font-mono">
              <span className="min-w-0 truncate text-[11px] text-text-mut">
                {formatSignedInEmail(authState.email)}
              </span>
              <button
                type="button"
                onClick={() => void logout()}
                disabled={loggingOut}
                className="inline-flex min-h-8 shrink-0 items-center justify-center gap-1.5 rounded-sharp border border-border-default bg-bg px-2 text-[11px] text-text-lo hover:border-risk/50 hover:bg-risk/5 hover:text-risk disabled:opacity-50"
              >
                {loggingOut ? (
                  <Loader2 className="h-3.5 w-3.5 animate-spin" />
                ) : (
                  <LogOut className="h-3.5 w-3.5" />
                )}
                Log out
              </button>
            </div>
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
      </section>
    </main>
  );
}

function OnboardingChecking() {
  return (
    <div
      role="status"
      className="flex items-center gap-3 border-brutal border-border-default bg-surface p-5 font-mono text-sm text-text-lo"
    >
      <Loader2 className="h-4 w-4 animate-spin text-accent-agent" />
      Opening portfolio setup.
    </div>
  );
}

function OnboardingSignedOut() {
  return (
    <div className="border-brutal border-warn/40 bg-surface p-5">
      <div className="mb-4 flex items-center gap-2">
        <CircleAlert className="h-4 w-4 text-warn" />
        <h2 className="font-mono text-base font-semibold text-text-hi">
          Continue before portfolio setup
        </h2>
      </div>
      <p className="font-mono text-xs leading-relaxed text-text-lo">
        Enter your email and verify a one-time code to continue.
      </p>
      <div className="mt-4 grid gap-2">
        <Link
          href="/login?next=%2Fonboarding"
          className="inline-flex min-h-11 items-center justify-center rounded-sharp border-brutal border-black bg-accent-agent px-4 font-mono text-sm font-semibold text-black shadow-brutal-sm hover:shadow-brutal"
        >
          Continue
        </Link>
      </div>
    </div>
  );
}

function OnboardingAccountPending({
  email,
  refreshing,
  setupError,
  loggingOut,
  logoutError,
  onRetry,
  onLogout,
}: {
  email: string;
  refreshing: boolean;
  setupError: string | null;
  loggingOut: boolean;
  logoutError: string | null;
  onRetry: () => void;
  onLogout: () => void;
}) {
  return (
    <div className="border-brutal border-warn/40 bg-surface p-5">
      <div className="mb-4 flex items-center gap-2">
        <Loader2 className="h-4 w-4 animate-spin text-warn" />
        <h2 className="font-mono text-base font-semibold text-text-hi">
          Setting up your account
        </h2>
      </div>
      <p className="break-all font-mono text-sm text-text-hi">{email}</p>
      <p className="mt-2 font-mono text-xs leading-relaxed text-text-lo">
        Portfolio setup opens as soon as this account is ready.
      </p>
      <div className="mt-4 grid gap-2 sm:grid-cols-2">
        <BrutalButton
          type="button"
          variant="agent"
          disabled={refreshing || loggingOut}
          onClick={onRetry}
        >
          {refreshing ? (
            <Loader2 className="h-4 w-4 animate-spin" />
          ) : (
            <RotateCw className="h-4 w-4" />
          )}
          Check again
        </BrutalButton>
        <BrutalButton
          type="button"
          variant="ghost"
          disabled={loggingOut || refreshing}
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
      {setupError && (
        <p role="alert" className="mt-3 font-mono text-[11px] text-risk">
          {setupError}
        </p>
      )}
      {logoutError && (
        <p role="alert" className="mt-3 font-mono text-[11px] text-risk">
          {logoutError}
        </p>
      )}
    </div>
  );
}

function formatSignedInEmail(email: string) {
  const at = email.indexOf("@");
  if (at <= 0) return email;
  const local = email.slice(0, at);
  const domain = email.slice(at + 1);
  if (local.length <= 20) return email;
  return `${local.slice(0, 8)}…@${domain}`;
}

function isSessionExpired(error: unknown) {
  const message = ((error as Error).message || "").toLowerCase();
  return (
    message.startsWith("401:") ||
    message.includes("session expired") ||
    message.includes("session_invalid") ||
    message.includes("unauthorized")
  );
}
