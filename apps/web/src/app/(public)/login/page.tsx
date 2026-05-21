import { Suspense } from "react";
import { Shield } from "lucide-react";
import Link from "next/link";
import { CreateWalletCard } from "@/components/wallet/create-wallet-card";

export const metadata = {
  title: "Aegis · Log in",
  description: "Log back into your Aegis account with your email or passkey.",
};

function WalletCardSkeleton() {
  return (
    <div className="max-w-md mx-auto h-64 border-2 border-white/10 bg-[#141414] animate-pulse" />
  );
}

export default function LoginPage() {
  return (
    <main className="min-h-screen bg-bg text-text-default flex items-center justify-center p-6">
      <div className="w-full max-w-md">
        <Link
          href="/"
          className="flex items-center gap-2 justify-center mb-8 group"
        >
          <div className="w-8 h-8 rounded-sharp bg-accent-agent flex items-center justify-center border-brutal border-black">
            <Shield className="w-4 h-4 text-black" />
          </div>
          <span className="font-semibold text-lg text-text-hi group-hover:text-accent-agent">
            Aegis
          </span>
        </Link>
        <div className="mb-6 text-center space-y-2">
          <h1 className="text-2xl font-semibold text-text-hi font-mono tracking-tight">
            Restore wallet access
          </h1>
          <p className="text-sm text-text-lo font-mono leading-relaxed">
            Same email, same Circle wallet, same portfolios. No seed phrase and
            no duplicate account.
          </p>
        </div>
        <Suspense fallback={<WalletCardSkeleton />}>
          <CreateWalletCard loginMode />
        </Suspense>
      </div>
    </main>
  );
}
