"use client";

import { useEffect, useRef, useState } from "react";
import Link from "next/link";
import { useRouter, useSearchParams } from "next/navigation";
import {
  ArrowRight,
  CircleAlert,
  CheckCircle2,
  LayoutDashboard,
  Loader2,
  LogIn,
  LogOut,
  RotateCw,
  ShieldCheck,
  UserPlus,
} from "lucide-react";
import {
  BrutalButton,
  BrutalCard,
  BrutalCardBody,
  BrutalCardHeader,
  BrutalPill,
} from "@aegis/ui";
import {
  walletApi,
  setToken,
  analyticsApi,
  type UserTokenBundle,
} from "@/lib/api";
import { usePortfolioStore } from "@/stores/portfolio";

type Mode = "email" | "challenge" | "polling" | "done";
type Recovery = {
  method: "passkey" | "returning";
  redirectTo: "/dashboard" | "/onboarding";
};

interface Props {
  /** When true, the card calls `walletApi.login` and redirects to /dashboard
   * instead of /onboarding. Returning users don't need to re-set their PIN —
   * the W3S bundle has `challengeId = null` and the SDK ceremony is skipped. */
  loginMode?: boolean;
}

/**
 * Circle W3S User-Controlled wallet onboarding.
 *
 * 1. User enters email → POST `/auth/wallet/{create,login}` → server returns
 *    a `UserTokenBundle` (userToken + encryptionKey + appId + challengeId).
 * 2. Browser dynamically imports `@circle-fin/w3s-pw-web-sdk`, instantiates
 *    `W3SSdk`, calls `setAuthentication(...)` then `execute(challengeId)` to
 *    drive the PIN ceremony. The SDK signs the wallet creation request.
 * 3. We poll `/auth/wallet/status` every 2s until Circle has provisioned both
 *    ARC and BASE addresses, then redirect.
 *
 * Returning users (`isNewUser=false`) skip step 2 — the bundle has no
 * challengeId and the wallet is already on the response.
 */
