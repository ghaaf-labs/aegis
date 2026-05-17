"use client";

import { Shield } from "lucide-react";
import { GoalWizard } from "@/components/onboarding/goal-wizard";

export default function OnboardingPage() {
  return (
    <div className="min-h-screen bg-bg text-text-default flex items-center justify-center p-6">
      <div className="w-full max-w-2xl">
        <div className="flex items-center gap-2 justify-center mb-8">
          <div className="w-8 h-8 rounded-sharp bg-accent-pnl flex items-center justify-center border-brutal border-black">
            <Shield className="w-4 h-4 text-black" />
          </div>
          <span className="font-semibold text-lg text-text-hi">Aegis</span>
        </div>
        <GoalWizard />
      </div>
    </div>
  );
}
