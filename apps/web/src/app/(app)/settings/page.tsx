"use client";

import { useEffect, useState } from "react";
import Link from "next/link";
import {
  BarChart3,
  Bot,
  CircleHelp,
  Wallet,
  Shield,
  Receipt,
  CreditCard,
  AlertTriangle,
  Mail,
  Eye,
  ArrowRight,
  ListChecks,
  SquareTerminal,
  LockKeyhole,
  Download,
  Loader2,
  Trash2,
} from "lucide-react";
import { PRICING_UI_ENABLED } from "@/lib/flags";
import { DigestOptIn } from "@/components/settings/digest-opt-in";
import { DiaryVisibilityToggle } from "@/components/settings/diary-visibility-toggle";
import { PlanUsage } from "@/components/settings/plan-usage";
import { ReferralShare } from "@/components/settings/referral-share";
import { accountApi, portfolioApi, walletApi } from "@/lib/api";
import { friendlyAccountError } from "@/lib/account-error-copy";
import { useApiQuery } from "@/lib/use-api-query";
import { useActivePortfolio, usePortfolioStore } from "@/stores/portfolio";

interface SectionLink {
  href: string;
  icon: React.ComponentType<{ className?: string }>;
  title: string;
  description: string;
  enabled?: boolean;
  requiresWallet?: boolean;
}