export function CreateWalletCard({ loginMode = false }: Props) {
  const router = useRouter();
  const searchParams = useSearchParams();
  const referrerHandle = searchParams?.get("ref")?.trim().toLowerCase();
  const queryEmail = searchParams?.get("email")?.trim().toLowerCase();
  const setWallet = usePortfolioStore((s) => s.setWallet);
  const setSessionActive = usePortfolioStore((s) => s.setSessionActive);
  const resetSession = usePortfolioStore((s) => s.resetSession);

  const [email, setEmail] = useState("");
  const [mode, setMode] = useState<Mode>("email");
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [recovery, setRecovery] = useState<Recovery | null>(null);
  const [checkingSession, setCheckingSession] = useState(true);
  const [existingSession, setExistingSession] = useState<{
    email: string;
  } | null>(null);
  // Tracks whether the component is still mounted. The 30s `pollStatus`
  // loop would otherwise keep firing after the user navigated away, then
  // call setWallet + router.push on a stale closure (zombie poll).
  const mountedRef = useRef(true);
  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
    };
  }, []);

  // Pre-fill the email field from localStorage when we have it — the SPA
  // remembers the address from the last successful signup/login, but the
  // form starts blank otherwise and the user has no way to know which
  // address they registered (especially after the JWT cookie expires).
  useEffect(() => {
    if (typeof window === "undefined") return;
    const saved = queryEmail ?? window.localStorage.getItem("aegis_email");
    if (saved) setEmail(saved);
  }, [queryEmail]);

  useEffect(() => {
    let cancelled = false;
    walletApi
      .me()
      .then((user) => {
        if (cancelled) return;
        setExistingSession({ email: user.email });
        setSessionActive(true);
        localStorage.setItem("aegis_email", user.email);
      })
      .catch(() => {
        if (!cancelled) setExistingSession(null);
      })
      .finally(() => {
        if (!cancelled) setCheckingSession(false);
      });
    return () => {
      cancelled = true;
    };
  }, [setSessionActive]);

  const finish = async (
    method: "passkey" | "returning",
    wallet: {
      walletId: string;
      arcAddress: string;
      baseAddress: string;
      createdAt: string;
    },
    redirectTo: "/dashboard" | "/onboarding",
  ) => {
    setMode("done");
    setRecovery(null);
    setWallet(wallet);
    setSessionActive(true);
    localStorage.setItem("aegis_email", email.trim());
    await analyticsApi.track(loginMode ? "wallet.login" : "wallet.created", {
      method,
      referrerHandle: loginMode ? null : (referrerHandle ?? null),
    });
    router.replace(redirectTo);
  };

  const runChallengeAndPoll = async (
    bundle: UserTokenBundle,
    redirectTo: "/dashboard" | "/onboarding",
  ) => {
    const challengeId = bundle.challengeId;
    if (!challengeId) {
      // Returning user; wallet is either on the auth response or arrives via
      // a quick status poll. Caller already handled the inline-wallet case.
      setMode("polling");
      setRecovery({ method: "returning", redirectTo });
      await pollStatus("returning", redirectTo);
      return;
    }
    setMode("challenge");
    const { W3SSdk } = await import("@circle-fin/w3s-pw-web-sdk");
    const sdk = new W3SSdk({ appSettings: { appId: bundle.appId } });
    sdk.setAuthentication({
      userToken: bundle.userToken,
      encryptionKey: bundle.encryptionKey,
    });
    await new Promise<void>((resolve, reject) => {
      sdk.execute(challengeId, (sdkError, result) => {
        if (sdkError) {
          reject(new Error(sdkError.message || "Circle SDK challenge failed"));
          return;
        }
        // Result may be undefined on user-cancel; treat as error so the
        // surface explains what happened.
        if (!result) {
          reject(new Error("Challenge cancelled"));
          return;
        }
        resolve();
      });
    });
    setMode("polling");
    setRecovery({ method: "passkey", redirectTo });
    await pollStatus("passkey", redirectTo);
  };

  /**
   * Poll `/auth/wallet/status` until both ARC and BASE addresses come back.
   * Caps at ~30s (15 attempts × 2s) so a stuck Circle never traps the user.
   */
  const pollStatus = async (
    method: "passkey" | "returning",
    redirectTo: "/dashboard" | "/onboarding",
  ) => {
    for (let i = 0; i < 15; i++) {
      if (!mountedRef.current) return;
      const resp = await walletApi.status();
      if (!mountedRef.current) return;
      if (resp.wallet) {
        await finish(method, resp.wallet, redirectTo);
        return;
      }
      await new Promise((r) => setTimeout(r, 2000));
    }
    if (!mountedRef.current) return;
    throw new Error("Wallet provisioning timed out — refresh and try again");
  };

  const submitEmail = async () => {
    const normalizedEmail = email.trim().toLowerCase();
    if (!isValidEmail(normalizedEmail)) {
      setError("Enter a valid email address like name@example.com.");
      return;
    }
    setSubmitting(true);
    setError(null);
    setRecovery(null);
    try {
      const resp = loginMode
        ? await walletApi.login(normalizedEmail)
        : await walletApi.create(normalizedEmail, referrerHandle || undefined);
      setToken(resp.token);
      setSessionActive(true);
      setEmail(normalizedEmail);
      const redirectTo: "/dashboard" | "/onboarding" =
        loginMode || !resp.isNewUser ? "/dashboard" : "/onboarding";
      // If the user already had a wallet (login, or re-running signup with the
      // same email), the response carries it inline — skip the SDK ceremony.
      if (resp.wallet) {
        await finish(
          resp.isNewUser ? "passkey" : "returning",
          resp.wallet,
          redirectTo,
        );
        return;
      }
      await runChallengeAndPoll(resp.bundle, redirectTo);
    } catch (e) {
      setError(friendlyAuthError(e, loginMode));
      setMode("email");
    } finally {
      setSubmitting(false);
    }
  };

  const logoutForDifferentWallet = async () => {
    setSubmitting(true);
    setError(null);
    try {
      await walletApi.logout();
    } catch {
      /* already signed out */
    }
    resetSession();
    setExistingSession(null);
    setEmail("");
    setMode("email");
    setRecovery(null);
    setSubmitting(false);
  };

  const retryStatus = async () => {
    if (!recovery) return;
    setSubmitting(true);
    setError(null);
    setMode("polling");
    try {
      await pollStatus(recovery.method, recovery.redirectTo);
    } catch (e) {
      setError(friendlyAuthError(e, loginMode));
      setMode("email");
    } finally {
      if (mountedRef.current) setSubmitting(false);
    }
  };

  const normalizedEmail = email.trim().toLowerCase();
  const emailValid = isValidEmail(normalizedEmail);
  const switchHref = authSwitchHref(loginMode ? "/signup" : "/login", email);
  const emailHelpId = loginMode
    ? "wallet-login-email-help"
    : "wallet-signup-email-help";
  const emailInputId = loginMode ? "wallet-login-email" : "wallet-signup-email";
  const errorId = loginMode ? "wallet-login-error" : "wallet-signup-error";
  const primaryCta = loginMode
    ? "Restore wallet session"
    : "Start wallet setup";
  const Icon = loginMode ? LogIn : UserPlus;

  if (existingSession) {
    return (
      <BrutalCard className="max-w-md mx-auto">
        <BrutalCardHeader>
          <div className="flex flex-wrap items-center gap-2">
            <CheckCircle2 className="w-4 h-4 text-accent-agent" />
            <span className="text-sm font-semibold text-text-hi">
              Wallet session is active
            </span>
            <BrutalPill tone="agent">SIGNED IN</BrutalPill>
          </div>
        </BrutalCardHeader>
        <BrutalCardBody>
          <div className="space-y-4 font-mono">
            <div className="border border-accent-agent/30 bg-accent-agent/5 px-3 py-2">
              <p className="text-[10px] uppercase tracking-widest text-text-mut">
                Current account
              </p>
              <p className="mt-1 break-all text-sm text-text-hi">
                {existingSession.email}
              </p>
              <p className="mt-2 text-[11px] leading-relaxed text-text-lo">
                Continue to Dashboard, or log out before restoring a different
                wallet. Aegis will not create a second wallet for this session.
              </p>
            </div>
            <div className="grid gap-2 sm:grid-cols-2">
              <BrutalButton
                type="button"
                variant="agent"
                onClick={() => router.push("/dashboard")}
              >
                <LayoutDashboard className="h-4 w-4" />
                Dashboard
              </BrutalButton>
              <BrutalButton
                type="button"
                variant="ghost"
                disabled={submitting}
                onClick={() => void logoutForDifferentWallet()}
              >
                <LogOut className="h-4 w-4" />
                Use another email
              </BrutalButton>
            </div>
          </div>
        </BrutalCardBody>
      </BrutalCard>
    );
  }

  return (
    <BrutalCard className="max-w-md mx-auto">
      <BrutalCardHeader>
        <div className="flex flex-wrap items-center gap-2">
          {mode === "email" ? (
            <Icon className="w-4 h-4 text-accent-agent" />
          ) : (
            <Loader2 className="w-4 h-4 text-accent-agent animate-spin" />
          )}
          <span className="text-sm font-semibold text-text-hi">
            {mode === "email"
              ? loginMode
                ? "Restore your wallet"
                : "Create your wallet"
              : mode === "challenge"
                ? "Set your PIN"
                : mode === "polling"
                  ? "Restoring wallets…"
                  : "Opening Aegis…"}
          </span>
          <BrutalPill tone="agent">CIRCLE W3S</BrutalPill>
        </div>
      </BrutalCardHeader>
      <BrutalCardBody>
        <div
          className="mb-4 grid grid-cols-3 gap-2"
          aria-label="Wallet setup steps"
        >
          {[
            { label: "Email", active: mode === "email" },
            { label: "PIN", active: mode === "challenge" },
            { label: "Wallet", active: mode === "polling" || mode === "done" },
          ].map((step) => (
            <div
              key={step.label}
              className={`border border-border-default px-2 py-1.5 text-center text-[10px] font-mono uppercase tracking-widest ${
                step.active
                  ? "bg-accent-agent text-black"
                  : "bg-bg text-text-mut"
              }`}
            >
              {step.label}
            </div>
          ))}
        </div>

        {mode === "email" && (
          <form
            onSubmit={(e) => {
              e.preventDefault();
              if (!submitting && emailValid) void submitEmail();
            }}
          >
            <label
              htmlFor={emailInputId}
              className="block text-xs text-text-lo font-mono mb-2"
            >
              Email used for this wallet
            </label>
            <input
              id={emailInputId}
              data-testid="wallet-auth-email"
              value={email}
              onChange={(e) => setEmail(e.target.value)}
              type="email"
              inputMode="email"
              autoComplete="email"
              placeholder="you@example.com"
              aria-invalid={!!normalizedEmail && !emailValid}
              aria-describedby={`${emailHelpId}${error ? ` ${errorId}` : ""}`}
              className="w-full px-3 py-2 bg-bg border-brutal border-border-default focus:border-border-hi rounded-sharp font-mono text-sm text-text-hi outline-none"
            />
            <p
              id={emailHelpId}
              className="mt-2 text-[11px] text-text-mut font-mono leading-relaxed"
            >
              {loginMode
                ? "Use the same email from signup. Aegis restores the existing Circle wallet and portfolios; it will not create a duplicate."
                : "If this email already has an Aegis wallet, Aegis signs you in instead of creating a duplicate account."}
            </p>
            <div className="mt-3 border border-accent-agent/30 bg-accent-agent/5 px-3 py-2 text-[11px] font-mono text-text-lo leading-relaxed">
              {loginMode
                ? "Returning users skip portfolio setup and land on Dashboard."
                : "New users choose a Circle PIN, then set a portfolio goal before any deployment can happen."}
            </div>
            {!emailValid && normalizedEmail && (
              <p className="mt-2 text-[11px] font-mono text-warn">
                Use a complete email address, for example name@example.com.
              </p>
            )}
          </form>
        )}
        {mode === "challenge" && (
          <div
            className="space-y-3 text-xs text-text-lo font-mono"
            role="status"
            aria-live="polite"
          >
            <div className="flex items-start gap-2">
              <ShieldCheck className="w-4 h-4 text-accent-agent mt-0.5 shrink-0" />
              <p>
                Circle&apos;s PIN dialog opens in this tab. Pick a 6-digit PIN
                and keep the tab open until the wallet step starts.
              </p>
            </div>
            <p className="text-[11px] text-text-mut">
              Aegis never receives the PIN. The browser SDK completes the wallet
              ceremony and then Aegis checks for Arc + Base addresses.
            </p>
          </div>
        )}
        {mode === "polling" && (
          <div
            className="space-y-3 text-xs text-text-lo font-mono"
            role="status"
            aria-live="polite"
          >
            <div className="flex items-start gap-2">
              <Loader2 className="w-4 h-4 text-accent-agent animate-spin mt-0.5 shrink-0" />
              <p>
                Checking Circle Wallets every 2 seconds for Arc Testnet + Base
                Sepolia addresses. This usually finishes in under 30 seconds.
              </p>
            </div>
            <p className="text-[11px] text-text-mut">
              If the network stalls, use “Check wallet status again” below; it
              resumes the same session instead of creating a new wallet.
            </p>
          </div>
        )}

        {error && (
          <div
            id={errorId}
            role="alert"
            className="mt-3 flex items-start gap-2 border border-risk/40 bg-risk/5 px-3 py-2 text-xs text-risk font-mono"
          >
            <CircleAlert className="mt-0.5 h-3.5 w-3.5 shrink-0" />
            <span>{error}</span>
          </div>
        )}

        {mode === "email" && (
          <div className="mt-4 flex flex-col gap-2">
            <BrutalButton
              type="button"
              data-testid="wallet-auth-submit"
              variant="agent"
              className="w-full"
              onClick={() => void submitEmail()}
              disabled={!emailValid || submitting || checkingSession}
            >
              {submitting ? (
                <>
                  <Loader2 className="h-4 w-4 animate-spin" />
                  Checking…
                </>
              ) : (
                <>
                  {primaryCta}
                  <ArrowRight className="h-4 w-4" />
                </>
              )}
            </BrutalButton>
            <div className="flex flex-wrap items-center justify-between gap-2 text-[11px] font-mono">
              <Link
                data-testid="wallet-auth-switch"
                href={switchHref}
                className="text-accent-agent hover:underline"
              >
                {loginMode
                  ? "Create a new wallet"
                  : "Sign in to an existing wallet"}
              </Link>
              {recovery && (
                <button
                  type="button"
                  onClick={() => void retryStatus()}
                  className="inline-flex items-center gap-1 text-text-lo hover:text-accent-agent"
                >
                  <RotateCw className="h-3 w-3" />
                  Check wallet status again
                </button>
              )}
            </div>
          </div>
        )}

        {mode === "done" && (
          <div className="mt-2 flex items-center gap-2 text-xs text-accent-agent font-mono">
            <CheckCircle2 className="h-4 w-4" />
            Wallet session restored. Opening Aegis…
          </div>
        )}
      </BrutalCardBody>
    </BrutalCard>
  );
}

