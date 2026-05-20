"use client";

import { Shield } from "lucide-react";
import { GoalWizard } from "@/components/onboarding/goal-wizard";

export default function OnboardingPage() {
  return (
    <div className="min-h-screen bg-bg text-text-default flex items-start justify-center p-6 py-12">
      <div className="w-full max-w-2xl">
        <div className="flex items-center gap-2 justify-center mb-6">
          <div className="w-8 h-8 rounded-sharp bg-accent-agent flex items-center justify-center border-brutal border-black">
            <Shield className="w-4 h-4 text-black" />
          </div>
          <span className="font-semibold text-lg text-text-hi">Aegis</span>
        </div>

        <div className="mb-8 text-center space-y-3">
          <h1 className="text-2xl font-semibold text-text-hi font-mono tracking-tight">
            Welcome to Aegis
          </h1>
          <p className="text-sm text-text-lo font-mono">
            Let&apos;s set your portfolio goal. The agent uses this every time
            it rebalances — you can update it later from Settings.
          </p>
          <p className="text-xs font-mono">
            <span className="text-accent-agent">Set goal</span>
            <span className="text-text-mut mx-2">·</span>
            <span className="text-text-mut">Agent analyzes</span>
            <span className="text-text-mut mx-2">·</span>
            <span className="text-text-mut">You approve trades</span>
          </p>
        </div>

        <GoalWizard />
      </div>
    </div>
  );
}
