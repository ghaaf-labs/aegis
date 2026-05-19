"use client";

import { Shield, Wallet, Brain, ShieldCheck } from "lucide-react";
import { GoalWizard } from "@/components/onboarding/goal-wizard";

export default function OnboardingPage() {
  return (
    <div className="min-h-screen bg-bg text-text-default flex items-start justify-center p-6 py-12">
      <div className="w-full max-w-2xl">
        <div className="flex items-center gap-2 justify-center mb-6">
          <div className="w-8 h-8 rounded-sharp bg-accent-pnl flex items-center justify-center border-brutal border-black">
            <Shield className="w-4 h-4 text-black" />
          </div>
          <span className="font-semibold text-lg text-text-hi">Aegis</span>
        </div>

        <div className="mb-6 text-center space-y-2">
          <h1 className="text-2xl font-semibold text-text-hi font-mono tracking-tight">
            Welcome to Aegis
          </h1>
          <p className="text-sm text-text-lo font-mono">
            Let&apos;s set your portfolio goal. The agent uses this every time
            it rebalances — you can update it later from Settings.
          </p>
        </div>

        <div className="grid grid-cols-3 gap-3 mb-8 text-center">
          <Step
            icon={<Wallet className="w-4 h-4" />}
            n={1}
            label="Set goal"
            active
          />
          <Step
            icon={<Brain className="w-4 h-4" />}
            n={2}
            label="Agent analyzes"
          />
          <Step
            icon={<ShieldCheck className="w-4 h-4" />}
            n={3}
            label="You approve trades"
          />
        </div>

        <GoalWizard />
      </div>
    </div>
  );
}

function Step({
  icon,
  n,
  label,
  active = false,
}: {
  icon: React.ReactNode;
  n: number;
  label: string;
  active?: boolean;
}) {
  const tone = active
    ? "border-accent-agent/40 bg-accent-agent/5 text-accent-agent"
    : "border-border-default bg-raised text-text-mut";
  return (
    <div className={`border-brutal ${tone} rounded-sharp p-3`}>
      <div className="flex items-center justify-center gap-1.5 mb-1">
        {icon}
        <span className="text-[10px] font-mono uppercase tracking-wider">
          Step {n}
        </span>
      </div>
      <p className="text-xs font-mono">{label}</p>
    </div>
  );
}
