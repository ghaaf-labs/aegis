import { Suspense } from "react";
import { AuthPageShell } from "@/components/wallet/auth-page-shell";
import { CreateWalletCard } from "@/components/wallet/create-wallet-card";

export const metadata = {
  title: "Aegis · Log in",
  description:
    "Log back into your Aegis wallet with an email verification code.",
};

function WalletCardSkeleton() {
  return (
    <div className="h-80 w-full border-2 border-white/10 bg-[#141414] animate-pulse" />
  );
}

export default function LoginPage() {
  return (
    <AuthPageShell
      mode="login"
      title="Restore wallet access"
      subtitle="Use the same email, verify the one-time code, and return to the existing Circle wallet and portfolios. No duplicate account is created."
    >
      <Suspense fallback={<WalletCardSkeleton />}>
        <CreateWalletCard loginMode />
      </Suspense>
    </AuthPageShell>
  );
}
