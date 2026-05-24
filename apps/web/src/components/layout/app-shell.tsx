"use client";

import { useEffect, useRef, useState } from "react";
import { usePathname } from "next/navigation";
import Link from "next/link";
import { CircleAlert, Menu, Wallet, X } from "lucide-react";
import { Sidebar } from "@/components/layout/sidebar";
import { Header } from "@/components/layout/header";
import { ErrorBoundary } from "@/components/error-boundary";
import { isProtectedAppPath, safeNextPath } from "@/lib/auth-routing";
import { usePortfolioStore } from "@/stores/portfolio";

const DRAWER_TITLE_ID = "mobile-nav-title";

export function AppShell({ children }: { children: React.ReactNode }) {
  const [open, setOpen] = useState(false);
  const pathname = usePathname();
  const sessionActive = usePortfolioStore((s) => s.sessionActive);
  const sessionResolved = usePortfolioStore((s) => s.sessionResolved);
  const wallet = usePortfolioStore((s) => s.wallet);
  const walletPending = sessionActive && !wallet;
  const protectedAppPath = isProtectedAppPath(pathname);
  const showAuthFrame = protectedAppPath && sessionResolved && !sessionActive;
  const mobileAction = !sessionResolved
    ? null
    : walletPending
      ? {
          href: "/onboarding",
          label: "Finish account",
          tone: "warn" as const,
        }
      : sessionActive
        ? null
        : {
            href: authHref("/login", pathname),
            label: "Continue",
            tone: "agent" as const,
          };

  const toggleRef = useRef<HTMLButtonElement>(null);
  const drawerRef = useRef<HTMLDivElement>(null);

  // Close drawer and restore focus on route change.
  useEffect(() => {
    setOpen(false);
  }, [pathname]);

  // Escape key closes the drawer.
  useEffect(() => {
    if (!open) return;
    const handleKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        setOpen(false);
        toggleRef.current?.focus();
      }
    };
    document.addEventListener("keydown", handleKey);
    return () => document.removeEventListener("keydown", handleKey);
  }, [open]);

  // Focus trap: cycle focus within the drawer while open.
  useEffect(() => {
    if (!open || !drawerRef.current) return;
    const drawer = drawerRef.current;
    const focusable = drawer.querySelectorAll<HTMLElement>(
      'a[href], button:not([disabled]), [tabindex]:not([tabindex="-1"])',
    );
    if (focusable.length > 0) focusable[0]?.focus();

    const handleTab = (e: KeyboardEvent) => {
      if (e.key !== "Tab") return;
      const items = Array.from(
        drawer.querySelectorAll<HTMLElement>(
          'a[href], button:not([disabled]), [tabindex]:not([tabindex="-1"])',
        ),
      );
      if (items.length === 0) return;
      const first = items[0]!;
      const last = items[items.length - 1]!;
      if (e.shiftKey) {
        if (document.activeElement === first) {
          e.preventDefault();
          last.focus();
        }
      } else {
        if (document.activeElement === last) {
          e.preventDefault();
          first.focus();
        }
      }
    };
    document.addEventListener("keydown", handleTab);
    return () => document.removeEventListener("keydown", handleTab);
  }, [open]);

  // Lock body scroll while drawer is open.
  useEffect(() => {
    if (open) {
      document.body.style.overflow = "hidden";
    } else {
      document.body.style.overflow = "";
    }
    return () => {
      document.body.style.overflow = "";
    };
  }, [open]);

  // Restore focus to toggle when drawer closes.
  const prevOpen = useRef(false);
  useEffect(() => {
    if (prevOpen.current && !open) {
      toggleRef.current?.focus();
    }
    prevOpen.current = open;
  }, [open]);

  if (showAuthFrame) {
    return (
      <AuthStateScreen
        href={authHref("/login", pathname, "session_required")}
      />
    );
  }

  return (
    <div className="flex h-screen bg-bg text-text-default overflow-hidden">
      {/* Desktop sidebar — hidden until the content area has enough width. */}
      <div className="hidden xl:flex">
        <Sidebar />
      </div>

      {/* Mobile drawer — proper dialog with focus trap and Escape key.
          Conditionally rendered so closed state is absent from the a11y tree. */}
      {open && (
        <>
          {/* Backdrop: aria-hidden so AT skips it. */}
          <div
            aria-hidden="true"
            className="fixed inset-0 z-40 bg-black/60 xl:hidden"
            onClick={() => setOpen(false)}
          />
          {/* Drawer panel as a dialog. */}
          <div
            id="mobile-nav-dialog"
            ref={drawerRef}
            role="dialog"
            aria-modal="true"
            aria-labelledby={DRAWER_TITLE_ID}
            className="fixed inset-y-0 left-0 z-50 w-[min(340px,100vw)] xl:hidden"
          >
            {/* Visually-hidden title for the dialog. */}
            <span id={DRAWER_TITLE_ID} className="sr-only">
              Navigation
            </span>
            <Sidebar onClose={() => setOpen(false)} />
          </div>
        </>
      )}

      <div className="flex flex-col flex-1 min-w-0 overflow-hidden">
        <div className="flex min-h-[58px] items-center gap-2 border-b border-border-default bg-surface px-3 py-2 xl:hidden">
          <button
            ref={toggleRef}
            type="button"
            id="mobile-nav-toggle"
            onClick={() => setOpen((v) => !v)}
            aria-label={open ? "Close navigation" : "Open navigation"}
            aria-expanded={open}
            aria-controls="mobile-nav-dialog"
            className="touch-target inline-flex min-h-11 min-w-11 items-center justify-center rounded-sharp border-brutal border-border-default bg-raised"
          >
            {open ? <X className="w-4 h-4" /> : <Menu className="w-4 h-4" />}
          </button>
          <div className="min-w-0 flex-1">
            <p className="font-mono text-[10px] uppercase tracking-widest text-text-mut">
              Aegis Console
            </p>
            <p className="truncate font-mono text-sm font-semibold text-text-hi">
              {mobileTitle(pathname)}
            </p>
          </div>
          {mobileAction && (
            <Link
              href={mobileAction.href}
              className={
                mobileAction.tone === "warn"
                  ? "touch-target inline-flex min-h-11 shrink-0 items-center gap-1.5 rounded-sharp border border-warn/40 bg-warn/5 px-2.5 font-mono text-[10px] uppercase tracking-widest text-warn"
                  : "touch-target inline-flex min-h-11 shrink-0 items-center gap-1.5 rounded-sharp border border-black bg-accent-agent px-2.5 font-mono text-[10px] font-semibold uppercase tracking-widest text-black shadow-brutal-sm"
              }
            >
              {mobileAction.tone === "warn" ? (
                <Wallet className="h-3.5 w-3.5" />
              ) : (
                <CircleAlert className="h-3.5 w-3.5" />
              )}
              {mobileAction.label}
            </Link>
          )}
        </div>
        <div className="hidden xl:block">
          <Header />
        </div>
        <main className="flex-1 overflow-y-auto p-4 xl:p-6 scrollbar-thin">
          <ErrorBoundary>{children}</ErrorBoundary>
        </main>
      </div>
    </div>
  );
}

