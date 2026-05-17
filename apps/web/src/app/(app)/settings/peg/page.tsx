import { PegRuleEditor } from "@/components/peg/PegRuleEditor";

export const metadata = {
  title: "Peg defense · Aegis",
};

export default function PegSettingsPage() {
  return (
    <div className="flex flex-col gap-6 max-w-3xl">
      <header>
        <h1 className="text-2xl font-bold tracking-tight">Peg defense</h1>
        <p className="text-sm text-text-mut">
          Watch USDC, EURC, and USYC. When a peg slips, the monitor either
          alerts you, drafts a defensive rebalance, or — on Pro / Business —
          executes it automatically. One-tap pause keeps you in control.
        </p>
      </header>
      <PegRuleEditor />
    </div>
  );
}
