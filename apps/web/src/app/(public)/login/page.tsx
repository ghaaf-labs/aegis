import { Suspense } from "react";
import { AuthPageShell } from "@/components/wallet/auth-page-shell";
import { EmailAuthCard } from "@/components/wallet/email-auth-card";

export const metadata = {
  title: "Aegis · Continue",
  description: "Continue to Aegis with an email code.",
};

function WalletCardSkeleton() {
  return (
    <div className="h-80 w-full border-2 border-white/10 bg-[#141414] animate-pulse" />
  );
}

export default function LoginPage() {
  return (
    <AuthPageShell>
      <Suspense fallback={<WalletCardSkeleton />}>
        <EmailAuthCard />
      </Suspense>
    </AuthPageShell>
  );
}