function mobileTitle(pathname: string) {
  const first = pathname.split("/").filter(Boolean)[0] ?? "dashboard";
  const labels: Record<string, string> = {
    "agent-logs": "Agent Logs",
    "agent-studio": "Agent Studio",
    analytics: "Analytics",
    dashboard: "Dashboard",
    explore: "Explore Demos",
    help: "Help",
    leaderboard: "Leaderboard",
    portfolio: "Portfolio",
    rebalance: "Rebalance Review",
    settings: "Settings",
    transactions: "Transactions",
    wallet: "Wallet",
    wallets: "Wallets",
    "tax-center": "Tax Center",
  };
  return labels[first] ?? titleCase(first);
}

function titleCase(value: string) {
  return value
    .split("-")
    .filter(Boolean)
    .map((part) => `${part.slice(0, 1).toUpperCase()}${part.slice(1)}`)
    .join(" ");
}

function AuthStateScreen({ href }: { href: string }) {
  return (
    <main className="flex min-h-screen items-center justify-center bg-bg px-5 py-10 text-text-default">
      <section className="w-full max-w-[420px] border-brutal border-border-default bg-surface p-5 shadow-brutal">
        <div className="flex items-center gap-3">
          <span
            aria-hidden="true"
            className="flex h-9 w-9 shrink-0 items-center justify-center rounded-sharp border border-accent-agent/50 bg-accent-agent/10 text-accent-agent"
          >
            <CircleAlert className="h-4 w-4" />
          </span>
          <div className="min-w-0">
            <p className="font-mono text-[10px] uppercase tracking-widest text-text-mut">
              Aegis Console
            </p>
            <h1 className="font-mono text-lg font-semibold text-text-hi">
              Continue with email
            </h1>
          </div>
        </div>
        <p className="mt-4 text-sm leading-6 text-text-lo">
          Your session is not active in this browser. Sign in to open the app.
        </p>
        <Link
          href={href}
          className="mt-5 inline-flex min-h-11 w-full items-center justify-center rounded-sharp border border-black bg-accent-agent px-4 text-center font-mono text-sm font-semibold text-black shadow-brutal-sm hover:shadow-brutal"
        >
          Continue
        </Link>
      </section>
    </main>
  );
}

function authHref(path: "/login", next: string, reason?: string) {
  const params = new URLSearchParams();
  const safeNext = safeNextPath(next);
  if (safeNext) params.set("next", safeNext);
  if (reason) params.set("reason", reason);
  const query = params.toString();
  return query ? `${path}?${query}` : path;
}
