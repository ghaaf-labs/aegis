"use client";

import { useState } from "react";
import { useRouter, useSearchParams } from "next/navigation";
import { Mail, Fingerprint } from "lucide-react";
import {
  BrutalButton,
  BrutalCard,
  BrutalCardBody,
  BrutalCardHeader,
  BrutalPill,
} from "@aegis/ui";
import { walletApi, setToken, analyticsApi } from "@/lib/api";
import { usePortfolioStore } from "@/stores/portfolio";

type Mode = "passkey" | "otp-start" | "otp-verify";

/**
 * Two-path wallet onboarding:
 *
 *  • Passkey (WebAuthn) when `navigator.credentials` is available.
 *  • Email OTP fallback otherwise.
 *
 * Both end in `setToken` + Zustand wallet state + `wallet.created` analytic.
 */
export function CreateWalletCard() {
  const router = useRouter();
  const searchParams = useSearchParams();
  const referrerHandle = searchParams?.get("ref")?.trim().toLowerCase();
  const setWallet = usePortfolioStore((s) => s.setWallet);

  const [email, setEmail] = useState("");
  const [code, setCode] = useState("");
  const [mode, setMode] = useState<Mode>(() =>
    typeof window !== "undefined" &&
    typeof window.PublicKeyCredential === "function"
      ? "passkey"
      : "otp-start",
  );
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const submitPasskey = async () => {
    setSubmitting(true);
    setError(null);
    try {
      // Hackathon ergonomics: pass-through attestation. The MOCK_CIRCLE path
      // ignores the value; the live Circle WaaS path does the WebAuthn
      // ceremony server-side.
      const passkey = {
        kind: "webauthn",
        platform: window.navigator?.userAgent ?? "unknown",
      };
      const resp = await walletApi.createPasskey(
        email.trim(),
        passkey,
        referrerHandle || undefined,
      );
      setToken(resp.token);
      setWallet(resp.wallet);
      await analyticsApi.track("wallet.created", {
        method: "passkey",
        referrerHandle: referrerHandle || null,
      });
      router.push("/onboarding");
    } catch (e) {
      // If the passkey path fails (user cancellation, sandbox hiccup, server
      // rejection), drop into the OTP flow with the same email instead of
      // dead-ending. The user still completes onboarding in one session.
      const msg = (e as Error).message;
      setError(`${msg} — switched to email code as a fallback.`);
      setMode("otp-start");
      void analyticsApi.track("wallet.passkey_fallback", { reason: msg });
    } finally {
      setSubmitting(false);
    }
  };

  const startOtp = async () => {
    setSubmitting(true);
    setError(null);
    try {
      await walletApi.startOtp(email.trim());
      setMode("otp-verify");
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setSubmitting(false);
    }
  };

  const verifyOtp = async () => {
    setSubmitting(true);
    setError(null);
    try {
      const resp = await walletApi.verifyOtp(
        email.trim(),
        code.trim(),
        referrerHandle || undefined,
      );
      setToken(resp.token);
      setWallet(resp.wallet);
      await analyticsApi.track("wallet.created", {
        method: "otp",
        referrerHandle: referrerHandle || null,
      });
      router.push("/onboarding");
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <BrutalCard className="max-w-md mx-auto">
      <BrutalCardHeader>
        <div className="flex items-center gap-2">
          {mode === "passkey" ? (
            <Fingerprint className="w-4 h-4 text-accent-pnl" />
          ) : (
            <Mail className="w-4 h-4 text-accent-agent" />
          )}
          <span className="text-sm font-semibold text-text-hi">
            {mode === "passkey"
              ? "Create wallet (passkey)"
              : mode === "otp-start"
                ? "Create wallet (email code)"
                : "Enter your 6-digit code"}
          </span>
          <BrutalPill tone="agent">CIRCLE WALLET</BrutalPill>
        </div>
      </BrutalCardHeader>
      <BrutalCardBody>
        {mode !== "otp-verify" && (
          <label className="block text-xs text-text-lo font-mono mb-2">
            Email
          </label>
        )}
        {mode !== "otp-verify" && (
          <input
            value={email}
            onChange={(e) => setEmail(e.target.value)}
            type="email"
            autoComplete="email"
            placeholder="you@example.com"
            className="w-full px-3 py-2 bg-bg border-brutal border-border-default focus:border-border-hi rounded-sharp font-mono text-sm text-text-hi outline-none"
          />
        )}

        {mode === "otp-verify" && (
          <>
            <label className="block text-xs text-text-lo font-mono mb-2">
              We emailed a code to {email}
            </label>
            <input
              value={code}
              onChange={(e) =>
                setCode(e.target.value.replace(/\D/g, "").slice(0, 6))
              }
              inputMode="numeric"
              placeholder="000000"
              className="w-full px-3 py-2 bg-bg border-brutal border-border-default focus:border-border-hi rounded-sharp font-mono text-lg tracking-widest text-text-hi outline-none text-center"
            />
          </>
        )}

        {error && (
          <div className="mt-3 text-xs text-risk font-mono">{error}</div>
        )}

        <div className="mt-4 flex flex-col gap-2">
          {mode === "passkey" && (
            <>
              <BrutalButton
                variant="pnl"
                onClick={() => void submitPasskey()}
                disabled={!email || submitting}
              >
                {submitting ? "Creating…" : "Create with passkey"}
              </BrutalButton>
              <BrutalButton
                variant="ghost"
                onClick={() => setMode("otp-start")}
              >
                Use email code instead
              </BrutalButton>
            </>
          )}
          {mode === "otp-start" && (
            <>
              <BrutalButton
                variant="agent"
                onClick={() => void startOtp()}
                disabled={!email || submitting}
              >
                {submitting ? "Sending…" : "Send me a code"}
              </BrutalButton>
              {typeof window !== "undefined" &&
                typeof window.PublicKeyCredential === "function" && (
                  <BrutalButton
                    variant="ghost"
                    onClick={() => setMode("passkey")}
                  >
                    Use passkey instead
                  </BrutalButton>
                )}
            </>
          )}
          {mode === "otp-verify" && (
            <>
              <BrutalButton
                variant="pnl"
                onClick={() => void verifyOtp()}
                disabled={code.length !== 6 || submitting}
              >
                {submitting ? "Verifying…" : "Verify & create wallet"}
              </BrutalButton>
              <BrutalButton
                variant="ghost"
                onClick={() => {
                  setCode("");
                  void startOtp();
                }}
                disabled={submitting}
              >
                Resend code
              </BrutalButton>
            </>
          )}
        </div>
      </BrutalCardBody>
    </BrutalCard>
  );
}
