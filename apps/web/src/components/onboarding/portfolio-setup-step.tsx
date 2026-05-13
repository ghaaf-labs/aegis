"use client";

import { Plus, Minus } from "lucide-react";
import { SUPPORTED_ASSETS } from "@aegis/shared";
import { cn } from "@/lib/utils";

interface Alloc { symbol: string; weight: number }

const DEFAULT_ALLOCATIONS: Alloc[] = [
  { symbol: "BTC", weight: 40 },
  { symbol: "ETH", weight: 30 },
  { symbol: "SOL", weight: 15 },
  { symbol: "BNB", weight: 15 },
];

interface Props {
  allocations: Alloc[];
  onChange: (a: Alloc[]) => void;
}

export function PortfolioSetupStep({ allocations, onChange }: Props) {
  const active = allocations.length > 0 ? allocations : DEFAULT_ALLOCATIONS;
  const total = active.reduce((sum, a) => sum + a.weight, 0);

  const toggle = (symbol: string) => {
    const exists = active.find((a) => a.symbol === symbol);
    if (exists) {
      onChange(active.filter((a) => a.symbol !== symbol));
    } else {
      onChange([...active, { symbol, weight: 0 }]);
    }
  };

  const setWeight = (symbol: string, weight: number) => {
    onChange(active.map((a) => (a.symbol === symbol ? { ...a, weight } : a)));
  };

  return (
    <div className="space-y-6">
      <div>
        <h2 className="text-xl font-bold text-white mb-1">Build your target allocation</h2>
        <p className="text-sm text-gray-400">
          Set target weights. Aegis will maintain these automatically.
        </p>
      </div>

      {/* Total indicator */}
      <div className="flex items-center justify-between">
        <span className="text-xs text-gray-500">Total allocation</span>
        <span
          className={cn(
            "text-sm font-bold",
            total === 100 ? "text-emerald-400" : total > 100 ? "text-red-400" : "text-yellow-400"
          )}
        >
          {total}% / 100%
        </span>
      </div>

      <div className="space-y-2">
        {SUPPORTED_ASSETS.map((asset) => {
          const alloc = active.find((a) => a.symbol === asset.symbol);
          const selected = Boolean(alloc);

          return (
            <div
              key={asset.symbol}
              className={cn(
                "flex items-center gap-3 p-3 rounded-xl border transition-all",
                selected ? "border-blue-500/30 bg-blue-500/5" : "border-white/5 bg-white/2"
              )}
            >
              <button
                onClick={() => toggle(asset.symbol)}
                className={cn(
                  "w-5 h-5 rounded border-2 flex items-center justify-center shrink-0 transition-all",
                  selected ? "border-blue-500 bg-blue-500" : "border-gray-600"
                )}
              >
                {selected && <div className="w-2 h-2 rounded-sm bg-white" />}
              </button>

              <div className="flex-1">
                <span className="text-sm font-semibold text-white font-mono">
                  {asset.symbol}
                </span>
                <span className="text-xs text-gray-500 ml-2">{asset.name}</span>
              </div>

              {selected && (
                <div className="flex items-center gap-2">
                  <button
                    onClick={() => setWeight(asset.symbol, Math.max(0, (alloc?.weight ?? 0) - 5))}
                    className="w-6 h-6 rounded-full bg-white/5 hover:bg-white/10 flex items-center justify-center"
                  >
                    <Minus className="w-3 h-3 text-gray-400" />
                  </button>
                  <span className="text-sm font-mono text-white w-10 text-center">
                    {alloc?.weight ?? 0}%
                  </span>
                  <button
                    onClick={() => setWeight(asset.symbol, Math.min(100, (alloc?.weight ?? 0) + 5))}
                    className="w-6 h-6 rounded-full bg-white/5 hover:bg-white/10 flex items-center justify-center"
                  >
                    <Plus className="w-3 h-3 text-gray-400" />
                  </button>
                </div>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
}
