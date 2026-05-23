import type { Metadata } from "next";
import Image from "next/image";
import Link from "next/link";
import { Github, Globe, Linkedin } from "lucide-react";
import { LandingShell } from "@/components/layout/landing-shell";
import { BrutalPill } from "@aegis/ui";

export const metadata: Metadata = {
  title: "About — Aegis",
  description:
    "Meet the team behind Aegis — an adaptive stablecoin portfolio agent built on Circle's stack.",
};

const TEAM = [
  {
    name: "Mahdi Zarrintareh",
    handle: "malivix",
    role: "Staff Engineer",
    avatar: "/team/malivix.png",
    linkedin: "https://www.linkedin.com/in/malivix/",
    github: "https://github.com/malivix",
    website: null,
  },
  {
    name: "Mohammad Jalili",
    handle: "mohijalili",
    role: "Staff Engineer",
    avatar: "/team/mohijalili.png",
    linkedin: "https://www.linkedin.com/in/mohammadjalili/",
    github: "https://github.com/mohijalili",
    website: "https://mohism.io",
  },
];

const STATS = [
  { value: "2", label: "Builders" },
  { value: "6", label: "Circle APIs" },
  { value: "2", label: "Chains" },
  { value: "May '26", label: "Shipped" },
];

const STACK = [
  "Circle Wallets",
  "Circle CCTP V2",
  "Circle Paymaster",
  "Circle Gateway",
  "Circle StableFX",
  "Circle Nanopayments",
  "Arc",
  "Base",
  "OpenRouter",
  "Next.js 15",
  "Rust · Axum",
];

export default function AboutPage() {
  return (
    <LandingShell>
      {/* ── Hero ─────────────────────────────────────────────────── */}
      <header className="pt-6 pb-16">
        <BrutalPill tone="agent" className="mb-6">
          Agora Agents Hackathon · May 2026
        </BrutalPill>

        <h1 className="text-6xl sm:text-7xl font-black tracking-tighter text-text-hi leading-none mb-6">
          WE BUILD
          <br />
          <span className="text-accent-agent">AEGIS</span>
        </h1>

        <p className="text-sm font-mono text-text-lo max-w-lg leading-relaxed">
          An adaptive stablecoin portfolio agent. The user steers — approves
          every move. A multi-model AI executes on Arc + Base through
          Circle&apos;s full stack.
        </p>

        {/* stats row */}
        <div className="mt-10 grid grid-cols-4 gap-px border-brutal border-border-default overflow-hidden">
          {STATS.map((s) => (
            <div key={s.label} className="bg-raised px-4 py-5 text-center">
              <p className="text-2xl font-black text-accent-agent tabular-nums">
                {s.value}
              </p>
              <p className="text-[10px] font-mono uppercase tracking-widest text-text-lo mt-1">
                {s.label}
              </p>
            </div>
          ))}
        </div>
      </header>

      {/* ── Team ─────────────────────────────────────────────────── */}
      <section className="mb-16">
        <p className="text-[10px] font-mono uppercase tracking-widest text-text-lo mb-6">
          {/* team */}
        </p>

        <div className="grid gap-px sm:grid-cols-2 border-brutal border-border-default overflow-hidden">
          {TEAM.map((member, i) => (
            <div
              key={member.handle}
              className="bg-raised p-8 flex flex-col gap-6 relative overflow-hidden"
            >
              {/* large index watermark */}
              <span className="absolute top-4 right-5 text-7xl font-black text-white/5 select-none tabular-nums leading-none">
                0{i + 1}
              </span>

              {/* avatar + name */}
              <div className="flex items-center gap-4">
                <div className="relative shrink-0">
                  <div className="absolute inset-0 rounded-sharp border-2 border-accent-agent translate-x-1 translate-y-1" />
                  <Image
                    src={member.avatar}
                    alt={member.name}
                    width={72}
                    height={72}
                    className="relative rounded-sharp object-cover border-2 border-border-default"
                  />
                </div>
                <div>
                  <p className="text-xl font-black text-text-hi leading-tight">
                    {member.name}
                  </p>
                  <p className="text-xs font-mono text-accent-agent mt-1">
                    {member.role}
                  </p>
                </div>
              </div>

              {/* social links */}
              <div className="flex flex-wrap items-center gap-2 mt-auto">
                <Link
                  href={member.linkedin}
                  target="_blank"
                  rel="noopener noreferrer"
                  className="inline-flex items-center gap-1.5 border border-border-default px-3 py-1.5 text-xs font-mono text-text-lo hover:text-text-hi hover:border-accent-agent transition-colors"
                >
                  <Linkedin className="w-3 h-3" />
                  LinkedIn
                </Link>
                <Link
                  href={member.github}
                  target="_blank"
                  rel="noopener noreferrer"
                  className="inline-flex items-center gap-1.5 border border-border-default px-3 py-1.5 text-xs font-mono text-text-lo hover:text-text-hi hover:border-accent-agent transition-colors"
                >
                  <Github className="w-3 h-3" />
                  GitHub
                </Link>
                {member.website && (
                  <Link
                    href={member.website}
                    target="_blank"
                    rel="noopener noreferrer"
                    className="inline-flex items-center gap-1.5 border border-border-default px-3 py-1.5 text-xs font-mono text-text-lo hover:text-text-hi hover:border-accent-agent transition-colors"
                  >
                    <Globe className="w-3 h-3" />
                    {member.website.replace("https://", "")}
                  </Link>
                )}
              </div>
            </div>
          ))}
        </div>
      </section>

      {/* ── Mission quote ─────────────────────────────────────────── */}
      <section className="mb-16 border-l-4 border-accent-agent pl-6 py-2">
        <p className="text-lg font-mono text-text-hi leading-relaxed">
          &ldquo;Stablecoin-native finance deserves an agent that earns trust
          one approved move at a time.&rdquo;
        </p>
      </section>

      {/* ── Stack ─────────────────────────────────────────────────── */}
      <section>
        <p className="text-[10px] font-mono uppercase tracking-widest text-text-lo mb-4">
          {/* built with */}
        </p>
        <div className="flex flex-wrap gap-2">
          {STACK.map((tech) => (
            <span
              key={tech}
              className="border border-border-default px-3 py-1 text-xs font-mono text-text-lo hover:text-accent-agent hover:border-accent-agent transition-colors"
            >
              {tech}
            </span>
          ))}
        </div>
      </section>
    </LandingShell>
  );
}