export default function SettingsIndex() {
  const portfolio = useActivePortfolio();
  const wallet = usePortfolioStore((s) => s.wallet);
  const resetSession = usePortfolioStore((s) => s.resetSession);
  const portfolioId = portfolio?.id ?? "";

  const diaryQuery = useApiQuery(
    `portfolio.diaryPublic.${portfolioId}`,
    () => portfolioApi.getDiaryPublic(portfolioId),
    { enabled: !!portfolioId },
  );
  const [localDiaryPublic, setLocalDiaryPublic] = useState<boolean | null>(
    null,
  );
  const diaryPublic = localDiaryPublic ?? diaryQuery.data?.diaryPublic ?? false;

  const [storedEmail, setStoredEmail] = useState("");
  const [exportStatus, setExportStatus] = useState<
    "idle" | "sending" | "sent" | "error"
  >("idle");
  const [exportMessage, setExportMessage] = useState("");
  const [deleteConfirming, setDeleteConfirming] = useState(false);
  const [deleteStatus, setDeleteStatus] = useState<
    "idle" | "closing" | "error"
  >("idle");
  const [deleteMessage, setDeleteMessage] = useState("");
  const [newEmail, setNewEmail] = useState("");
  const [emailCode, setEmailCode] = useState("");
  const [emailChallenge, setEmailChallenge] = useState<{
    id: string;
    email: string;
  } | null>(null);
  const [emailStatus, setEmailStatus] = useState<
    "idle" | "sending" | "verifying" | "sent" | "updated" | "error"
  >("idle");
  const [emailMessage, setEmailMessage] = useState("");
  const normalizedNewEmail = newEmail.trim().toLowerCase();
  const storedEmailNormalized = storedEmail.trim().toLowerCase();
  const newEmailValidationMessage =
    normalizedNewEmail.length === 0
      ? ""
      : !isValidEmail(normalizedNewEmail)
        ? "Enter a valid email address."
        : normalizedNewEmail === storedEmailNormalized
          ? "Use a different email address."
          : "";
  const newEmailInvalid = !!newEmailValidationMessage;
  const newEmailHelpId = "settings-new-email-help";
  const newEmailValidationId = "settings-new-email-validation";
  const emailMessageId = "settings-email-message";
  const newEmailDescription = [
    !emailChallenge && !emailMessage && !newEmailInvalid ? newEmailHelpId : "",
    newEmailInvalid ? newEmailValidationId : "",
    emailMessage ? emailMessageId : "",
  ]
    .filter(Boolean)
    .join(" ");
  const canRequestEmailUpdate =
    isValidEmail(normalizedNewEmail) &&
    normalizedNewEmail !== storedEmailNormalized;
  const emailActionDisabled =
    emailStatus === "sending" ||
    emailStatus === "verifying" ||
    (!emailChallenge && !canRequestEmailUpdate);

  useEffect(() => {
    let cancelled = false;
    const remembered = localStorage.getItem("aegis_email") ?? "";
    setStoredEmail(remembered);
    if (remembered) return;
    walletApi
      .session()
      .then((session) => {
        if (cancelled) return;
        localStorage.setItem("aegis_email", session.user.email);
        setStoredEmail(session.user.email);
      })
      .catch(() => {
        if (!cancelled) setStoredEmail("");
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const requestExport = async () => {
    setExportStatus("sending");
    setExportMessage("");
    try {
      const response = await accountApi.exportData();
      setExportStatus("sent");
      setExportMessage(
        `Export queued. Check ${response.deliveryEmail} for a signed download link.`,
      );
    } catch (e) {
      setExportStatus("error");
      setExportMessage(friendlyAccountError(e));
    }
  };

  const closeAccount = async () => {
    if (!deleteConfirming) {
      setDeleteConfirming(true);
      setDeleteMessage("Click Confirm close account to finish.");
      return;
    }
    setDeleteStatus("closing");
    setDeleteMessage("");
    try {
      await accountApi.deleteAccount();
      resetSession();
      localStorage.removeItem("aegis_email");
      window.location.replace("/login?signedOut=1");
    } catch (e) {
      setDeleteStatus("error");
      setDeleteMessage(friendlyAccountError(e));
      setDeleteConfirming(false);
    }
  };

  const requestEmailUpdate = async () => {
    if (!isValidEmail(normalizedNewEmail)) {
      setEmailStatus("error");
      setEmailMessage("Enter a valid email address.");
      return;
    }
    if (normalizedNewEmail === storedEmail.trim().toLowerCase()) {
      setEmailStatus("error");
      setEmailMessage("Use a different email address.");
      return;
    }
    setEmailStatus("sending");
    setEmailMessage("");
    try {
      const response = await accountApi.startEmailUpdate(normalizedNewEmail);
      setEmailChallenge({
        id: response.challengeId,
        email: response.email,
      });
      setEmailCode("");
      setEmailStatus("sent");
      setEmailMessage(`Code sent to ${response.email}.`);
    } catch (e) {
      setEmailStatus("error");
      setEmailMessage(friendlyAccountError(e));
    }
  };

  const resetEmailChallenge = () => {
    setEmailChallenge(null);
    setEmailCode("");
    setEmailStatus("idle");
    setEmailMessage("");
  };

  const confirmEmailUpdate = async () => {
    if (!emailChallenge) return;
    if (!/^\d{6}$/.test(emailCode.trim())) {
      setEmailStatus("error");
      setEmailMessage("Enter the 6-digit code.");
      return;
    }
    setEmailStatus("verifying");
    setEmailMessage("");
    try {
      const response = await accountApi.verifyEmailUpdate(
        emailChallenge.id,
        emailCode.trim(),
      );
      localStorage.setItem("aegis_email", response.email);
      setStoredEmail(response.email);
      setNewEmail("");
      setEmailCode("");
      setEmailChallenge(null);
      setEmailStatus("updated");
      setEmailMessage("Email updated.");
    } catch (e) {
      setEmailStatus("error");
      setEmailMessage(friendlyAccountError(e));
    }
  };

  const sections: SectionLink[] = [
    {
      href: "/wallets",
      icon: Wallet,
      title: "Wallet",
      description: "One account wallet with network token balances",
      requiresWallet: false,
    },
    {
      href: "/transactions",
      icon: ListChecks,
      title: "Transactions",
      description: "Reviews, approvals, and completed moves",
    },
    {
      href: "/analytics",
      icon: BarChart3,
      title: "Analytics",
      description: "Value, targets, and recent performance",
    },
    {
      href: "/settings/agent",
      icon: Shield,
      title: "Agent",
      description: "Pause automatic checks and review agent controls",
    },
    {
      href: "/agent-logs",
      icon: SquareTerminal,
      title: "Agent logs",
      description: "Past recommendations and status",
    },
    {
      href: "/agent-studio",
      icon: Bot,
      title: "Agent Studio",
      description: "Ask for a recommendation",
    },
    {
      href: "/settings/peg",
      icon: AlertTriangle,
      title: "Peg defense",
      description: "Stablecoin guardrails and alerts",
    },
    {
      href: "/tax-center",
      icon: Receipt,
      title: "Tax center",
      description: "Download reports and share them with your accountant",
    },
    {
      href: "/help",
      icon: CircleHelp,
      title: "Help",
      description: "Answers for wallet cash, approvals, logs, and exports",
      requiresWallet: false,
    },
    {
      href: "/settings/billing",
      icon: CreditCard,
      title: "Billing",
      description: "Subscription tier, fee history, payment method",
      enabled: PRICING_UI_ENABLED,
    },
  ];

  return (
    <div className="max-w-[1400px] mx-auto space-y-6">
      <h1 className="text-2xl font-semibold text-text-hi font-mono tracking-tight">
        Settings
      </h1>

      {!wallet && (
        <section className="border border-warn/40 bg-warn/5 p-4 font-mono">
          <div className="flex flex-wrap items-start justify-between gap-3">
            <div>
              <p className="text-[10px] uppercase tracking-widest text-warn">
                Account setup required
              </p>
              <p className="mt-2 max-w-2xl text-xs leading-relaxed text-text-lo">
                Account setup is still finishing. Portfolio, tax, billing, peg,
                and agent controls unlock when your wallet is ready.
              </p>
            </div>
            <Link
              href="/wallets"
              className="inline-flex min-h-9 items-center justify-center rounded-sharp border border-warn/40 bg-bg px-3 text-[11px] uppercase tracking-widest text-warn hover:bg-warn/10"
            >
              Finish setup
            </Link>
          </div>
        </section>
      )}

      <section>
        <h2 className="text-xs uppercase tracking-wider text-text-mut font-mono mb-3">
          Account
        </h2>
        <div className="grid grid-cols-1 gap-3 lg:grid-cols-3">
          <div className="rounded-sharp border-brutal border-border-default bg-bg p-4">
            <div className="flex items-start gap-3">
              <Mail className="mt-0.5 h-4 w-4 shrink-0 text-accent-agent" />
              <div className="min-w-0 flex-1">
                <p className="font-mono text-sm font-semibold text-text-hi">
                  Change email
                </p>
                <p className="mt-1 break-all font-mono text-[11px] leading-relaxed text-text-lo">
                  {storedEmail || "No email found in this browser."}
                </p>
                <p className="mt-2 font-mono text-[11px] leading-relaxed text-text-mut">
                  Enter a new email. We will send a 6-digit code.
                </p>
                <input
                  value={newEmail}
                  onChange={(e) => {
                    setNewEmail(e.target.value);
                    if (emailStatus === "error") {
                      setEmailStatus("idle");
                      setEmailMessage("");
                    }
                  }}
                  type="email"
                  inputMode="email"
                  autoComplete="email"
                  placeholder="new@example.com"
                  aria-label="New email address"
                  aria-invalid={newEmailInvalid}
                  aria-describedby={newEmailDescription || undefined}
                  className="mt-3 min-h-10 w-full rounded-sharp border-brutal border-border-default bg-bg px-3 py-2 font-mono text-sm text-text-hi outline-none focus:border-border-hi"
                />
                {newEmailInvalid && (
                  <p
                    id={newEmailValidationId}
                    role="alert"
                    className="mt-2 font-mono text-[11px] leading-relaxed text-risk"
                  >
                    {newEmailValidationMessage}
                  </p>
                )}
                {emailChallenge && (
                  <div className="mt-2 space-y-2">
                    <input
                      value={emailCode}
                      onChange={(e) =>
                        setEmailCode(
                          e.target.value.replace(/\D/g, "").slice(0, 6),
                        )
                      }
                      type="text"
                      inputMode="numeric"
                      autoComplete="one-time-code"
                      placeholder="123456"
                      aria-label="6-digit email verification code"
                      className="min-h-10 w-full rounded-sharp border-brutal border-border-default bg-bg px-3 py-2 font-mono text-sm tracking-[0.3em] text-text-hi outline-none focus:border-border-hi"
                    />
                    <button
                      type="button"
                      onClick={resetEmailChallenge}
                      className="font-mono text-[11px] text-text-mut underline-offset-2 hover:text-text-hi hover:underline"
                    >
                      Use a different email
                    </button>
                  </div>
                )}
                <button
                  type="button"
                  onClick={() =>
                    void (emailChallenge
                      ? confirmEmailUpdate()
                      : requestEmailUpdate())
                  }
                  disabled={emailActionDisabled}
                  className="mt-3 inline-flex min-h-10 w-full items-center justify-center gap-2 rounded-sharp border-brutal border-black bg-accent-agent px-4 font-mono text-sm font-semibold text-black shadow-brutal-sm hover:shadow-brutal disabled:opacity-50"
                >
                  {emailStatus === "sending" || emailStatus === "verifying" ? (
                    <Loader2 className="h-4 w-4 animate-spin" />
                  ) : (
                    <Mail className="h-4 w-4" />
                  )}
                  {emailChallenge ? "Verify email" : "Send code"}
                </button>
                {!emailChallenge && !emailMessage && !newEmailInvalid && (
                  <p
                    id={newEmailHelpId}
                    className="mt-2 font-mono text-[11px] leading-relaxed text-text-mut"
                  >
                    Use an email you can access.
                  </p>
                )}
                {emailMessage && (
                  <p
                    id={emailMessageId}
                    role={emailStatus === "error" ? "alert" : undefined}
                    aria-live="polite"
                    className={`mt-2 font-mono text-[11px] leading-relaxed ${
                      emailStatus === "error" ? "text-risk" : "text-text-lo"
                    }`}
                  >
                    {emailMessage}
                  </p>
                )}
              </div>
            </div>
          </div>

          <div className="rounded-sharp border-brutal border-border-default bg-bg p-4">
            <div className="flex items-start gap-3">
              <Download className="mt-0.5 h-4 w-4 shrink-0 text-accent-agent" />
              <div className="min-w-0 flex-1">
                <p className="font-mono text-sm font-semibold text-text-hi">
                  Export account data
                </p>
                <p className="mt-1 font-mono text-[11px] leading-relaxed text-text-lo">
                  Get a private download link by email.
                </p>
                <button
                  type="button"
                  onClick={() => void requestExport()}
                  disabled={exportStatus === "sending"}
                  className="mt-3 inline-flex min-h-10 w-full items-center justify-center gap-2 rounded-sharp border-brutal border-black bg-accent-agent px-4 font-mono text-sm font-semibold text-black shadow-brutal-sm hover:shadow-brutal disabled:opacity-50"
                >
                  {exportStatus === "sending" ? (
                    <Loader2 className="h-4 w-4 animate-spin" />
                  ) : (
                    <Download className="h-4 w-4" />
                  )}
                  Email export link
                </button>
                {exportMessage && (
                  <p
                    aria-live="polite"
                    className={`mt-2 font-mono text-[11px] leading-relaxed ${
                      exportStatus === "error" ? "text-risk" : "text-text-lo"
                    }`}
                  >
                    {exportMessage}
                  </p>
                )}
              </div>
            </div>
          </div>

          <div className="rounded-sharp border-brutal border-risk/45 bg-risk/5 p-4">
            <div className="flex items-start gap-3">
              <Trash2 className="mt-0.5 h-4 w-4 shrink-0 text-risk" />
              <div className="min-w-0 flex-1">
                <p className="font-mono text-sm font-semibold text-text-hi">
                  Close account
                </p>
                <p className="mt-1 font-mono text-[11px] leading-relaxed text-text-lo">
                  This signs you out and starts deletion. Wallet balances must
                  be empty first.
                </p>
                <button
                  type="button"
                  onClick={() => void closeAccount()}
                  disabled={deleteStatus === "closing"}
                  className="mt-3 inline-flex min-h-10 w-full items-center justify-center gap-2 rounded-sharp border-brutal border-risk bg-risk px-4 font-mono text-sm font-semibold text-black shadow-brutal-sm hover:shadow-brutal disabled:opacity-50"
                >
                  {deleteStatus === "closing" ? (
                    <Loader2 className="h-4 w-4 animate-spin" />
                  ) : (
                    <Trash2 className="h-4 w-4" />
                  )}
                  {deleteConfirming ? "Confirm close account" : "Close account"}
                </button>
                {deleteMessage && (
                  <p
                    aria-live="polite"
                    className={`mt-2 font-mono text-[11px] leading-relaxed ${
                      deleteStatus === "error" ? "text-risk" : "text-text-lo"
                    }`}
                  >
                    {deleteMessage}
                  </p>
                )}
              </div>
            </div>
          </div>
        </div>
      </section>

      {wallet && (
        <section>
          <h2 className="text-xs uppercase tracking-wider text-text-mut font-mono mb-3 flex items-center gap-2">
            <CreditCard className="w-3 h-3" /> Plan & referrals
          </h2>
          <div className="grid grid-cols-1 gap-3 lg:grid-cols-2">
            <PlanUsage portfolioId={portfolioId} />
            <ReferralShare />
          </div>
        </section>
      )}

      <section>
        <h2 className="text-xs uppercase tracking-wider text-text-mut font-mono mb-3">
          Sections
        </h2>
        <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
          {sections
            .filter((s) => s.enabled !== false)
            .map((s) => {
              const locked = !wallet && s.requiresWallet !== false;
              return (
                <Link
                  key={s.href}
                  href={locked ? "/wallets" : s.href}
                  title={
                    locked
                      ? `${s.title} unlocks after account setup finishes`
                      : s.title
                  }
                  className={`group flex items-start gap-3 rounded-sharp border-brutal bg-bg p-4 transition-colors ${
                    locked
                      ? "border-warn/35 hover:border-warn/60"
                      : "border-border-default hover:border-border-hi"
                  }`}
                >
                  <s.icon
                    className={`mt-0.5 h-4 w-4 shrink-0 ${
                      locked ? "text-warn" : "text-accent-agent"
                    }`}
                  />
                  <div className="flex-1 min-w-0">
                    <p className="text-sm font-semibold text-text-hi font-mono">
                      {s.title}
                    </p>
                    <p className="text-[11px] text-text-lo font-mono mt-0.5 leading-relaxed">
                      {locked
                        ? "Finish account setup first. This page uses account-backed data or actions."
                        : s.description}
                    </p>
                  </div>
                  {locked ? (
                    <LockKeyhole className="mt-0.5 h-3.5 w-3.5 shrink-0 text-warn" />
                  ) : (
                    <ArrowRight className="mt-0.5 h-3.5 w-3.5 shrink-0 text-text-mut group-hover:text-text-hi" />
                  )}
                </Link>
              );
            })}
        </div>
      </section>

      <section>
        <h2 className="text-xs uppercase tracking-wider text-text-mut font-mono mb-3 flex items-center gap-2">
          <Mail className="w-3 h-3" /> Notifications
        </h2>
        <DigestOptIn key={storedEmail} defaultEmail={storedEmail} />
      </section>

      {portfolioId && (
        <section>
          <h2 className="text-xs uppercase tracking-wider text-text-mut font-mono mb-3 flex items-center gap-2">
            <Eye className="w-3 h-3" /> Privacy
          </h2>
          <DiaryVisibilityToggle
            key={`diary-${portfolioId}-${diaryPublic}`}
            initialPublic={diaryPublic}
            walletAddress={wallet?.arcAddress}
            onChange={async (next) => {
              const res = await portfolioApi.setDiaryPublic(portfolioId, next);
              setLocalDiaryPublic(res.diaryPublic);
            }}
          />
        </section>
      )}
    </div>
  );
}

function isValidEmail(value: string) {
  return /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(value) && value.length <= 254;
}
