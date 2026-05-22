"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import Link from "next/link";
import { useRouter, useSearchParams } from "next/navigation";
import {
  ArrowRight,
  CircleAlert,
  CheckCircle2,
  Loader2,
  LogIn,
  LogOut,
  RotateCw,
  ServerCog,
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
  analyticsApi,
  type UserTokenBundle,
  type WalletAuthReadinessResponse,
} from "@/lib/api";
import { usePortfolioStore } from "@/stores/portfolio";

type Mode = "email" | "verify" | "challenge" | "polling" | "done";
type Recovery = {
  method: "circle_pin" | "returning";
  redirectTo: string;
};
type ExistingSession = {
  email: string;
  walletState: "ready" | "pending" | "unknown";
};

interface Props {
  /** When true, the card verifies the email, calls `walletApi.login`, and
   * redirects to /dashboard instead of /onboarding. Returning users do not
   * re-set their PIN; the Aegis email code is the app-level login proof. */
  loginMode?: boolean;
}

/**
 * Circle W3S User-Controlled wallet onboarding.
 *
 * 1. User enters email → server sends a short-lived verification code.
 * 2. User enters the code → POST `/auth/wallet/{create,login}` → server sets
 *    the HttpOnly Aegis session cookie. It returns Circle credentials only
 *    when a wallet setup challenge must run in the browser.
 * 3. Browser dynamically imports `@circle-fin/w3s-pw-web-sdk`, instantiates
 *    `W3SSdk`, calls `setAuthentication(...)` then `execute(challengeId)` to
 *    drive the Circle PIN ceremony. The SDK signs the wallet creation request.
 * 4. We poll `/auth/wallet/status` every 2s until Circle has provisioned both
 *    ARC and BASE addresses, then redirect.
 *
 * Returning users (`isNewUser=false`) skip the Circle setup challenge — the
 * wallet is already on the verified auth response.
 */
