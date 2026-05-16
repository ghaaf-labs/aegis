import Link from "next/link";

export interface DecisionLeg {
  legIndex: number;
  kind: string;
  srcChain: string | null;
  destChain: string | null;
  srcSymbol: string | null;
  destSymbol: string | null;
  amountUsdc: number;
  status: string;
  txHash: string | null;
}

export interface DecisionFull {
  decisionId: string;
  portfolioId: string;
  regime: string | null;
  modelSlug: string | null;
  confidence: number;
  rawConfidence: number | null;
  calibratedConfidence: number | null;
  promptTokens: number | null;
  completionTokens: number | null;
  latencyMs: number | null;
  promptExcerpt: string;
  recommendation: unknown;
  criticVerdict: {
    verdict?: string;
    notes?: string;
    clauses?: string[];
  } | null;
  counterfactual: string | null;
  snapshot: unknown;
  createdAt: string;
  legs: DecisionLeg[];
}

const EXPLORERS: Record<string, string> = {
  arc: "https://testnet.arcscan.app/tx/",
  base: "https://sepolia.basescan.org/tx/",
};

export function AuditTrail({ data }: { data: DecisionFull }) {
  const rec = data.recommendation as Record<string, unknown> | null;
  const trades = (rec?.trades as unknown[] | undefined) ?? [];
  const cost = totalCost(
    data.promptTokens,
    data.completionTokens,
    data.modelSlug,
  );

  return (
    <section className="border-2 border-white/10 bg-[#141414] divide-y divide-white/10">
      <Section title="Inputs">
        <KV
          k="Regime"
          v={data.regime?.replace("_", " ").toUpperCase() ?? "—"}
        />
        <KV k="Triggered at" v={new Date(data.createdAt).toISOString()} />
        <KV
          k="Portfolio"
          v={`${data.portfolioId.slice(0, 8)}… (see /diary/wallet for context)`}
        />
      </Section>

      <Section title="Strategist">
        <KV k="Model" v={data.modelSlug ?? "—"} />
        <KV
          k="Tokens"
          v={
            data.promptTokens != null && data.completionTokens != null
              ? `${data.promptTokens} prompt · ${data.completionTokens} completion`
              : "—"
          }
        />
        <KV
          k="Latency"
          v={data.latencyMs != null ? `${data.latencyMs}ms` : "—"}
        />
        {cost != null && <KV k="Est. spend" v={`$${cost.toFixed(4)}`} />}
        <KV
          k="Confidence"
          v={`${Math.round(data.confidence * 100)}%${
            data.calibratedConfidence != null
              ? ` · calibrated ${Math.round(data.calibratedConfidence * 100)}%`
              : ""
          }`}
        />
        <details className="md:col-span-2">
          <summary className="text-xs font-mono uppercase tracking-widest text-text-lo cursor-pointer">
            Reasoning excerpt
          </summary>
          <pre className="mt-2 text-xs text-text-default whitespace-pre-wrap font-mono leading-relaxed">
            {data.promptExcerpt || "(no reasoning persisted)"}
          </pre>
        </details>
      </Section>

      <Section title="Critic">
        {data.criticVerdict ? (
          <>
            <KV
              k="Verdict"
              v={(data.criticVerdict.verdict ?? "unknown").toUpperCase()}
            />
            <KV k="Notes" v={data.criticVerdict.notes ?? "—"} />
            {Array.isArray(data.criticVerdict.clauses) &&
              data.criticVerdict.clauses.length > 0 && (
                <div className="md:col-span-2 flex flex-wrap gap-2 mt-2">
                  {data.criticVerdict.clauses.map((id) => (
                    <Link
                      key={id}
                      href={`/about/constitution#${id}`}
                      className="px-2 py-0.5 text-[10px] font-mono border border-cyan-500/40 text-cyan-300 bg-cyan-500/10 hover:bg-cyan-500/20"
                    >
                      {id}
                    </Link>
                  ))}
                </div>
              )}
          </>
        ) : (
          <p className="text-xs text-text-mut md:col-span-2">
            No critic pass recorded (Free-tier or skipped).
          </p>
        )}
        {data.counterfactual && (
          <div className="md:col-span-2 mt-2 text-xs text-text-lo">
            <span className="font-mono uppercase tracking-widest text-text-mut">
              Counterfactual:
            </span>{" "}
            {data.counterfactual}
          </div>
        )}
      </Section>

      <Section title="Plan">
        {trades.length === 0 ? (
          <p className="text-xs text-text-mut md:col-span-2">
            No proposed trades (the agent abstained).
          </p>
        ) : (
          <div className="md:col-span-2 overflow-x-auto">
            <table className="w-full text-xs font-mono">
              <thead>
                <tr className="text-text-mut text-left">
                  <th className="py-1 pr-3">Asset</th>
                  <th className="py-1 pr-3">Action</th>
                  <th className="py-1 pr-3 text-right">Qty</th>
                  <th className="py-1 pr-3 text-right">USD</th>
                </tr>
              </thead>
              <tbody>
                {trades.map((t, i) => {
                  const trade = t as Record<string, unknown>;
                  return (
                    <tr key={i} className="border-t border-white/5">
                      <td className="py-1 pr-3 text-text-hi">
                        {String(trade.symbol ?? "—")}
                      </td>
                      <td className="py-1 pr-3 text-text-default">
                        {String(trade.action ?? "—")}
                      </td>
                      <td className="py-1 pr-3 text-right text-text-default">
                        {numStr(trade.quantity)}
                      </td>
                      <td className="py-1 pr-3 text-right text-text-default">
                        {numStr(trade.valueUsd)}
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        )}
      </Section>

      <Section title="Execution">
        {data.legs.length === 0 ? (
          <p className="text-xs text-text-mut md:col-span-2">
            No executed legs — decision is either pending approval or the user
            declined.
          </p>
        ) : (
          <ul className="md:col-span-2 space-y-2">
            {data.legs.map((leg) => (
              <li
                key={leg.legIndex}
                className="flex flex-wrap items-baseline justify-between gap-2 border border-white/5 bg-[#0F0F0F] px-3 py-2 text-xs font-mono"
              >
                <div className="flex flex-wrap items-baseline gap-2">
                  <span className="text-text-mut">#{leg.legIndex}</span>
                  <span className="text-text-hi">{leg.kind}</span>
                  {leg.srcChain && (
                    <span className="text-text-lo">
                      {leg.srcChain.toUpperCase()}
                      {leg.destChain &&
                        leg.destChain !== leg.srcChain &&
                        ` → ${leg.destChain.toUpperCase()}`}
                    </span>
                  )}
                  <span className="text-text-default">
                    ${leg.amountUsdc.toFixed(2)}
                  </span>
                </div>
                <div className="flex items-baseline gap-2">
                  <span
                    className={
                      leg.status === "confirmed"
                        ? "text-cyan-300"
                        : leg.status === "failed"
                          ? "text-rose-300"
                          : "text-text-lo"
                    }
                  >
                    {leg.status}
                  </span>
                  {leg.txHash && (
                    <a
                      href={`${EXPLORERS[leg.destChain ?? leg.srcChain ?? "base"] ?? EXPLORERS.base}${leg.txHash}`}
                      target="_blank"
                      rel="noreferrer"
                      className="text-cyan-300 underline-offset-4 hover:underline"
                    >
                      {leg.txHash.slice(0, 8)}…
                    </a>
                  )}
                </div>
              </li>
            ))}
          </ul>
        )}
      </Section>
    </section>
  );
}

function Section({
  title,
  children,
}: {
  title: string;
  children: React.ReactNode;
}) {
  return (
    <div className="p-4">
      <h3 className="text-[10px] font-mono uppercase tracking-widest text-text-mut mb-3">
        {title}
      </h3>
      <div className="grid grid-cols-1 md:grid-cols-2 gap-x-6 gap-y-1 text-sm">
        {children}
      </div>
    </div>
  );
}

function KV({ k, v }: { k: string; v: string }) {
  return (
    <div className="flex items-baseline justify-between gap-3">
      <span className="text-xs text-text-mut font-mono uppercase tracking-widest">
        {k}
      </span>
      <span className="text-text-default text-right break-all">{v}</span>
    </div>
  );
}

function numStr(v: unknown): string {
  return typeof v === "number" ? v.toFixed(2) : "—";
}

// Rough cost estimate against the per-1M-token list price for a few model
// families. Used only as a "what did this decision approximately cost the
// system?" diagnostic; not a billable figure.
function totalCost(
  pt: number | null,
  ct: number | null,
  slug: string | null,
): number | null {
  if (pt == null || ct == null) return null;
  const family = (slug ?? "").toLowerCase();
  const prices: Record<string, { in: number; out: number }> = {
    haiku: { in: 0.8, out: 4 },
    deepseek: { in: 0.27, out: 1.1 },
    "gpt-5": { in: 10, out: 30 },
    gemini: { in: 0.075, out: 0.3 },
    opus: { in: 15, out: 75 },
    qwen3: { in: 0.05, out: 0.2 },
  };
  const key = Object.keys(prices).find((k) => family.includes(k));
  if (!key) return null;
  const p = prices[key]!;
  return (pt / 1_000_000) * p.in + (ct / 1_000_000) * p.out;
}
