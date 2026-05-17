"use client";

import { useState } from "react";
import { useRouter, useSearchParams } from "next/navigation";
import { Mail, Loader2 } from "lucide-react";
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

type Mode = "email" | "challenge" | "polling";

/**
 * Circle W3S User-Controlled wallet onboarding.
 *
 * 1. User enters email → POST `/auth/wallet/create` → server returns a
 *    `UserTokenBundle` (userToken + encryptionKey + appId + challengeId).
 * 2. Browser dynamically imports `@circle-fin/w3s-pw-web-sdk`, instantiates
 *    `W3SSdk`, calls `setAuthentication(...)` then `execute(challengeId)` to
 *    drive the PIN ceremony. The SDK signs the wallet creation request.
 * 3. We poll `/auth/wallet/status` every 2s until Circle has provisioned both
 *    ARC and BASE addresses, then redirect to onboarding.
 *
 * Returning users (`isNewUser=false`) skip step 2 — the bundle has no
 * challengeId and the wallet is already on the response.
 */
export function CreateWalletCard() {
  const router = useRouter();
  const searchParams = useSearchParams();
  const referrerHandle = searchParams?.get("ref")?.trim().toLowerCase();
  const setWallet = usePortfolioStore((s) => s.setWallet);

  const [email, setEmail] = useState("");
  const [mode, setMode] = useState<Mode>("email");
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const finish = async (
    method: "passkey" | "returning",
    wallet: {
      walletId: string;
      arcAddress: string;
      baseAddress: string;
      createdAt: string;
    },
  ) => {
    setWallet(wallet);
    await analyticsApi.track("wallet.created", {
      method,
      referrerHandle: referrerHandle || null,
    });
    router.push("/onboarding");
  };

  const runChallengeAndPoll = async (bundle: UserTokenBundle) => {
    if (!bundle.challengeId) {
      // Returning user; wallet is either on the auth response or arrives via
      // a quick status poll. Caller already handled the inline-wallet case.
      setMode("polling");
      await pollStatus("returning");
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
      sdk.execute(bundle.challengeId!, (sdkError, result) => {
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
    await pollStatus("passkey");
  };

  /**
   * Poll `/auth/wallet/status` until both ARC and BASE addresses come back.
   * Caps at ~30s (15 attempts × 2s) so a stuck Circle never traps the user.
   */
  const pollStatus = async (method: "passkey" | "returning") => {
    for (let i = 0; i < 15; i++) {
      const resp = await walletApi.status();
      if (resp.wallet) {
        await finish(method, resp.wallet);
        return;
      }
      await new Promise((r) => setTimeout(r, 2000));
    }
    throw new Error("Wallet provisioning timed out — refresh and try again");
  };

  const submitEmail = async () => {
    setSubmitting(true);
    setError(null);
    try {
      const resp = await walletApi.create(
        email.trim(),
        referrerHandle || undefined,
      );
      setToken(resp.token);
      // If the user already had a wallet (re-running signup with the same
      // email), the response carries it inline — skip the SDK ceremony.
      if (resp.wallet) {
        await finish(resp.isNewUser ? "passkey" : "returning", resp.wallet);
        return;
      }
      await runChallengeAndPoll(resp.bundle);
    } catch (e) {
      setError((e as Error).message);
      setMode("email");
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <BrutalCard className="max-w-md mx-auto">
      <BrutalCardHeader>
        <div className="flex items-center gap-2">
          {mode === "email" ? (
            <Mail className="w-4 h-4 text-accent-agent" />
          ) : (
            <Loader2 className="w-4 h-4 text-accent-agent animate-spin" />
          )}
          <span className="text-sm font-semibold text-text-hi">
            {mode === "email"
              ? "Create wallet"
              : mode === "challenge"
                ? "Set your PIN"
                : "Provisioning wallets…"}
          </span>
          <BrutalPill tone="agent">CIRCLE W3S</BrutalPill>
        </div>
      </BrutalCardHeader>
      <BrutalCardBody>
        {mode === "email" && (
          <>
            <label className="block text-xs text-text-lo font-mono mb-2">
              Email
            </label>
            <input
              value={email}
              onChange={(e) => setEmail(e.target.value)}
              type="email"
              autoComplete="email"
              placeholder="you@example.com"
              className="w-full px-3 py-2 bg-bg border-brutal border-border-default focus:border-border-hi rounded-sharp font-mono text-sm text-text-hi outline-none"
            />
          </>
        )}
        {mode === "challenge" && (
          <p className="text-xs text-text-lo font-mono">
            A PIN dialog will open in this tab — pick a 6-digit PIN and confirm.
            The PIN is held by Circle&apos;s SDK locally; it never reaches Aegis
            or Circle&apos;s servers.
          </p>
        )}
        {mode === "polling" && (
          <p className="text-xs text-text-lo font-mono">
            Circle is creating your wallets on Arc Testnet + Base Sepolia. This
            usually finishes in 5–10s.
          </p>
        )}

        {error && (
          <div className="mt-3 text-xs text-risk font-mono">{error}</div>
        )}

        {mode === "email" && (
          <div className="mt-4 flex flex-col gap-2">
            <BrutalButton
              variant="agent"
              onClick={() => void submitEmail()}
              disabled={!email || submitting}
            >
              {submitting ? "Starting…" : "Continue"}
            </BrutalButton>
          </div>
        )}
      </BrutalCardBody>
    </BrutalCard>
  );
}
