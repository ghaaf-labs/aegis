/**
 * Builds a twitter / X share intent URL pointing at a decision's public
 * page. Twitter scrapes the URL's OG metadata, which (via the route's
 * `generateMetadata`) points at `/og/[decisionId]` — so the share card is
 * the OG image, the click-through is the public decision page.
 */
export function buildShareIntent(opts: {
  decisionId: string;
  summary: string;
  realizedPct: number | null | undefined;
}): { intentUrl: string; shareUrl: string } {
  const PUBLIC_BASE =
    process.env.NEXT_PUBLIC_APP_URL ?? "http://localhost:3000";
  const shareUrl = `${PUBLIC_BASE}/decision/${opts.decisionId}`;
  const realized = opts.realizedPct;
  const realizedLabel =
    realized != null
      ? `Aegis ${realized >= 0 ? "+" : ""}${realized.toFixed(2)}% — `
      : "Aegis · ";
  const text = `${realizedLabel}${opts.summary} (via @aegisapp)`;
  const intentUrl = `https://x.com/intent/tweet?text=${encodeURIComponent(text)}&url=${encodeURIComponent(shareUrl)}`;
  return { intentUrl, shareUrl };
}
