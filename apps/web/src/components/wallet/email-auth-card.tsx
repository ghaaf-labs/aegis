"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import Link from "next/link";
import { useRouter, useSearchParams } from "next/navigation";
import {
  ArrowRight,
  CheckCircle2,
  CircleAlert,
  KeyRound,
  Loader2,
  LogIn,
  RotateCw,
} from "lucide-react";
import {
  BrutalButton,
  BrutalCard,
  BrutalCardBody,
  BrutalCardHeader,
} from "@aegis/ui";
import { analyticsApi, walletApi, type WalletAuthResponse } from "@/lib/api";
import { safeNextPath } from "@/lib/auth-routing";
import { usePortfolioStore } from "@/stores/portfolio";

type Mode = "email" | "verify" | "finishing" | "done";

export function EmailAuthCard() {
  const router = useRouter();
  const searchParams = useSearchParams();
  const referrerHandle = searchParams?.get("ref")?.trim().toLowerCase();
  const queryEmail = searchParams?.get("email")?.trim().toLowerCase();
  const nextPath = safeNextPath(searchParams?.get("next"));
  const signedOutFromQuery = searchParams?.get("signedOut") === "1";
  const redirectReason = authRedirectReason(searchParams?.get("reason"));
  const setWallet = usePortfolioStore((s) => s.setWallet);
  const setSessionActive = usePortfolioStore((s) => s.setSessionActive);
  const setSessionResolved = usePortfolioStore((s) => s.setSessionResolved);
  const resetSession = usePortfolioStore((s) => s.resetSession);

  const [email, setEmail] = useState("");
  const [code, setCode] = useState("");
  const [marketingOptIn, setMarketingOptIn] = useState(false);
  const [codeChallenge, setCodeChallenge] = useState<{
    id: string;
    email: string;
    expiresAt: string;
  } | null>(null);
  const [mode, setMode] = useState<Mode>("email");
  const [submitting, setSubmitting] = useState(false);
  const [resending, setResending] = useState(false);
  const [resendSeconds, setResendSeconds] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const [checkingAccount, setCheckingAccount] = useState(true);
  const mountedRef = useRef(true);
  const authFlowStartedRef = useRef(false);
  const codeInputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
    };
  }, []);

  useEffect(() => {
    if (typeof window === "undefined") return;
    if (
      signedOutFromQuery ||
      redirectReason === "session_expired" ||
      redirectReason === "session_check_failed"
    ) {
      window.localStorage.removeItem("aegis_email");
    }
    setEmail(queryEmail ?? "");
  }, [queryEmail, redirectReason, signedOutFromQuery]);

  useEffect(() => {
    let cancelled = false;
    let redirected = false;
    walletApi
      .session()
      .then((session) => {
        if (cancelled) return;
        setSessionActive(true);
        setSessionResolved(true);
        localStorage.setItem("aegis_email", session.user.email);
        setEmail(session.user.email);
        if (session.wallet) {
          setWallet(session.wallet);
          redirected = true;
          router.replace(nextPath ?? "/dashboard");
        } else {
          setWallet(null);
          redirected = true;
          router.replace("/onboarding");
        }
      })
      .catch(() => {
        if (!cancelled) {
          if (!authFlowStartedRef.current) resetSession();
          setSessionResolved(true);
        }
      })
      .finally(() => {
        if (!cancelled && !redirected) setCheckingAccount(false);
      });
    return () => {
      cancelled = true;
    };
  }, [
    nextPath,
    resetSession,
    router,
    setSessionActive,
    setSessionResolved,
    setWallet,
  ]);

  useEffect(() => {
    if (mode === "verify") codeInputRef.current?.focus();
  }, [mode]);

  useEffect(() => {
    if (mode !== "verify" || resendSeconds <= 0) return;
    const timer = window.setTimeout(() => {
      setResendSeconds((current) => Math.max(0, current - 1));
    }, 1000);
    return () => window.clearTimeout(timer);
  }, [mode, resendSeconds]);

  const finish = useCallback(
    async (resp: WalletAuthResponse) => {
      setError(null);
      setSessionActive(true);
      setSessionResolved(true);
      setWallet(resp.wallet);
      localStorage.setItem("aegis_email", resp.user.email);
      if (!resp.wallet) return false;
      setMode("done");
      await analyticsApi.track("auth.continued", {
        method: "email",
        referrerHandle: referrerHandle ?? null,
      });
      router.replace(nextPath ?? "/dashboard");
      return true;
    },
    [
      nextPath,
      referrerHandle,
      router,
      setSessionActive,
      setSessionResolved,
      setWallet,
    ],
  );

  const checkAccountReady = useCallback(async () => {
    setSubmitting(true);
    authFlowStartedRef.current = true;
    setError(null);
    try {
      const session = await walletApi.session();
      if (session.wallet) {
        await finish({
          user: {
            id: session.user.id,
            email: session.user.email,
            riskTolerance: session.user.riskTolerance,
            accountStatus: session.user.accountStatus,
          },
          status: "active",
          wallet: session.wallet,
        });
        return;
      }
      setMode("finishing");
      setError(
        "Setting up your account is taking longer than usual. Try again.",
      );
    } catch (e) {
      setError(friendlyAuthError(e));
      setMode("email");
    } finally {
      if (mountedRef.current) setSubmitting(false);
    }
  }, [finish]);

  const requestVerificationCode = async () => {
    const normalizedEmail = email.trim().toLowerCase();
    if (checkingAccount) {
      setError("Aegis is still checking whether you are already signed in.");
      return;
    }
    if (!isValidEmail(normalizedEmail)) {
      setError("Enter a valid email address.");
      return;
    }
    setSubmitting(true);
    authFlowStartedRef.current = true;
    setError(null);
    try {
      const resp = await walletApi.startEmail(
        normalizedEmail,
        referrerHandle || undefined,
      );
      setEmail(resp.email);
      setCode("");
      setCodeChallenge({
        id: resp.challengeId,
        email: resp.email,
        expiresAt: resp.expiresAt,
      });
      setResendSeconds(resp.resendInSeconds);
      setMode("verify");
    } catch (e) {
      setError(friendlyAuthError(e));
      setMode("email");
    } finally {
      setSubmitting(false);
    }
  };

  const resendVerificationCode = async () => {
    if (!codeChallenge) {
      setError("Request a new code first.");
      setMode("email");
      return;
    }
    if (resendSeconds > 0) return;

    setResending(true);
    setError(null);
    try {
      const resp = await walletApi.resendEmail(codeChallenge.id);
      setCode("");
      setCodeChallenge({
        id: resp.challengeId,
        email: resp.email,
        expiresAt: resp.expiresAt,
      });
      setResendSeconds(resp.resendInSeconds);
    } catch (e) {
      setError(friendlyAuthError(e));
    } finally {
      if (mountedRef.current) setResending(false);
    }
  };

  const submitVerificationCode = async () => {
    const normalizedCode = code.trim();
    if (checkingAccount) {
      setError("Aegis is still checking whether you are already signed in.");
      return;
    }
    if (!codeChallenge) {
      setError("Request a new code first.");
      setMode("email");
      return;
    }
    if (!/^\d{6}$/.test(normalizedCode)) {
      setError("Enter the 6-digit code.");
      return;
    }
    setSubmitting(true);
    authFlowStartedRef.current = true;
    setError(null);
    try {
      const resp = await walletApi.verifyEmail(
        codeChallenge.id,
        normalizedCode,
        {
          tos: true,
          privacy: true,
          tosVersion: "2026-05",
          privacyVersion: "2026-05",
          marketingOptIn,
        },
      );
      setCodeChallenge(null);
      setCode("");
      if (await finish(resp)) return;
      setMode("finishing");
      setError(
        "Setting up your account is taking longer than usual. Try again.",
      );
    } catch (e) {
      setError(friendlyAuthError(e));
      setMode(isCorrectableCodeError(e) ? "verify" : "email");
    } finally {
      if (mountedRef.current) setSubmitting(false);
    }
  };

  const normalizedEmail = email.trim().toLowerCase();
  const emailValid = isValidEmail(normalizedEmail);
  const emailInvalid = normalizedEmail.length > 0 && !emailValid;
  const codeValid = /^\d{6}$/.test(code.trim());
  const emailHelpId = "wallet-auth-email-help";
  const emailInvalidId = "wallet-auth-email-invalid";
  const emailInputId = "wallet-auth-email-input";
  const errorId = "wallet-auth-error";
  const showSignedOutNotice = signedOutFromQuery && mode === "email";
  const redirectNotice = redirectReason
    ? authRedirectNotice(redirectReason)
    : null;
  const emailNotice =
    mode === "email"
      ? showSignedOutNotice
        ? "Signed out. Enter your email to continue."
        : redirectNotice
      : null;

  if (checkingAccount) {
    return (
      <BrutalCard className="w-full">
        <BrutalCardHeader>
          <div className="flex flex-wrap items-center gap-2">
            <Loader2 className="h-4 w-4 animate-spin text-accent-agent" />
            <span className="text-sm font-semibold text-text-hi">
              Opening Aegis
            </span>
          </div>
        </BrutalCardHeader>
        <BrutalCardBody>
          <p className="font-mono text-xs leading-relaxed text-text-lo">
            Checking whether this browser is already signed in.
          </p>
        </BrutalCardBody>
      </BrutalCard>
    );
  }

  return (
    <BrutalCard className="w-full">
      <BrutalCardHeader>
        <div className="flex flex-wrap items-center gap-2">
          {mode === "email" ? (
            <LogIn className="h-4 w-4 text-accent-agent" />
          ) : mode === "done" ? (
            <CheckCircle2 className="h-4 w-4 text-accent-agent" />
          ) : mode === "finishing" ? (
            <Loader2 className="h-4 w-4 animate-spin text-accent-agent" />
          ) : (
            <KeyRound className="h-4 w-4 text-accent-agent" />
          )}
          <span className="text-sm font-semibold text-text-hi">
            {mode === "email"
              ? "Continue with email"
              : mode === "verify"
                ? "Enter the code we emailed you"
                : mode === "finishing"
                  ? "Setting up your account..."
                  : "Opening Aegis..."}
          </span>
        </div>
      </BrutalCardHeader>
      <BrutalCardBody>
        {emailNotice && (
          <div className="mb-4 border border-accent-agent/40 bg-accent-agent/5 px-3 py-2 font-mono text-[11px] leading-relaxed text-text-lo">
            {emailNotice}
          </div>
        )}

        {mode === "email" && (
          <form
            onSubmit={(e) => {
              e.preventDefault();
              if (!submitting && emailValid) void requestVerificationCode();
            }}
          >
            <label
              htmlFor={emailInputId}
              className="mb-2 block font-mono text-xs text-text-lo"
            >
              Email
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
              aria-invalid={emailInvalid}
              aria-describedby={`${emailHelpId}${emailInvalid ? ` ${emailInvalidId}` : ""}${error ? ` ${errorId}` : ""}`}
              className="min-h-11 w-full rounded-sharp border-brutal border-border-default bg-bg px-3 py-2 font-mono text-base text-text-hi outline-none focus:border-border-hi sm:text-sm"
            />
            <p
              id={emailHelpId}
              className="mt-2 font-mono text-[11px] leading-relaxed text-text-mut"
            >
              We&apos;ll email you a 6-digit code.
            </p>
            {emailInvalid && (
              <p
                id={emailInvalidId}
                className="mt-2 font-mono text-[11px] leading-relaxed text-warn"
              >
                Enter a valid email address.
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
            <p className="break-all font-mono text-sm text-text-lo">
              Sent to{" "}
              <span className="text-text-hi">{codeChallenge.email}</span>.
            </p>
            <label
              htmlFor={`${emailInputId}-code`}
              className="block font-mono text-xs text-text-lo"
            >
              6-digit code
            </label>
            <input
              ref={codeInputRef}
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
              className="min-h-11 w-full rounded-sharp border-brutal border-border-default bg-bg px-3 py-2 font-mono text-base tracking-[0.3em] text-text-hi outline-none focus:border-border-hi sm:text-sm"
            />
            <p className="font-mono text-[11px] leading-relaxed text-text-mut">
              By continuing, you agree to our{" "}
              <Link
                href="/policy#terms"
                className="text-accent-agent hover:underline"
              >
                Terms
              </Link>{" "}
              and{" "}
              <Link
                href="/policy#privacy"
                className="text-accent-agent hover:underline"
              >
                Privacy Policy
              </Link>
              .
            </p>
            <label className="flex min-h-9 items-center gap-2 font-mono text-[11px] text-text-lo">
              <input
                checked={marketingOptIn}
                onChange={(e) => setMarketingOptIn(e.target.checked)}
                type="checkbox"
                className="h-4 w-4 accent-accent-agent"
              />
              Email me product updates.
            </label>
          </form>
        )}

        {mode === "finishing" && (
          <div
            className="space-y-3 font-mono text-xs text-text-lo"
            role="status"
            aria-live="polite"
          >
            <div className="flex items-start gap-2">
              <Loader2 className="mt-0.5 h-4 w-4 shrink-0 animate-spin text-accent-agent" />
              <p>This is taking longer than usual.</p>
            </div>
          </div>
        )}

        {error && (
          <div
            id={errorId}
            role="alert"
            className="mt-3 space-y-3 border border-risk/40 bg-risk/5 px-3 py-2 font-mono text-xs text-risk"
          >
            <div className="flex items-start gap-2">
              <CircleAlert className="mt-0.5 h-3.5 w-3.5 shrink-0" />
              <span>{error}</span>
            </div>
          </div>
        )}

        {mode !== "done" && (
          <div className="mt-4 flex flex-col gap-2">
            <BrutalButton
              type="button"
              data-testid="wallet-auth-submit"
              variant="agent"
              className="min-h-11 w-full"
              onClick={() =>
                void (mode === "email"
                  ? requestVerificationCode()
                  : mode === "verify"
                    ? submitVerificationCode()
                    : checkAccountReady())
              }
              disabled={
                submitting ||
                (mode === "email"
                  ? !emailValid
                  : mode === "verify"
                    ? !codeValid
                    : false)
              }
            >
              {submitting ? (
                <>
                  <Loader2 className="h-4 w-4 animate-spin" />
                  {mode === "email"
                    ? "Sending..."
                    : mode === "verify"
                      ? "Checking..."
                      : "Trying again..."}
                </>
              ) : (
                <>
                  {mode === "finishing" ? "Try again" : "Continue"}
                  {mode === "finishing" ? (
                    <RotateCw className="h-4 w-4" />
                  ) : (
                    <ArrowRight className="h-4 w-4" />
                  )}
                </>
              )}
            </BrutalButton>
            {mode === "verify" && (
              <div className="flex flex-wrap items-center justify-between gap-2 font-mono text-[11px]">
                <button
                  type="button"
                  className="min-h-9 text-accent-agent hover:underline"
                  onClick={() => {
                    setMode("email");
                    setCode("");
                    setCodeChallenge(null);
                    setResendSeconds(0);
                    setError(null);
                  }}
                >
                  Use a different email
                </button>
                <button
                  type="button"
                  disabled={submitting || resending || resendSeconds > 0}
                  onClick={() => void resendVerificationCode()}
                  className="inline-flex min-h-9 items-center gap-1 text-text-lo hover:text-accent-agent disabled:opacity-50"
                >
                  <RotateCw
                    className={`h-3 w-3 ${resending ? "animate-spin" : ""}`}
                  />
                  {resendSeconds > 0
                    ? `Resend code (${resendSeconds}s)`
                    : "Resend code"}
                </button>
              </div>
            )}
          </div>
        )}

        {mode === "done" && (
          <div className="mt-2 flex items-center gap-2 font-mono text-xs text-accent-agent">
            <CheckCircle2 className="h-4 w-4" />
            Opening Aegis...
          </div>
        )}
      </BrutalCardBody>
    </BrutalCard>
  );
}

function isValidEmail(email: string) {
  return /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(email) && email.length <= 254;
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
      return "Your session expired. Enter your email to continue.";
    case "session_check_failed":
      return "Aegis could not confirm this browser. Enter your email to continue.";
    default:
      return "Enter your email to continue.";
  }
}

