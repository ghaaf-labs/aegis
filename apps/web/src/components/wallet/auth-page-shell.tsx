import Link from "next/link";
import { ArrowRight, KeyRound, Shield, WalletCards } from "lucide-react";
import type { ReactNode } from "react";

interface AuthPageShellProps {
  title: string;
  subtitle: string;
  mode: "login" | "signup";
  footer?: ReactNode;
  children: ReactNode;
}

export function AuthPageShell({
  title,
  subtitle,
  mode,
  footer,
  children,
}: AuthPageShellProps) {
  const firstLabel = mode === "login" ? "Restore" : "Create";
  const detailSteps =
    mode === "login"
      ? [
          [
            "1",
            "Email code",
            "Aegis proves inbox control before issuing a fresh session.",
          ],
          [
            "2",
            "Session restore",
            "Returning wallets reuse the existing Circle wallet record.",
          ],
          [
            "3",
            "Execution gate",
            "Portfolio actions stay blocked until Arc + Base wallets exist.",
          ],
        ]
      : [
          [
            "1",
            "Email code",
            "Aegis proves inbox control before issuing a session.",
          ],
          [
            "2",
            "Circle PIN",
            "New wallets still use Circle's hosted PIN ceremony.",
          ],
          [
            "3",
            "Execution gate",
            "Portfolio actions stay blocked until Arc + Base wallets exist.",
          ],
        ];
  return (
    <main className="relative min-h-screen overflow-hidden bg-bg text-text-default">
      <div className="absolute inset-0 bg-[linear-gradient(rgba(255,255,255,0.035)_1px,transparent_1px),linear-gradient(90deg,rgba(255,255,255,0.035)_1px,transparent_1px)] bg-[size:48px_48px]" />
      <section className="relative mx-auto grid min-h-screen w-full max-w-6xl items-center gap-8 px-5 py-8 lg:grid-cols-[minmax(0,480px)_minmax(0,1fr)] lg:px-8">
        <div className="w-full">
          <Link href="/" className="mb-8 inline-flex items-center gap-2 group">
            <div className="flex h-9 w-9 items-center justify-center rounded-sharp border-brutal border-black bg-accent-agent">
              <Shield className="h-4 w-4 text-black" />
            </div>
            <span className="text-lg font-semibold text-text-hi group-hover:text-accent-agent">
              Aegis
            </span>
          </Link>

          <div className="mb-5 space-y-2">
            <p className="font-mono text-[10px] uppercase tracking-widest text-accent-agent">
              Wallet session
            </p>
            <h1 className="font-mono text-3xl font-semibold tracking-tight text-text-hi sm:text-4xl">
              {title}
            </h1>
            <p className="max-w-lg font-mono text-sm leading-relaxed text-text-lo">
              {subtitle}
            </p>
          </div>

          {children}
          {footer}
        </div>

        <aside className="hidden border-brutal border-border-default bg-surface p-6 shadow-brutal lg:block">
          <div className="mb-5 flex items-center justify-between border-b border-border-default pb-4">
            <div>
              <p className="font-mono text-[10px] uppercase tracking-widest text-text-mut">
                Session path
              </p>
              <h2 className="mt-1 font-mono text-lg font-semibold text-text-hi">
                Email alone is never enough
              </h2>
            </div>
            <div className="flex h-10 w-10 items-center justify-center rounded-sharp border-brutal border-black bg-accent-agent">
              <KeyRound className="h-5 w-5 text-black" />
            </div>
          </div>

          <AuthFlowSvg firstLabel={firstLabel} mode={mode} />

          <div className="mt-6 grid gap-3 font-mono text-xs">
            {detailSteps.map(([n, label, copy]) => (
              <div
                key={n}
                className="grid grid-cols-[32px_1fr] gap-3 border border-border-default bg-bg p-3"
              >
                <div className="flex h-8 w-8 items-center justify-center rounded-sharp border border-accent-agent/50 bg-accent-agent/10 text-accent-agent">
                  {n}
                </div>
                <div>
                  <p className="font-semibold text-text-hi">{label}</p>
                  <p className="mt-1 leading-relaxed text-text-lo">{copy}</p>
                </div>
              </div>
            ))}
          </div>
        </aside>
      </section>
    </main>
  );
}

function AuthFlowSvg({
  firstLabel,
  mode,
}: {
  firstLabel: string;
  mode: "login" | "signup";
}) {
  const steps =
    mode === "login"
      ? [
          { x: 52, y: 72, label: "Email", icon: "@" },
          { x: 192, y: 72, label: "Code", icon: "••••••" },
          { x: 332, y: 72, label: "Session", icon: "OK" },
          { x: 472, y: 72, label: firstLabel, icon: "$" },
        ]
      : [
          { x: 52, y: 72, label: "Email", icon: "@" },
          { x: 192, y: 72, label: "Code", icon: "••••••" },
          { x: 332, y: 72, label: "PIN", icon: "••••" },
          { x: 472, y: 72, label: firstLabel, icon: "$" },
        ];

  return (
    <svg
      viewBox="0 0 540 180"
      role="img"
      aria-label="Aegis wallet authentication flow"
      className="h-auto w-full border border-border-default bg-bg"
    >
      <defs>
        <pattern
          id="auth-grid"
          width="18"
          height="18"
          patternUnits="userSpaceOnUse"
        >
          <path d="M18 0H0V18" fill="none" stroke="#242424" strokeWidth="1" />
        </pattern>
      </defs>
      <rect width="540" height="180" fill="url(#auth-grid)" />
      <path
        d="M90 72H154M230 72H294M370 72H434"
        fill="none"
        stroke="#55D7FF"
        strokeWidth="3"
        strokeLinecap="square"
      />
      {steps.map((step) => (
        <g key={step.label}>
          <rect
            x={step.x - 38}
            y={step.y - 38}
            width="76"
            height="76"
            fill="#111"
            stroke="#3A3A3A"
            strokeWidth="2"
          />
          <rect
            x={step.x - 29}
            y={step.y - 29}
            width="58"
            height="34"
            fill="#55D7FF"
            stroke="#050505"
            strokeWidth="3"
          />
          <text
            x={step.x}
            y={step.y - 7}
            textAnchor="middle"
            fontFamily="monospace"
            fontSize={step.icon.length > 1 ? 10 : 18}
            fontWeight="700"
            fill="#050505"
          >
            {step.icon}
          </text>
          <text
            x={step.x}
            y={step.y + 50}
            textAnchor="middle"
            fontFamily="monospace"
            fontSize="12"
            fill="#E8E8E8"
          >
            {step.label}
          </text>
        </g>
      ))}
      <g transform="translate(432 118)">
        <rect
          width="82"
          height="26"
          fill="#151515"
          stroke="#38E27D"
          strokeWidth="2"
        />
        <WalletCards x="8" y="6" width="14" height="14" color="#38E27D" />
        <text x="30" y="17" fontFamily="monospace" fontSize="10" fill="#38E27D">
          WALLET
        </text>
      </g>
      <ArrowRight x="248" y="126" width="24" height="24" color="#55D7FF" />
    </svg>
  );
}
