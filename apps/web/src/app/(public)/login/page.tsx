import { Suspense } from "react";
import { AuthPageShell } from "@/components/wallet/auth-page-shell";
import { EmailAuthCard } from "@/components/wallet/email-auth-card";
import { pageMetadata } from "@/lib/seo";

export const metadata = {
  ...pageMetadata({
    title: "Continue — Aegis",
    description: "Sign in to Aegis with your email. No password required.",
    path: "/login",
  }),
  robots: "noindex, nofollow",
};

function WalletCardSkeleton() {
  return (
    <div className="w-full border-2 border-white/10 bg-[#141414] animate-pulse p-4">
      <h1 className="font-mono text-sm font-semibold text-white/40">
        Continue with email
      </h1>
      <div className="mt-4 h-11 rounded bg-white/5" />
      <div className="mt-3 h-4 w-3/5 rounded bg-white/5" />
      <div className="mt-4 h-11 rounded bg-white/10" />
    </div>
  );
}

export default function LoginPage() {
  return (
    <AuthPageShell>
      <Suspense fallback={<WalletCardSkeleton />}>
        <EmailAuthCard entry="login" />
      </Suspense>
    </AuthPageShell>
  );
}
