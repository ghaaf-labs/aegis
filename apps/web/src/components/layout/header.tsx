"use client";

import { Bell, Search } from "lucide-react";
import { Button } from "@/components/ui/button";
import { usePortfolioStore } from "@/stores/portfolio";
import { formatCurrency, formatPercent, changeColor } from "@/lib/utils";

export function Header() {
  const portfolio = usePortfolioStore((s) => s.portfolio);

  return (
    <header className="flex items-center justify-between px-6 py-3.5 border-b border-white/5 bg-gray-950/30 backdrop-blur-sm shrink-0">
      {/* Portfolio quick stats */}
      {portfolio && (
        <div className="flex items-center gap-6">
          <div>
            <p className="text-xs text-gray-500">Portfolio Value</p>
            <p className="text-sm font-semibold text-white">
              {formatCurrency(portfolio.totalValueUsd)}
            </p>
          </div>
          <div>
            <p className="text-xs text-gray-500">All-time P&L</p>
            <p className={`text-sm font-semibold ${changeColor(portfolio.totalPnlUsd)}`}>
              {formatCurrency(portfolio.totalPnlUsd)} ({formatPercent(portfolio.totalPnlPct)})
            </p>
          </div>
        </div>
      )}

      {/* Right actions */}
      <div className="flex items-center gap-2 ml-auto">
        <Button variant="ghost" size="icon" className="text-gray-500 hover:text-gray-300 w-8 h-8">
          <Search className="w-4 h-4" />
        </Button>
        <Button variant="ghost" size="icon" className="text-gray-500 hover:text-gray-300 w-8 h-8 relative">
          <Bell className="w-4 h-4" />
          <span className="absolute top-1 right-1 w-1.5 h-1.5 rounded-full bg-blue-500" />
        </Button>
        <div className="w-7 h-7 rounded-full bg-gradient-to-br from-blue-500 to-violet-600 flex items-center justify-center ml-1">
          <span className="text-xs font-semibold text-white">A</span>
        </div>
      </div>
    </header>
  );
}