function friendlyAuthError(error: unknown) {
  const raw = (error as Error).message || "Something went wrong.";
  const message = raw.replace(/^\d{3}:\s*/, "");
  const lower = message.toLowerCase();
  if (
    lower.includes("invalid verification code") ||
    lower.includes("verification code not found") ||
    lower.includes("code_invalid") ||
    lower.includes("that code didn't match")
  ) {
    return "That code did not match. Check it or request a new one.";
  }
  if (
    lower.includes("verification code expired") ||
    lower.includes("code_expired")
  ) {
    return "That code expired. Enter your email to get a new one.";
  }
  if (lower.includes("already used") || lower.includes("code_used")) {
    return "That code was already used. Enter your email to get a new one.";
  }
  if (
    lower.includes("too many verification attempts") ||
    lower.includes("too_many_attempts")
  ) {
    return "Too many tries. Enter your email to get a new code.";
  }
  if (
    lower.includes("too many verification code requests") ||
    lower.includes("rate_limited")
  ) {
    return "Too many requests. Try again shortly.";
  }
  if (lower.includes("resend_cooldown")) {
    return "You can request a new code shortly.";
  }
  if (
    lower.includes("verification email could not be sent") ||
    lower.includes("wallet auth email is disabled")
  ) {
    return "We could not send your code. Try again.";
  }
  if (lower.includes("invalid email")) {
    return "Enter a valid email address.";
  }
  if (lower.includes("failed to fetch") || lower.includes("networkerror")) {
    return "Aegis could not connect. Try again.";
  }
  if (lower.includes("unauthorized") || lower.includes("session expired")) {
    return "Your session expired. Enter your email to continue.";
  }
  return "Something went wrong on our end. Try again.";
}

function isCorrectableCodeError(error: unknown) {
  const lower = ((error as Error).message || "").toLowerCase();
  return (
    lower.includes("invalid verification code") ||
    lower.includes("verification code not found") ||
    lower.includes("code_invalid") ||
    lower.includes("that code didn't match") ||
    lower.includes("that code did not match")
  );
}
