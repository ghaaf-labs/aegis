import { Suspense } from "react";
import { Shield } from "lucide-react";
import Link from "next/link";
import { CreateWalletCard } from "@/components/wallet/create-wallet-card";

export const metadata = {
  title: "Aegis · Sign up",
  description:
    "Create your Circle Wallet — passkey or email code, no seed phrase.",
};

function WalletCardSkeleton() {
  return (
    <div className="max-w-md mx-auto h-64 border-2 border-white/10 bg-[#141414] animate-pulse" />
  );
}

export default function SignupPage() {
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
            Create wallet access
          </h1>
          <p className="text-sm text-text-lo font-mono leading-relaxed">
            One email starts a Circle wallet, then Aegis builds your first
            portfolio goal before any trade can be approved.
          </p>
        </div>
        {/* CreateWalletCard reads `?ref=` via useSearchParams (referral
            attribution, Sprint 4). Next.js 15 requires a Suspense boundary
            around any child that calls useSearchParams during SSG. */}
        <Suspense fallback={<WalletCardSkeleton />}>
          <CreateWalletCard />
        </Suspense>
        <p className="mt-6 text-center text-xs font-mono text-text-mut">
          Just looking?{" "}
          <Link href="/explore" className="text-accent-agent hover:underline">
            Explore demo portfolios
          </Link>
          .
        </p>
      </div>
    </main>
  );
}
