import { Shield } from "lucide-react";
import Link from "next/link";
import { CreateWalletCard } from "@/components/wallet/create-wallet-card";

export const metadata = {
  title: "Aegis · Sign up",
  description:
    "Create your Circle Wallet — passkey or email code, no seed phrase.",
};

export default function SignupPage() {
  return (
    <div className="min-h-screen bg-bg text-text-default flex items-center justify-center p-6">
      <div className="w-full max-w-md">
        <Link
          href="/"
          className="flex items-center gap-2 justify-center mb-8 group"
        >
          <div className="w-8 h-8 rounded-sharp bg-accent-pnl flex items-center justify-center border-brutal border-black">
            <Shield className="w-4 h-4 text-black" />
          </div>
          <span className="font-semibold text-lg text-text-hi group-hover:text-accent-pnl">
            Aegis
          </span>
        </Link>
        <CreateWalletCard />
        <p className="mt-6 text-center text-xs font-mono text-text-mut">
          Just looking?{" "}
          <Link href="/explore" className="text-accent-agent hover:underline">
            Explore demo portfolios
          </Link>
          .
        </p>
      </div>
    </div>
  );
}
