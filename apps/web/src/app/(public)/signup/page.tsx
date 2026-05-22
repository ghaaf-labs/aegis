import { Suspense } from "react";
import Link from "next/link";
import { AuthPageShell } from "@/components/wallet/auth-page-shell";
import { CreateWalletCard } from "@/components/wallet/create-wallet-card";

export const metadata = {
  title: "Aegis · Sign up",
  description: "Create your Circle Wallet with email verification and a PIN.",
};

function WalletCardSkeleton() {
  return (
    <div className="h-80 w-full border-2 border-white/10 bg-[#141414] animate-pulse" />
  );
}

export default function SignupPage() {
  return (
    <AuthPageShell
      mode="signup"
      title="Create wallet access"
      subtitle="Verify your email, set the Circle wallet PIN, then build a portfolio goal before any trade can be approved."
      footer={
        <p className="mt-6 text-xs font-mono text-text-mut">
          Just looking?{" "}
          <Link href="/explore" className="text-accent-agent hover:underline">
            Explore demo portfolios
          </Link>
          .
        </p>
      }
    >
      {/* CreateWalletCard reads `?ref=` via useSearchParams (referral
            attribution, Sprint 4). Next.js 15 requires a Suspense boundary
            around any child that calls useSearchParams during SSG. */}
      <Suspense fallback={<WalletCardSkeleton />}>
        <CreateWalletCard />
      </Suspense>
    </AuthPageShell>
  );
}
