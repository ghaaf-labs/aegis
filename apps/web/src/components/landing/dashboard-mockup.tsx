"use client";

import { useState } from "react";
import Link from "next/link";
import { motion } from "framer-motion";
import { cn } from "@/lib/utils";

export function DashboardMockup() {
  const [mockApproved, setMockApproved] = useState(false);

  return (
    <motion.section
      initial={{ opacity: 0, y: 40 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ delay: 0.5, duration: 0.7 }}
      className="relative z-10 max-w-6xl mx-auto px-6 py-20"
    >
      <div className="border-brutal border-border-default bg-surface shadow-brutal">
        {/* Browser chrome */}
        <div className="flex items-center justify-between px-4 py-3 border-b border-border-default bg-raised">
          <div className="flex items-center gap-2">
            <span className="text-xs text-text-mut font-mono">dashboard</span>
            <span className="text-[10px] font-mono uppercase tracking-widest text-text-mut border border-border-default px-1.5 py-0.5 rounded-sharp">
              Illustrative
            </span>
          </div>
          <div className="flex items-center gap-2">
            <span className="flex items-center gap-1 text-[10px] font-mono text-accent-agent">
              <span className="w-1.5 h-1.5 rounded-full bg-accent-agent animate-pulse" />
              arc
            </span>
            <span className="flex items-center gap-1 text-[10px] font-mono text-accent-agent/60">
              <span className="w-1.5 h-1.5 rounded-full bg-accent-agent/60" />
              base
            </span>
          </div>
        </div>

        <div className="p-6 grid grid-cols-1 md:grid-cols-[1fr_280px] gap-6">
          {/* Left: portfolio table */}
          <div className="space-y-4">
            <div className="flex items-end justify-between">
              <div>
                <p className="text-xs font-mono text-text-lo">
                  Portfolio value
                </p>
                <p className="text-3xl font-mono font-bold text-text-hi tabular-nums">
                  $24,180
                  <span className="text-lg">.00</span>
                </p>
              </div>
              <span className="text-sm font-mono text-accent-pnl font-semibold">
                ↑ +2.4% today
              </span>
            </div>

            <div className="border-brutal border-border-default overflow-hidden">
              <table className="w-full text-xs font-mono">
                <thead className="border-b border-border-default bg-raised text-text-lo">
                  <tr>
                    <th className="text-left px-3 py-2 font-medium">Asset</th>
                    <th className="text-right px-3 py-2 font-medium">Value</th>
                    <th className="text-right px-3 py-2 font-medium">Alloc</th>
                    <th className="text-right px-3 py-2 font-medium">Yield</th>
                  </tr>
                </thead>
                <tbody>
                  {[
                    {
                      asset: "USDC",
                      chain: "Arc",
                      value: "$15,830",
                      alloc: "65%",
                      yield: null,
                    },
                    {
                      asset: "USDC",
                      chain: "Base",
                      value: "$8,350",
                      alloc: "35%",
                      yield: null,
                    },
                  ].map((row) => (
                    <tr
                      key={`${row.asset}-${row.chain}`}
                      className="border-b border-border-default last:border-b-0"
                    >
                      <td className="px-3 py-2.5 text-text-hi font-semibold">
                        {row.asset}
                        <span className="ml-1.5 text-[10px] text-text-mut font-normal">
                          {row.chain}
                        </span>
                      </td>
                      <td className="px-3 py-2.5 text-right tabular-nums text-text-default">
                        {row.value}
                      </td>
                      <td className="px-3 py-2.5 text-right tabular-nums text-text-lo">
                        {row.alloc}
                      </td>
                      <td className="px-3 py-2.5 text-right">
                        {row.yield ? (
                          <span className="text-accent-pnl">{row.yield}</span>
                        ) : (
                          <span className="text-text-mut">—</span>
                        )}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>

            {/* Approval preview interaction — demo only, no on-chain action */}
            <div
              className="flex items-center gap-3 p-3 border-brutal border-accent-agent/30 bg-accent-agent/5"
              role="status"
              aria-live="polite"
            >
              {mockApproved ? (
                <span className="flex items-center gap-2 text-xs font-mono text-text-lo">
                  This is a demo preview — connect a wallet to run real
                  rebalances →{" "}
                  <Link href="/login" className="underline text-accent-agent">
                    Get started
                  </Link>
                </span>
              ) : (
                <>
                  <span className="text-xs font-mono text-text-lo flex-1">
                    Agent: rebalance USDC across Arc + Base · fee: $0.12 USDC
                  </span>
                  <button
                    type="button"
                    onClick={() => setMockApproved(true)}
                    className="shrink-0 px-3 py-1.5 bg-accent-agent text-black text-xs font-mono font-semibold border-brutal border-black rounded-sharp hover:opacity-90 transition-opacity"
                    aria-label="Preview approval (demo only)"
                  >
                    Preview approval
                  </button>
                </>
              )}
            </div>
          </div>

          {/* Right: allocation + agent card */}
          <div className="space-y-4">
            <div className="border-brutal border-border-default bg-raised p-4 space-y-3">
              <p className="text-xs font-mono text-text-lo">Allocation</p>
              {[
                { label: "USDC · Arc", pct: 65, color: "bg-accent-agent" },
                {
                  label: "USDC · Base",
                  pct: 35,
                  color: "bg-accent-agent/50",
                },
              ].map((row) => (
                <div key={row.label} className="space-y-1">
                  <div className="flex justify-between text-[11px] font-mono">
                    <span className="text-text-default">{row.label}</span>
                    <span className="text-text-lo tabular-nums">
                      {row.pct}%
                    </span>
                  </div>
                  <div className="h-1.5 bg-border-default rounded-full overflow-hidden">
                    <div
                      className={cn("h-full rounded-full", row.color)}
                      style={{ width: `${row.pct}%` }}
                    />
                  </div>
                </div>
              ))}
            </div>

            <div className="border-brutal border-accent-agent/30 bg-accent-agent/5 p-4 space-y-2">
              <div className="flex items-center justify-between">
                <span className="text-[10px] font-mono text-accent-agent uppercase tracking-widest">
                  Last decision
                </span>
                <span className="text-[10px] font-mono text-text-mut">
                  2m ago
                </span>
              </div>
              <div className="flex items-center gap-1.5">
                <span className="text-[10px] font-mono px-1.5 py-0.5 border border-accent-agent/30 text-accent-agent rounded-sharp">
                  deepseek/deepseek-v4-flash
                </span>
                <div
                  className="flex gap-0.5"
                  role="progressbar"
                  aria-valuenow={82}
                  aria-valuemin={0}
                  aria-valuemax={100}
                  aria-valuetext="confidence 82%"
                >
                  {[1, 2, 3, 4].map((d) => (
                    <span
                      key={d}
                      className="w-2 h-2 rounded-full bg-accent-agent"
                    />
                  ))}
                  <span className="w-2 h-2 rounded-full bg-border-default" />
                </div>
                <span className="text-[10px] font-mono text-text-mut">82%</span>
              </div>
              <p className="text-xs text-text-lo leading-relaxed">
                &ldquo;Rebalance $800 USDC from Arc to Base — current regime is
                consolidating, Base liquidity is deeper at this horizon.&rdquo;
              </p>
            </div>
          </div>
        </div>
      </div>
    </motion.section>
  );
}
