import type { Metadata } from "next";

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
      <main className="mx-auto max-w-3xl px-6 py-12 text-white">
        <h1 className="text-3xl font-bold mb-4">Aegis Constitution</h1>
        <p className="text-gray-400 font-mono text-sm">
          Constitution document is currently unavailable.
        </p>
      </main>
    );
  }

  return (
    <main className="mx-auto max-w-3xl px-6 py-12 text-white">
      <header className="mb-8">
        <h1 className="text-3xl font-bold mb-2">Aegis Constitution</h1>
        <p className="text-gray-300 text-sm leading-relaxed mb-4 max-w-2xl">
          Every Aegis decision is checked against this versioned rulebook
          before the LLM critic runs. When the strategist proposes a move
          that violates one of these clauses, the critic short-circuits to a
          veto and cites the clause ID — no model can override it. This is
          the auditable rulebook behind every block.
        </p>
        <div className="flex gap-4 font-mono text-[11px] text-gray-400">
          <span>
            version{" "}
            <span className="text-cyan-300">v{doc.version}</span>
          </span>
          <span>
            effective{" "}
            <span className="text-cyan-300">
              {new Date(doc.effectiveAt).toISOString().slice(0, 10)}
            </span>
          </span>
          <span>
            clauses{" "}
            <span className="text-cyan-300">{doc.clauses.length}</span>
          </span>
        </div>
      </header>

      <ol className="space-y-4">
        {doc.clauses.map((c) => {
          const tier = tierLabel(c.tierMin);
          return (
            <li
              key={c.id}
              className="border border-white/10 bg-[#141414] p-4 shadow-[4px_4px_0_0_#000]"
            >
              <div className="flex items-center gap-3 flex-wrap mb-2">
                <span className="font-mono text-sm bg-rose-500/15 border border-rose-500/30 px-2 py-0.5 text-rose-200">
                  {c.id}
                </span>
                <span className="font-mono text-[10px] uppercase tracking-wider text-gray-400">
                  {kindLabel(c.kind)}
                </span>
                {tier && (
                  <span className="font-mono text-[10px] uppercase tracking-wider text-cyan-300">
                    {tier}+
                  </span>
                )}
              </div>
              <h2 className="font-semibold text-white mb-1">{c.summary}</h2>
              <p className="text-gray-300 text-sm whitespace-pre-line">
                {c.description}
              </p>
              <p className="text-[11px] font-mono text-gray-500 mt-2">
                field: <span className="text-gray-300">{c.field}</span>
              </p>
            </li>
          );
        })}
      </ol>

      <footer className="mt-12 border-t border-white/10 pt-4 text-[11px] font-mono text-gray-500">
        Versioning rule: removing or weakening a clause is a major version
        bump. The current version is recorded on every persisted decision so
        future audits can re-evaluate historical proposals against the
        constitution that was in force at the time.
      </footer>
    </main>
  );
}
