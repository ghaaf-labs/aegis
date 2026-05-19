import type { Metadata } from "next";
import { BrutalPill } from "@aegis/ui";
import { LandingShell } from "@/components/layout/landing-shell";

import type { ConstitutionDocument } from "@/types";

export const metadata: Metadata = {
  title: "Aegis · Constitution",
  description:
    "Versioned hard constraints the Aegis agent must obey. Every critic veto cites the clause ID below.",
};

export const dynamic = "force-dynamic";
export const revalidate = 0;

async function fetchConstitution(): Promise<ConstitutionDocument | null> {
  const apiBase = process.env.NEXT_PUBLIC_API_URL ?? "http://localhost:8080";
  try {
    const res = await fetch(`${apiBase}/about/constitution`, {
      cache: "no-store",
    });
    if (!res.ok) return null;
    return (await res.json()) as ConstitutionDocument;
  } catch {
    return null;
  }
}

function kindLabel(kind: string): string {
  switch (kind) {
    case "hard_limit":
      return "Hard limit";
    case "band":
      return "Band";
    case "floor":
      return "Floor";
    case "ceiling":
      return "Ceiling";
    default:
      return kind;
  }
}

function tierLabel(tier?: string): string | null {
  if (!tier) return null;
  return tier.charAt(0).toUpperCase() + tier.slice(1);
}

export default async function ConstitutionPage() {
  const doc = await fetchConstitution();

  if (!doc) {
    return (
      <LandingShell>
        <header className="mb-10 pt-4">
          <BrutalPill tone="agent" className="mb-3">
            Agent rulebook
          </BrutalPill>
          <h1 className="mt-3 text-4xl font-bold text-text-hi tracking-tight">
            Aegis Constitution
          </h1>
        </header>
        <div className="border-brutal border-border-default bg-raised p-6">
          <p className="text-sm font-mono text-text-lo">
            Constitution document is unavailable — the API is offline or hasn
            &apos;t loaded its clauses yet. Try again in a moment.
          </p>
        </div>
      </LandingShell>
    );
  }

  return (
    <LandingShell>
      <header className="mb-10 pt-4">
        <BrutalPill tone="agent" className="mb-3">
          Agent rulebook
        </BrutalPill>
        <h1 className="mt-3 text-4xl font-bold text-text-hi tracking-tight">
          Aegis Constitution
        </h1>
        <p className="mt-4 text-sm text-text-lo font-mono leading-relaxed max-w-2xl">
          Every Aegis decision is checked against this versioned rulebook before
          the LLM critic runs. When the strategist proposes a move that violates
          one of these clauses, the critic short-circuits to a veto and cites
          the clause ID — no model can override it.
        </p>
        <div className="flex gap-4 font-mono text-[11px] text-text-mut mt-4">
          <span>
            version <span className="text-accent-agent">v{doc.version}</span>
          </span>
          <span>
            effective{" "}
            <span className="text-accent-agent">
              {new Date(doc.effectiveAt).toISOString().slice(0, 10)}
            </span>
          </span>
          <span>
            clauses{" "}
            <span className="text-accent-agent">{doc.clauses.length}</span>
          </span>
        </div>
      </header>

      <ol className="space-y-4">
        {doc.clauses.map((c) => {
          const tier = tierLabel(c.tierMin);
          return (
            <li
              key={c.id}
              className="border-brutal border-border-default bg-raised p-5"
            >
              <div className="flex items-center gap-3 flex-wrap mb-2">
                <span className="font-mono text-sm bg-risk/15 border border-risk/30 px-2 py-0.5 text-risk">
                  {c.id}
                </span>
                <span className="font-mono text-[10px] uppercase tracking-wider text-text-mut">
                  {kindLabel(c.kind)}
                </span>
                {tier && <BrutalPill tone="agent">{tier}+</BrutalPill>}
              </div>
              <h2 className="font-semibold text-text-hi mb-1">{c.summary}</h2>
              <p className="text-text-lo text-sm font-mono whitespace-pre-line">
                {c.description}
              </p>
              <p className="text-[11px] font-mono text-text-mut mt-2">
                field: <span className="text-text-lo">{c.field}</span>
              </p>
            </li>
          );
        })}
      </ol>

      <footer className="mt-10 border-t border-border-default pt-6 text-xs font-mono text-text-mut">
        Removing or weakening a clause is a major version bump. The version in
        force at decision time is recorded on every persisted decision for
        future audits.
      </footer>
    </LandingShell>
  );
}
