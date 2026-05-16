/**
 * Frontend feature flags. Each flag has a `false` default so trunk-shippable
 * builds never expose half-finished UI. Server components and route shells
 * read these via the `NEXT_PUBLIC_*` env at build time (Next.js inlines them);
 * we re-export typed helpers so call-sites don't sprinkle string-typing.
 */

function readFlag(name: string): boolean {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const v = (process.env as any)[name];
  return v === "true" || v === "1";
}

export const PRICING_UI_ENABLED = readFlag("NEXT_PUBLIC_PRICING_UI_ENABLED");

export const flags = {
  pricingUi: PRICING_UI_ENABLED,
} as const;
