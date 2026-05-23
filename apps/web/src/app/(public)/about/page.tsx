import type { Metadata } from "next";
import Link from "next/link";
import { Github, Linkedin, Users } from "lucide-react";
import { LandingShell } from "@/components/layout/landing-shell";
import { BrutalPill } from "@aegis/ui";

export const metadata: Metadata = {
  title: "About — Aegis",
  description:
    "Meet the team behind Aegis — an adaptive stablecoin portfolio agent built on Circle's stack.",
};

const TEAM = [
  {
    name: "Mohammad Jalili",
    handle: "mohijalili",
    role: "Co-founder & Engineer",
    linkedin: "https://www.linkedin.com/in/mohammadjalili/",
    github: "https://github.com/mohijalili",
  },
  {
    name: "Mahdi",
    handle: "malivix",
    role: "Co-founder & Engineer",
    linkedin: "https://www.linkedin.com/in/malivix/",
    github: "https://github.com/malivix",
  },
];

export default function AboutPage() {
  return (
    <LandingShell>
      <header className="mb-12 pt-4">
        <BrutalPill tone="agent" className="mb-3">
          <Users className="w-3 h-3 mr-1 inline-block" />
          Team
        </BrutalPill>
        <h1 className="mt-3 text-4xl font-bold text-text-hi tracking-tight">
          About Aegis
        </h1>
        <p className="mt-4 text-sm font-mono leading-relaxed text-text-lo max-w-2xl">
          Aegis is an adaptive crypto portfolio harness for stablecoin-native
          finance — built on Circle&apos;s stack for the Agora Agents Hackathon.
          The user steers; a multi-model AI agent executes on Arc + Base.
        </p>
      </header>

      <section className="space-y-4">
        <h2 className="text-xs font-bold uppercase tracking-widest text-text-lo mb-6">
          Builders
        </h2>
        <div className="grid gap-4 sm:grid-cols-2">
          {TEAM.map((member) => (
            <div
              key={member.handle}
              className="border-brutal border-border-default bg-raised p-6 flex flex-col gap-4"
            >
              <div>
                <p className="text-lg font-bold text-text-hi">{member.name}</p>
                <p className="text-xs font-mono text-accent-agent mt-0.5">
                  {member.role}
                </p>
              </div>

              <div className="flex items-center gap-3 mt-auto">
                <Link
                  href={member.linkedin}
                  target="_blank"
                  rel="noopener noreferrer"
                  className="inline-flex items-center gap-1.5 text-xs font-mono text-text-lo hover:text-text-hi transition-colors"
                >
                  <Linkedin className="w-3.5 h-3.5" />
                  LinkedIn
                </Link>
                <span className="text-border-default">·</span>
                <Link
                  href={member.github}
                  target="_blank"
                  rel="noopener noreferrer"
                  className="inline-flex items-center gap-1.5 text-xs font-mono text-text-lo hover:text-text-hi transition-colors"
                >
                  <Github className="w-3.5 h-3.5" />
                  GitHub
                </Link>
              </div>
            </div>
          ))}
        </div>
      </section>

      <section className="mt-12 border-brutal border-border-default bg-raised p-6">
        <h2 className="text-xs font-bold uppercase tracking-widest text-text-lo mb-4">
          Built with
        </h2>
        <div className="flex flex-wrap gap-2 font-mono text-xs text-text-lo">
          {[
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
          ].map((tech) => (
            <span
              key={tech}
              className="border border-border-default px-2 py-0.5"
            >
              {tech}
            </span>
          ))}
        </div>
      </section>
    </LandingShell>
  );
}