function authSwitchHref(path: "/login" | "/signup", email: string) {
  const normalized = email.trim().toLowerCase();
  if (!isValidEmail(normalized)) return path;
  return `${path}?email=${encodeURIComponent(normalized)}`;
}

function isValidEmail(email: string) {
  return /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(email) && email.length <= 254;
}

function friendlyAuthError(error: unknown, loginMode: boolean) {
  const raw = (error as Error).message || "Wallet setup failed";
  const message = raw.replace(/^\d{3}:\s*/, "");
  const lower = message.toLowerCase();
  if (lower.includes("no account for this email")) {
    return "No wallet uses this email yet. Create a wallet first, then use this page to sign back in.";
  }
  if (lower.includes("invalid email")) {
    return "Enter a valid email address without spaces.";
  }
  if (lower.includes("challenge cancelled")) {
    return "Circle wallet setup was cancelled. Start again with the same email when you are ready.";
  }
  if (lower.includes("wallet provisioning timed out")) {
    return "Circle has not returned both wallet addresses yet. Check wallet status again, or sign in with the same email in a minute.";
  }
  if (lower.includes("failed to fetch") || lower.includes("networkerror")) {
    return "Aegis could not reach the API. Check that the backend is running, then try again.";
  }
  if (lower.includes("circle") || lower.includes("w3s")) {
    return loginMode
      ? "Circle could not restore this wallet session. Try again, or create a wallet if this email has never completed setup."
      : "Circle could not finish wallet setup. Try again with the same email; Aegis will resume instead of creating a duplicate.";
  }
  return message;
}
