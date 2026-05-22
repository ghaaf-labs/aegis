import { PegRuleEditor } from "@/components/peg/PegRuleEditor";

export const metadata = {
  title: "Peg defense · Aegis",
};

export default function PegSettingsPage() {
  return (
    <div className="max-w-[1400px] mx-auto space-y-6">
      <div>
        <h1 className="text-2xl font-mono font-semibold text-text-hi tracking-tight">
          Peg defense
        </h1>
        <p className="text-sm text-text-lo mt-1">
          Watch USDC, EURC, and USYC. If a stablecoin trades below your comfort
          level, Aegis can alert you or prepare a defensive review for approval.
        </p>
      </div>
      <PegRuleEditor />
    </div>
  );
}
