"use client";

import { useState } from "react";
import { motion, AnimatePresence } from "framer-motion";
import { useRouter } from "next/navigation";
import { Shield, ChevronRight, ChevronLeft } from "lucide-react";
import { Button } from "@/components/ui/button";
import { RiskToleranceStep } from "@/components/onboarding/risk-tolerance-step";
import { PortfolioSetupStep } from "@/components/onboarding/portfolio-setup-step";
import { ReviewStep } from "@/components/onboarding/review-step";

const STEPS = ["Risk Profile", "Portfolio Setup", "Review"];

export default function OnboardingPage() {
  const [step, setStep] = useState(0);
  const [formData, setFormData] = useState({
    riskTolerance: "moderate" as "conservative" | "moderate" | "aggressive",
    investmentHorizonMonths: 12,
    initialAllocations: [] as Array<{ symbol: string; weight: number }>,
  });
  const router = useRouter();

  const update = (patch: Partial<typeof formData>) =>
    setFormData((prev) => ({ ...prev, ...patch }));

  const handleFinish = () => {
    // In production: POST /auth/register + POST /portfolios
    router.push("/dashboard");
  };

  return (
    <div className="min-h-screen bg-[#030712] text-white flex items-center justify-center p-6">
      {/* Background orbs */}
      <div className="fixed inset-0 pointer-events-none">
        <div className="absolute top-[-10%] left-[20%] w-[400px] h-[400px] bg-blue-600/15 rounded-full blur-[100px]" />
        <div className="absolute bottom-[10%] right-[15%] w-[300px] h-[300px] bg-violet-600/15 rounded-full blur-[80px]" />
      </div>

      <div className="relative z-10 w-full max-w-2xl">
        {/* Logo */}
        <div className="flex items-center gap-2 justify-center mb-8">
          <div className="w-8 h-8 rounded-lg bg-gradient-to-br from-blue-500 to-violet-600 flex items-center justify-center">
            <Shield className="w-4 h-4 text-white" />
          </div>
          <span className="font-bold text-lg">Aegis</span>
        </div>

        {/* Step indicator */}
        <div className="flex items-center justify-center gap-2 mb-8">
          {STEPS.map((label, i) => (
            <div key={label} className="flex items-center gap-2">
              <div
                className={`flex items-center gap-1.5 px-3 py-1 rounded-full text-xs font-medium transition-all ${
                  i === step
                    ? "bg-blue-600 text-white"
                    : i < step
                      ? "bg-blue-600/20 text-blue-400"
                      : "bg-white/5 text-gray-500"
                }`}
              >
                <span
                  className={`w-4 h-4 rounded-full flex items-center justify-center text-[10px] ${
                    i < step ? "bg-blue-400/30" : "bg-white/10"
                  }`}
                >
                  {i + 1}
                </span>
                {label}
              </div>
              {i < STEPS.length - 1 && (
                <div
                  className={`w-8 h-px ${i < step ? "bg-blue-500/50" : "bg-white/10"}`}
                />
              )}
            </div>
          ))}
        </div>

        {/* Step content */}
        <div className="rounded-2xl border border-white/10 bg-gray-950/80 backdrop-blur-sm p-8">
          <AnimatePresence mode="wait">
            <motion.div
              key={step}
              initial={{ opacity: 0, x: 20 }}
              animate={{ opacity: 1, x: 0 }}
              exit={{ opacity: 0, x: -20 }}
              transition={{ duration: 0.25 }}
            >
              {step === 0 && (
                <RiskToleranceStep
                  value={formData.riskTolerance}
                  horizonMonths={formData.investmentHorizonMonths}
                  onChange={(v) => update(v)}
                />
              )}
              {step === 1 && (
                <PortfolioSetupStep
                  allocations={formData.initialAllocations}
                  onChange={(allocations) =>
                    update({ initialAllocations: allocations })
                  }
                />
              )}
              {step === 2 && <ReviewStep formData={formData} />}
            </motion.div>
          </AnimatePresence>
        </div>

        {/* Navigation */}
        <div className="flex justify-between mt-6">
          <Button
            variant="ghost"
            onClick={() => setStep((s) => s - 1)}
            disabled={step === 0}
            className="text-gray-400 hover:text-white"
          >
            <ChevronLeft className="w-4 h-4 mr-1" />
            Back
          </Button>
          {step < STEPS.length - 1 ? (
            <Button
              onClick={() => setStep((s) => s + 1)}
              className="bg-blue-600 hover:bg-blue-500"
            >
              Continue
              <ChevronRight className="w-4 h-4 ml-1" />
            </Button>
          ) : (
            <Button
              onClick={handleFinish}
              className="bg-blue-600 hover:bg-blue-500"
            >
              Launch Aegis
              <ChevronRight className="w-4 h-4 ml-1" />
            </Button>
          )}
        </div>
      </div>
    </div>
  );
}
