import Link from "next/link";
import { Shield } from "lucide-react";
import type { ReactNode } from "react";

interface AuthPageShellProps {
  title?: string;
  subtitle?: string;
  footer?: ReactNode;
  children: ReactNode;
}

export function AuthPageShell({
  title,
  subtitle,
  footer,
  children,
}: AuthPageShellProps) {
  return (
    <main className="relative min-h-screen overflow-hidden bg-bg text-text-default">
      <div className="absolute inset-0 bg-[linear-gradient(rgba(255,255,255,0.035)_1px,transparent_1px),linear-gradient(90deg,rgba(255,255,255,0.035)_1px,transparent_1px)] bg-[size:48px_48px]" />
      <section className="relative mx-auto flex min-h-screen w-full max-w-lg flex-col justify-center px-5 py-8">
        <div className="w-full">
          <Link
            href="/"
            className="mx-auto mb-8 inline-flex items-center gap-2 group"
          >
            <div className="flex h-9 w-9 items-center justify-center rounded-sharp border-brutal border-black bg-accent-agent">
              <Shield className="h-4 w-4 text-black" />
            </div>
            <span className="text-lg font-semibold text-text-hi group-hover:text-accent-agent">
              Aegis
            </span>
          </Link>

          {(title || subtitle) && (
            <div className="mb-5 space-y-2 text-center">
              {title && (
                <h1 className="font-mono text-3xl font-semibold text-text-hi sm:text-4xl">
                  {title}
                </h1>
              )}
              {subtitle && (
                <p className="mx-auto max-w-md font-mono text-sm leading-relaxed text-text-lo">
                  {subtitle}
                </p>
              )}
            </div>
          )}

          {children}
          {footer}
        </div>
      </section>
    </main>
  );
}
