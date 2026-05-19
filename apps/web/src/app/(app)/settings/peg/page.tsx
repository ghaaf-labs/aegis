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
          Watch USDC, EURC, and USYC. When a peg slips, the monitor either
          alerts you, drafts a defensive rebalance, or — on Pro / Business —
          executes it automatically. One-tap pause keeps you in control.
        </p>
      </div>
      <PegRuleEditor />
    </div>
  );
}
