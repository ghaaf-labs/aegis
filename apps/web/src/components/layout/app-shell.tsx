"use client";

import { useEffect, useState } from "react";
import { usePathname } from "next/navigation";
import { Menu, X } from "lucide-react";
import { Sidebar } from "@/components/layout/sidebar";
import { Header } from "@/components/layout/header";
import { ErrorBoundary } from "@/components/error-boundary";

export function AppShell({ children }: { children: React.ReactNode }) {
  const [open, setOpen] = useState(false);
  const pathname = usePathname();

  // Close the drawer on every route change so a mobile user doesn't have to
  // dismiss it manually after each tap.
  useEffect(() => {
    setOpen(false);
  }, [pathname]);

  return (
    <div className="flex h-screen bg-[#030712] text-white overflow-hidden">
      {/* Desktop sidebar — hidden on small screens. */}
      <div className="hidden md:flex">
        <Sidebar />
      </div>

      {/* Mobile drawer — fixed overlay, slides in from the left. */}
      {open && (
        <button
          type="button"
          aria-label="Close navigation"
          className="md:hidden fixed inset-0 z-40 bg-black/60"
          onClick={() => setOpen(false)}
        />
      )}
      <div
        className={
          "md:hidden fixed inset-y-0 left-0 z-50 transition-transform duration-200 " +
          (open ? "translate-x-0" : "-translate-x-full")
        }
        aria-hidden={!open}
      >
        <Sidebar />
      </div>

      <div className="flex flex-col flex-1 min-w-0 overflow-hidden">
        <div className="flex items-center md:hidden border-b border-white/5 bg-surface px-3 py-2 gap-2">
          <button
            type="button"
            onClick={() => setOpen((v) => !v)}
            aria-label={open ? "Close navigation" : "Open navigation"}
            aria-expanded={open}
            className="min-h-[44px] min-w-[44px] inline-flex items-center justify-center border-brutal border-border-default rounded-sharp bg-raised"
          >
            {open ? <X className="w-4 h-4" /> : <Menu className="w-4 h-4" />}
          </button>
          <span className="font-mono font-semibold text-sm tracking-tight">
            Aegis
          </span>
        </div>
        <Header />
        <main className="flex-1 overflow-y-auto p-4 md:p-6 scrollbar-thin">
          <ErrorBoundary>{children}</ErrorBoundary>
        </main>
      </div>
    </div>
  );
}