export function CreateWalletCard({ loginMode = false }: Props) {
  const router = useRouter();
  const searchParams = useSearchParams();
  const referrerHandle = searchParams?.get("ref")?.trim().toLowerCase();
  const queryEmail = searchParams?.get("email")?.trim().toLowerCase();
  const nextPath = safeNextPath(searchParams?.get("next"));
  const signedOutFromQuery = searchParams?.get("signedOut") === "1";
  const redirectReason = authRedirectReason(searchParams?.get("reason"));
  const setWallet = usePortfolioStore((s) => s.setWallet);
  const setSessionActive = usePortfolioStore((s) => s.setSessionActive);
  const resetSession = usePortfolioStore((s) => s.resetSession);

  const [email, setEmail] = useState("");
  const [code, setCode] = useState("");
  const [codeChallenge, setCodeChallenge] = useState<{
    id: string;
    email: string;
    expiresAt: string;
    devCode?: string;
  } | null>(null);
  const [mode, setMode] = useState<Mode>("email");
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [recovery, setRecovery] = useState<Recovery | null>(null);
  const [manualSignedOut, setManualSignedOut] = useState(false);
  const [checkingSession, setCheckingSession] = useState(true);
  const [authReadiness, setAuthReadiness] =
    useState<WalletAuthReadinessResponse | null>(null);
  const [checkingReadiness, setCheckingReadiness] = useState(true);
  const [existingSession, setExistingSession] =
    useState<ExistingSession | null>(null);
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

  const refreshAuthReadiness = useCallback(async () => {
    setCheckingReadiness(true);
    setError(null);
    try {
      const readiness = await walletApi.readiness();
      if (!mountedRef.current) return;
      setAuthReadiness(readiness);
    } catch {
      if (!mountedRef.current) return;
      setAuthReadiness(null);
    } finally {
      if (mountedRef.current) setCheckingReadiness(false);
    }
  }, []);

  // Only explicit query params may pre-fill the auth form. Local remembered
  // email hints made a fresh login look like a stale session restore.
  useEffect(() => {
    if (typeof window === "undefined") return;
    if (
      signedOutFromQuery ||
      redirectReason === "session_expired" ||
      redirectReason === "session_check_failed"
    ) {
      window.localStorage.removeItem("aegis_email");
      setEmail(queryEmail ?? "");
      return;
    }
    setEmail(queryEmail ?? "");
  }, [queryEmail, redirectReason, signedOutFromQuery]);

  useEffect(() => {
    void refreshAuthReadiness();
  }, [refreshAuthReadiness]);

  useEffect(() => {
    let cancelled = false;
    walletApi
      .me()
      .then(async (user) => {
        if (cancelled) return;
        setSessionActive(true);
        localStorage.setItem("aegis_email", user.email);
        setEmail(user.email);
        setExistingSession({ email: user.email, walletState: "unknown" });
        const status = await walletApi.status().catch(() => null);
        if (cancelled) return;
        if (status?.wallet) {
          setExistingSession({ email: user.email, walletState: "ready" });
        } else if (status) {
          setExistingSession({ email: user.email, walletState: "pending" });
        } else {
          setExistingSession({ email: user.email, walletState: "unknown" });
        }
      })
      .catch(() => {
        if (!cancelled) {
          resetSession();
          setExistingSession(null);
        }
      })
      .finally(() => {
        if (!cancelled) setCheckingSession(false);
      });
    return () => {
      cancelled = true;
    };
  }, [resetSession, setSessionActive]);

  const finish = async (
    method: "circle_pin" | "returning",
    wallet: {
      walletId: string;
      arcAddress: string;
      baseAddress: string;
      createdAt: string;
    },
    redirectTo: string,
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
    redirectTo: string,
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
    setRecovery({ method: "circle_pin", redirectTo });
    await pollStatus("circle_pin", redirectTo);
  };

  /**
   * Poll `/auth/wallet/status` until both ARC and BASE addresses come back.
   * Caps at ~30s (15 attempts × 2s) so a stuck Circle never traps the user.
   */
  const pollStatus = async (
    method: "circle_pin" | "returning",
    redirectTo: string,
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

  const requestVerificationCode = async () => {
    const normalizedEmail = email.trim().toLowerCase();
    if (checkingSession) {
      setError(
        "Aegis is still checking whether this browser already has a server-accepted session.",
      );
      return;
    }
    if (existingSession) {
      setError(
        "This browser is already signed in. Log out first, then request a fresh one-time code.",
      );
      return;
    }
    if (checkingReadiness) {
      setError(
        "Aegis is still checking whether this backend can send real one-time codes.",
      );
      return;
    }
    if (authUnavailable) {
      setError(readinessUnavailableError(loginMode));
      return;
    }
    if (!isValidEmail(normalizedEmail)) {
      setError("Enter a valid email address like name@example.com.");
      return;
    }
    setSubmitting(true);
    setError(null);
    setRecovery(null);
    setManualSignedOut(false);
    try {
      const resp = await walletApi.requestCode(
        normalizedEmail,
        loginMode ? "login" : "signup",
        loginMode ? undefined : referrerHandle || undefined,
      );
      setEmail(resp.email);
      setCode("");
      setCodeChallenge({
        id: resp.challengeId,
        email: resp.email,
        expiresAt: resp.expiresAt,
        devCode: resp.devCode,
      });
      setMode("verify");
    } catch (e) {
      setError(friendlyAuthError(e, loginMode));
      setMode("email");
    } finally {
      setSubmitting(false);
    }
  };

  const submitVerificationCode = async () => {
    const normalizedEmail = email.trim().toLowerCase();
    const normalizedCode = code.trim();
    if (checkingSession) {
      setError(
        "Aegis is still checking this browser session. Wait for that check to finish before verifying a code.",
      );
      return;
    }
    if (existingSession) {
      setError(
        "This browser is already signed in. Log out first, then verify a fresh code.",
      );
      return;
    }
    if (!codeChallenge) {
      setError("Request a fresh verification code first.");
      setMode("email");
      return;
    }
    if (!/^\d{6}$/.test(normalizedCode)) {
      setError("Enter the 6-digit verification code.");
      return;
    }
    setSubmitting(true);
    setError(null);
    setRecovery(null);
    try {
      const resp = loginMode
        ? await walletApi.login(
            normalizedEmail,
            codeChallenge.id,
            normalizedCode,
          )
        : await walletApi.create(
            normalizedEmail,
            codeChallenge.id,
            normalizedCode,
            referrerHandle || undefined,
          );
      setSessionActive(true);
      setEmail(normalizedEmail);
      setCodeChallenge(null);
      setCode("");
      const redirectTo =
        loginMode || !resp.isNewUser
          ? (nextPath ?? "/dashboard")
          : "/onboarding";
      // If the user already had a wallet (login, or re-running signup with the
      // same email), the response carries it inline — skip the SDK ceremony.
      if (resp.wallet) {
        await finish(
          resp.isNewUser ? "circle_pin" : "returning",
          resp.wallet,
          redirectTo,
        );
        return;
      }
      if (!resp.bundle) {
        throw new Error(
          "Circle challenge credentials were not returned for this wallet setup step.",
        );
      }
      await runChallengeAndPoll(resp.bundle, redirectTo);
    } catch (e) {
      setError(friendlyAuthError(e, loginMode));
      setMode(isCorrectableCodeError(e) ? "verify" : "email");
    } finally {
      setSubmitting(false);
    }
  };

  const logoutForDifferentWallet = async () => {
    setSubmitting(true);
    setError(null);
    try {
      await walletApi.logout();
    } catch (e) {
      setError(logoutFailureMessage(e));
      setSubmitting(false);
      return;
    }
    resetSession();
    setExistingSession(null);
    setEmail("");
    setCode("");
    setCodeChallenge(null);
    setMode("email");
    setRecovery(null);
    setManualSignedOut(true);
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
  const codeValid = /^\d{6}$/.test(code.trim());
  const authUnavailable =
    !authReadiness ||
    (!authReadiness.emailDeliveryConfigured && !authReadiness.devCodesEnabled);
  const authReadinessFailed = !checkingReadiness && !authReadiness;
  const authFormBlocked =
    mode === "email" &&
    !checkingReadiness &&
    (authReadinessFailed || authUnavailable);
  const switchHref = authSwitchHref(
    loginMode ? "/signup" : "/login",
    email,
    nextPath,
  );
  const emailHelpId = loginMode
    ? "wallet-login-email-help"
    : "wallet-signup-email-help";
  const emailInputId = loginMode ? "wallet-login-email" : "wallet-signup-email";
  const errorId = loginMode ? "wallet-login-error" : "wallet-signup-error";
  const primaryCta = checkingReadiness
    ? "Checking auth status"
    : authReadinessFailed
      ? "Auth check unavailable"
      : loginMode
        ? authUnavailable
          ? "Login email unavailable"
          : "Send one-time login code"
        : authUnavailable
          ? "Signup email unavailable"
          : "Send signup code";
  const verifyCta = loginMode
    ? "Verify code and restore"
    : "Verify and continue";
  const Icon = loginMode ? LogIn : UserPlus;
  const showSignupRecovery =
    loginMode &&
    mode === "email" &&
    emailValid &&
    error?.startsWith("No wallet uses this email");
  const showLoginRecovery =
    !loginMode &&
    mode === "email" &&
    emailValid &&
    error?.startsWith("This email already has a wallet");
  const showSignedOutNotice =
    (signedOutFromQuery || manualSignedOut) &&
    mode === "email" &&
    !existingSession;
  const redirectNotice = redirectReason
    ? authRedirectNotice(redirectReason)
    : null;
  const setupSteps = loginMode
    ? [
        { label: "Email", active: mode === "email" },
        { label: "Code", active: mode === "verify" },
        { label: "Session", active: mode === "polling" || mode === "done" },
        { label: "Wallet", active: mode === "polling" || mode === "done" },
      ]
    : [
        { label: "Email", active: mode === "email" },
        { label: "Code", active: mode === "verify" },
        { label: "PIN", active: mode === "challenge" },
        { label: "Wallet", active: mode === "polling" || mode === "done" },
      ];

  if (signedOutFromQuery && existingSession) {
    return (
      <BrutalCard className="w-full border-warn/50">
        <BrutalCardHeader>
          <div className="flex flex-wrap items-center gap-2">
            <CircleAlert className="w-4 h-4 text-warn" />
            <span className="text-sm font-semibold text-text-hi">
              Logout did not finish
            </span>
            <BrutalPill tone="warn">SESSION STILL ACTIVE</BrutalPill>
          </div>
        </BrutalCardHeader>
        <BrutalCardBody>
          <div className="space-y-4 font-mono">
            <div className="border border-warn/40 bg-warn/5 px-3 py-2">
              <p className="text-[10px] uppercase tracking-widest text-warn">
                Server still accepts this browser
              </p>
              <p className="mt-1 break-all text-sm text-text-hi">
                {existingSession.email}
              </p>
              <p className="mt-2 text-[11px] leading-relaxed text-text-lo">
                This login page was opened after a sign-out redirect, but the
                backend still reports a valid session cookie. Do not treat this
                as a fresh login. Retry logout, then request a new one-time
                code.
              </p>
            </div>
            <div className="grid gap-2">
              <BrutalButton
                type="button"
                variant="agent"
                disabled={submitting}
                onClick={() => void logoutForDifferentWallet()}
              >
                <LogOut className="h-4 w-4" />
                Retry server logout
              </BrutalButton>
            </div>
            {error && (
              <p role="alert" className="text-[11px] text-risk">
                {error}
              </p>
            )}
          </div>
        </BrutalCardBody>
      </BrutalCard>
    );
  }

  if (existingSession) {
    const activeSessionCopy = activeSessionCopyFor(
      loginMode,
      existingSession.walletState,
    );
    const activeSessionInstruction = activeSessionInstructionFor(
      loginMode,
      existingSession.walletState,
    );
    const activeSessionTitle = activeSessionTitleFor(
      loginMode,
      existingSession.walletState,
    );
    return (
      <BrutalCard className="w-full">
        <BrutalCardHeader>
          <div className="flex flex-wrap items-center gap-2">
            <CheckCircle2 className="w-4 h-4 text-accent-agent" />
            <span className="text-sm font-semibold text-text-hi">
              {activeSessionTitle}
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
                {activeSessionCopy}
              </p>
              <p className="mt-2 text-[11px] leading-relaxed text-warn">
                {activeSessionInstruction}
              </p>
            </div>
            {loginMode && (
              <div className="grid gap-2 sm:grid-cols-4">
                <AuthStateFact label="Login action" value="not run" />
                <AuthStateFact label="Cookie" value="server valid" />
                <AuthStateFact label="Fresh code" value="requires logout" />
                <AuthStateFact
                  label="Wallet status"
                  value={walletStateLabel(existingSession.walletState)}
                />
              </div>
            )}
            <div className="grid gap-2">
              <BrutalButton
                type="button"
                variant="agent"
                disabled={submitting}
                onClick={() => void logoutForDifferentWallet()}
              >
                <LogOut className="h-4 w-4" />
                {loginMode ? "Log out and re-authenticate" : "Log out first"}
              </BrutalButton>
            </div>
            {error && (
              <p role="alert" className="text-[11px] text-risk">
                {error}
              </p>
            )}
          </div>
        </BrutalCardBody>
      </BrutalCard>
    );
  }

  if (checkingSession) {
    return (
      <BrutalCard className="w-full">
        <BrutalCardHeader>
          <div className="flex flex-wrap items-center gap-2">
            <Loader2 className="w-4 h-4 text-accent-agent animate-spin" />
            <span className="text-sm font-semibold text-text-hi">
              Checking current session
            </span>
            <BrutalPill tone="agent">SERVER CHECK</BrutalPill>
          </div>
        </BrutalCardHeader>
        <BrutalCardBody>
          <div className="space-y-4 font-mono">
            <div className="border border-accent-agent/30 bg-accent-agent/5 px-3 py-2">
              <p className="text-[10px] uppercase tracking-widest text-accent-agent">
                No code request yet
              </p>
              <p className="mt-1 text-[11px] leading-relaxed text-text-lo">
                Aegis is asking the API whether this browser still has an
                accepted HttpOnly session. The email form stays closed until the
                server rejects the old session or confirms it is active.
              </p>
            </div>
            <div className="grid gap-2 sm:grid-cols-3">
              <AuthStateFact label="Cookie" value="checking" />
              <AuthStateFact label="Email code" value="locked" />
              <AuthStateFact label="Session" value="server first" />
            </div>
          </div>
        </BrutalCardBody>
      </BrutalCard>
    );
  }

  return (
    <BrutalCard className="w-full">
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
              : mode === "verify"
                ? "Verify email"
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
          className="mb-4 grid grid-cols-4 gap-2"
          aria-label="Wallet setup steps"
        >
          {setupSteps.map((step) => (
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

        {showSignedOutNotice && (
          <div className="mb-4 border border-accent-agent/40 bg-accent-agent/5 px-3 py-2 font-mono text-[11px] leading-relaxed text-text-lo">
            <p className="text-[10px] uppercase tracking-widest text-accent-agent">
              Signed out
            </p>
            <p className="mt-1">
              The server session was cleared and remembered login hints were
              removed from this browser. Enter the email again; Aegis will not
              open the wallet until a fresh one-time code is verified.
            </p>
          </div>
        )}

        {!showSignedOutNotice && redirectNotice && mode === "email" && (
          <div className="mb-4 border border-warn/40 bg-warn/5 px-3 py-2 font-mono text-[11px] leading-relaxed text-warn">
            <p className="text-[10px] uppercase tracking-widest">
              {redirectNotice.title}
            </p>
            <p className="mt-1 text-text-lo">{redirectNotice.body}</p>
          </div>
        )}

        {mode === "email" && nextPath && (
          <div className="mb-4 border border-accent-agent/30 bg-accent-agent/5 px-3 py-2 font-mono text-[11px] leading-relaxed text-text-lo">
            <p className="text-[10px] uppercase tracking-widest text-accent-agent">
              Return after sign-in
            </p>
            <p className="mt-1">
              After verification, Aegis will open {humanizeNextPath(nextPath)}.
            </p>
          </div>
        )}

        {mode === "email" && (checkingReadiness || authReadiness) && (
          <div
            className={`mb-4 border px-3 py-2 font-mono text-[11px] leading-relaxed ${
              checkingReadiness
                ? "border-border-default bg-bg text-text-lo"
                : authUnavailable
                  ? "border-risk/40 bg-risk/5 text-risk"
                  : authReadiness?.devCodesEnabled
                    ? "border-warn/40 bg-warn/5 text-warn"
                    : "border-accent-agent/30 bg-accent-agent/5 text-text-lo"
            }`}
          >
            {checkingReadiness ? (
              <>
                <p className="text-[10px] uppercase tracking-widest">
                  Auth environment
                </p>
                <p className="mt-1">
                  Checking the backend before enabling the auth form. Aegis will
                  not send or accept codes until the server capability is known.
                </p>
              </>
            ) : authUnavailable ? (
              <AuthUnavailablePanel
                loginMode={loginMode}
                readiness={authReadiness!}
                refreshing={checkingReadiness}
                onRefresh={() => void refreshAuthReadiness()}
              />
            ) : (
              <>
                <p className="text-[10px] uppercase tracking-widest">
                  Auth environment
                </p>
                <p className="mt-1">
                  {readinessCopy(authReadiness!, loginMode)}
                </p>
              </>
            )}
          </div>
        )}

        {mode === "email" && authReadinessFailed && (
          <div className="mb-4 space-y-3 border border-risk/40 bg-risk/5 px-3 py-2 font-mono text-[11px] leading-relaxed text-risk">
            <p className="text-[10px] uppercase tracking-widest">
              Auth check failed
            </p>
            <p className="mt-1 text-text-lo">
              Aegis could not verify whether this backend can issue one-time
              codes. Login and signup stay locked instead of trusting stale
              browser state.
            </p>
            <button
              type="button"
              onClick={() => void refreshAuthReadiness()}
              disabled={checkingReadiness}
              className="inline-flex min-h-9 w-full items-center justify-center gap-2 rounded-sharp border border-risk/40 bg-risk/10 px-3 text-[11px] font-semibold text-risk hover:bg-risk/15 disabled:cursor-not-allowed disabled:opacity-50"
            >
              {checkingReadiness ? (
                <Loader2 className="h-3.5 w-3.5 animate-spin" />
              ) : (
                <RotateCw className="h-3.5 w-3.5" />
              )}
              Recheck backend auth capability
            </button>
          </div>
        )}

        {mode === "email" && authFormBlocked && (
          <div
            data-testid="wallet-auth-form-blocked"
            className="border border-border-default bg-bg px-3 py-3 font-mono text-[11px] leading-relaxed text-text-lo"
          >
            <p className="text-[10px] uppercase tracking-widest text-text-mut">
              Email form locked
            </p>
            <p className="mt-1">
              {loginMode
                ? "The login form is hidden until the backend can deliver a real one-time code. No remembered email, stale cookie, or local browser state can restore a session from here."
                : "The signup form is hidden until the backend can deliver a real one-time code. Aegis will not create a wallet from email text alone."}
            </p>
            <Link
              data-testid="wallet-auth-switch"
              href={switchHref}
              className="mt-3 inline-flex items-center gap-2 text-accent-agent hover:underline"
            >
              {loginMode ? "Check signup status" : "Check sign-in status"}
              <ArrowRight className="h-3.5 w-3.5" />
            </Link>
          </div>
        )}

        {mode === "email" && !authFormBlocked && (
          <form
            onSubmit={(e) => {
              e.preventDefault();
              if (
                !submitting &&
                !checkingSession &&
                emailValid &&
                !authUnavailable
              ) {
                void requestVerificationCode();
              }
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
                ? "Use the same email from signup. A fresh one-time code is required after logout or expiry; stale browser hints are not used to fill this field."
                : "Aegis verifies this email before creating or restoring any wallet-backed account."}
            </p>
            <div className="mt-3 border border-accent-agent/30 bg-accent-agent/5 px-3 py-2 text-[11px] font-mono text-text-lo leading-relaxed">
              {loginMode
                ? "Knowing an email is not enough to sign in. The code proves you control the inbox."
                : "After email verification, new users choose a Circle PIN, then set a portfolio goal before any deployment can happen."}
            </div>
            {!emailValid && normalizedEmail && (
              <p className="mt-2 text-[11px] font-mono text-warn">
                Use a complete email address, for example name@example.com.
              </p>
            )}
          </form>
        )}
        {mode === "verify" && codeChallenge && (
          <form
            onSubmit={(e) => {
              e.preventDefault();
              if (!submitting && codeValid) void submitVerificationCode();
            }}
            className="space-y-3"
          >
            <div className="border border-accent-agent/30 bg-accent-agent/5 px-3 py-2 font-mono">
              <p className="text-[10px] uppercase tracking-widest text-text-mut">
                Code sent to
              </p>
              <p className="mt-1 break-all text-sm text-text-hi">
                {codeChallenge.email}
              </p>
              <p className="mt-2 text-[11px] leading-relaxed text-text-lo">
                It expires at {formatCodeExpiry(codeChallenge.expiresAt)}.
              </p>
            </div>
            {codeChallenge.devCode && (
              <div className="border border-warn/40 bg-warn/5 px-3 py-2 font-mono text-[11px] text-warn">
                Mock dev code:{" "}
                <span className="text-sm font-semibold tracking-widest">
                  {codeChallenge.devCode}
                </span>
              </div>
            )}
            <label
              htmlFor={`${emailInputId}-code`}
              className="block text-xs text-text-lo font-mono"
            >
              6-digit verification code
            </label>
            <input
              id={`${emailInputId}-code`}
              data-testid="wallet-auth-code"
              value={code}
              onChange={(e) =>
                setCode(e.target.value.replace(/\D/g, "").slice(0, 6))
              }
              type="text"
              inputMode="numeric"
              autoComplete="one-time-code"
              placeholder="123456"
              aria-invalid={!!code && !codeValid}
              aria-describedby={error ? errorId : undefined}
              className="w-full px-3 py-2 bg-bg border-brutal border-border-default focus:border-border-hi rounded-sharp font-mono text-sm text-text-hi outline-none tracking-[0.3em]"
            />
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
            className="mt-3 space-y-3 border border-risk/40 bg-risk/5 px-3 py-2 text-xs text-risk font-mono"
          >
            <div className="flex items-start gap-2">
              <CircleAlert className="mt-0.5 h-3.5 w-3.5 shrink-0" />
              <span>{error}</span>
            </div>
            {showSignupRecovery && (
              <Link
                href={authSwitchHref("/signup", email, nextPath)}
                className="inline-flex w-full items-center justify-center gap-2 rounded-sharp border border-risk/40 bg-risk/10 px-3 py-2 text-center text-[11px] font-semibold text-risk hover:bg-risk/15"
              >
                Create wallet with this email
                <ArrowRight className="h-3.5 w-3.5" />
              </Link>
            )}
            {showLoginRecovery && (
              <Link
                href={authSwitchHref("/login", email, nextPath)}
                className="inline-flex w-full items-center justify-center gap-2 rounded-sharp border border-risk/40 bg-risk/10 px-3 py-2 text-center text-[11px] font-semibold text-risk hover:bg-risk/15"
              >
                Sign in with this email
                <ArrowRight className="h-3.5 w-3.5" />
              </Link>
            )}
          </div>
        )}

        {(mode === "verify" || (mode === "email" && !authFormBlocked)) && (
          <div className="mt-4 flex flex-col gap-2">
            <BrutalButton
              type="button"
              data-testid="wallet-auth-submit"
              variant="agent"
              className="w-full"
              onClick={() =>
                void (mode === "email"
                  ? requestVerificationCode()
                  : submitVerificationCode())
              }
              disabled={
                submitting ||
                checkingSession ||
                checkingReadiness ||
                authUnavailable ||
                (mode === "email" ? !emailValid : !codeValid)
              }
            >
              {submitting ? (
                <>
                  <Loader2 className="h-4 w-4 animate-spin" />
                  {mode === "email" ? "Sending…" : "Verifying…"}
                </>
              ) : (
                <>
                  {mode === "email" ? primaryCta : verifyCta}
                  <ArrowRight className="h-4 w-4" />
                </>
              )}
            </BrutalButton>
            <div className="flex flex-wrap items-center justify-between gap-2 text-[11px] font-mono">
              {mode === "email" ? (
                <Link
                  data-testid="wallet-auth-switch"
                  href={switchHref}
                  className="text-accent-agent hover:underline"
                >
                  {loginMode
                    ? "Create a new wallet"
                    : "Sign in to an existing wallet"}
                </Link>
              ) : (
                <button
                  type="button"
                  className="text-accent-agent hover:underline"
                  onClick={() => {
                    setMode("email");
                    setCode("");
                    setCodeChallenge(null);
                    setError(null);
                  }}
                >
                  Change email
                </button>
              )}
              {mode === "verify" && (
                <button
                  type="button"
                  disabled={submitting}
                  onClick={() => void requestVerificationCode()}
                  className="inline-flex items-center gap-1 text-text-lo hover:text-accent-agent disabled:opacity-50"
                >
                  <RotateCw className="h-3 w-3" />
                  Send a new code
                </button>
              )}
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

function AuthStateFact({ label, value }: { label: string; value: string }) {
  return (
    <div className="border border-border-default bg-bg px-2 py-2">
      <p className="text-[9px] uppercase tracking-widest text-text-mut">
        {label}
      </p>
      <p className="mt-1 text-[11px] text-text-hi">{value}</p>
    </div>
  );
}

function activeSessionTitleFor(
  loginMode: boolean,
  walletState: ExistingSession["walletState"],
) {
  if (loginMode) {
    return walletState === "ready"
      ? "Already signed in - login form locked"
      : "Session active - login form locked";
  }
  return walletState === "ready"
    ? "Active wallet session found"
    : "Active session found";
}

function activeSessionCopyFor(
  loginMode: boolean,
  walletState: ExistingSession["walletState"],
) {
  if (walletState === "ready") {
    return loginMode
      ? "Aegis checked the server cookie and found an already-valid session. No login happened on this screen. To prove inbox control again, log out here and request a fresh one-time code."
      : "Aegis checked the server cookie and found an already-valid session. Signup is blocked while this browser is signed in; log out first if you want to use a different email.";
  }
  if (walletState === "pending") {
    return "Aegis checked the server cookie and found a valid app session, but Arc + Base wallet addresses are not attached yet. This is still an active session, so the auth form stays locked instead of looking like a fresh login.";
  }
  return "Aegis checked the server cookie and found a valid app session, but wallet status could not be verified. This is still treated as signed in until logout is confirmed by the backend.";
}

function activeSessionInstructionFor(
  loginMode: boolean,
  walletState: ExistingSession["walletState"],
) {
  if (walletState !== "ready") {
    return "Log out and request a fresh one-time code if you want to restart wallet recovery. Aegis will not issue another code while this session is still accepted.";
  }
  return loginMode
    ? "This auth form will not open the app from an existing cookie. Log out, then verify a new one-time code."
    : "This signup form will not create or open another wallet while an existing session is active.";
}

function walletStateLabel(walletState: ExistingSession["walletState"]) {
  if (walletState === "ready") return "wallet ready";
  if (walletState === "pending") return "setup pending";
  return "unknown";
}

function AuthUnavailablePanel({
  loginMode,
  readiness,
  refreshing,
  onRefresh,
}: {
  loginMode: boolean;
  readiness: WalletAuthReadinessResponse;
  refreshing: boolean;
  onRefresh: () => void;
}) {
  return (
    <div className="space-y-3">
      <div className="flex items-start gap-2">
        <ServerCog className="mt-0.5 h-4 w-4 shrink-0" />
        <div>
          <p className="text-[10px] uppercase tracking-widest">
            Real auth is blocked
          </p>
          <p className="mt-1 text-text-lo">
            {readinessCopy(readiness, loginMode)} This is a backend capability
            problem, not a wrong email or a hidden browser session.
          </p>
        </div>
      </div>

      <div className="grid gap-2 sm:grid-cols-3">
        <AuthStateFact
          label="Circle mode"
          value={readiness.circleMock ? "mock wallet" : "real Circle"}
        />
        <AuthStateFact
          label="Email sender"
          value={readiness.emailDeliveryConfigured ? "ready" : "missing"}
        />
        <AuthStateFact
          label="Local codes"
          value={readiness.devCodesEnabled ? "enabled" : "off"}
        />
      </div>

      <div className="border border-risk/30 bg-bg px-3 py-2 text-text-lo">
        <p>
          To unlock real {loginMode ? "login" : "signup"}, set{" "}
          <code className="text-text-hi">RESEND_API_KEY</code> on the API,
          restart the backend, then recheck this screen.
        </p>
      </div>

      <button
        type="button"
        onClick={onRefresh}
        disabled={refreshing}
        className="inline-flex min-h-9 w-full items-center justify-center gap-2 rounded-sharp border border-risk/40 bg-risk/10 px-3 text-[11px] font-semibold text-risk hover:bg-risk/15 disabled:cursor-not-allowed disabled:opacity-50"
      >
        {refreshing ? (
          <Loader2 className="h-3.5 w-3.5 animate-spin" />
        ) : (
          <RotateCw className="h-3.5 w-3.5" />
        )}
        Recheck backend auth capability
      </button>
    </div>
  );
}

function authSwitchHref(
  path: "/login" | "/signup",
  email: string,
  nextPath: string | null,
) {
  const normalized = email.trim().toLowerCase();
  const params = new URLSearchParams();
  if (isValidEmail(normalized)) params.set("email", normalized);
  if (nextPath) params.set("next", nextPath);
  const query = params.toString();
  return query ? `${path}?${query}` : path;
}

function safeNextPath(path: string | null | undefined) {
  if (!path || !path.startsWith("/") || path.startsWith("//")) return null;
  if (path.startsWith("/login") || path.startsWith("/signup")) return null;
  return path;
}

function humanizeNextPath(path: string) {
  const clean = path.split("?")[0]?.replace(/^\/+/, "") || "the dashboard";
  return clean
    .split("/")
    .filter(Boolean)
    .map((part) => part.replace(/-/g, " "))
    .join(" / ");
}

function isValidEmail(email: string) {
  return /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(email) && email.length <= 254;
}

function formatCodeExpiry(expiresAt: string) {
  const date = new Date(expiresAt);
  if (Number.isNaN(date.getTime())) return "soon";
  return date.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
}

function readinessCopy(
  readiness: WalletAuthReadinessResponse,
  loginMode: boolean,
) {
  if (!readiness.emailDeliveryConfigured && !readiness.devCodesEnabled) {
    return loginMode
      ? "Real Circle login is locked because this backend has no email sender configured. Aegis will not issue a session or show a local code."
      : "Real Circle signup is locked because this backend has no email sender configured. Aegis will not create a wallet or show a local code.";
  }
  if (readiness.devCodesEnabled) {
    return "Local mock wallet auth is active. The next step will show a mock dev code in this browser.";
  }
  return loginMode
    ? "Aegis will email a one-time login code. The browser cannot restore the wallet from email alone."
    : "Aegis will email a one-time signup code before the Circle wallet PIN step starts.";
}

function readinessUnavailableError(loginMode: boolean) {
  return loginMode
    ? "Email delivery is not configured for real login. Set RESEND_API_KEY on the backend, or switch to explicit mock mode for local-only dev codes."
    : "Email delivery is not configured for real signup. Set RESEND_API_KEY on the backend, or switch to explicit mock mode for local-only dev codes.";
}

function logoutFailureMessage(error: unknown) {
  const message = (error as Error).message.toLowerCase();
  if (message.includes("still accepts")) {
    return "Aegis retried logout, but the backend still accepts this browser session. The current session stays active; do not request a fresh login code until logout verifies.";
  }
  if (message.includes("verification failed")) {
    return "Aegis received the logout response, but could not verify sign-out with the API. The current session stays active.";
  }
  return "Aegis could not confirm logout with the API, so the current session is still active. Check the backend connection and try again.";
}

function authRedirectReason(value: string | null | undefined) {
  if (
    value === "session_required" ||
    value === "session_expired" ||
    value === "session_check_failed"
  ) {
    return value;
  }
  return null;
}

function authRedirectNotice(
  reason: "session_required" | "session_expired" | "session_check_failed",
) {
  switch (reason) {
    case "session_expired":
      return {
        title: "Session not accepted",
        body: "Aegis checked the server before opening the app. The previous session is expired or revoked, so a fresh one-time code is required.",
      };
    case "session_check_failed":
      return {
        title: "Session check unavailable",
        body: "Aegis could not verify the server session, so it failed closed instead of opening portfolio pages from stale browser state.",
      };
    default:
      return {
        title: "Session required",
        body: "Protected portfolio pages open only after the backend accepts this browser session. Enter the email and verify the one-time code.",
      };
  }
}

function friendlyAuthError(error: unknown, loginMode: boolean) {
  const raw = (error as Error).message || "Wallet setup failed";
  const message = raw.replace(/^\d{3}:\s*/, "");
  const lower = message.toLowerCase();
  if (lower.includes("no account for this email")) {
    return "No wallet uses this email yet. Create a wallet first, then use this page to sign back in.";
  }
  if (
    lower.includes("wallet already exists") ||
    lower.includes("account already exists")
  ) {
    return "This email already has a wallet. Sign in with a one-time code instead of creating another wallet.";
  }
  if (
    lower.includes("invalid verification code") ||
    lower.includes("verification code not found")
  ) {
    return "That verification code is not correct. Check the latest email and try again.";
  }
  if (lower.includes("verification code expired")) {
    return "That verification code expired. Request a fresh code and try again.";
  }
  if (lower.includes("too many verification attempts")) {
    return "Too many wrong attempts. Request a fresh verification code.";
  }
  if (lower.includes("too many verification code requests")) {
    return "Too many codes were requested for this email. Wait a few minutes, then request one fresh code.";
  }
  if (lower.includes("wallet auth email is disabled")) {
    return loginMode
      ? "Email delivery is not configured for real login. Set RESEND_API_KEY on the backend, or run the wallet flow in explicit mock mode for local dev codes."
      : "Email delivery is not configured for real signup. Set RESEND_API_KEY on the backend, or run the wallet flow in explicit mock mode for local dev codes.";
  }
  if (lower.includes("resend status") || lower.includes("resend net")) {
    return "Aegis could not send the verification email. Check the backend mail provider, then request a fresh code.";
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

function isCorrectableCodeError(error: unknown) {
  const lower = ((error as Error).message || "").toLowerCase();
  return (
    lower.includes("invalid verification code") ||
    lower.includes("verification code not found")
  );
}
