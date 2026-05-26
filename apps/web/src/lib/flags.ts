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

/**
 * Mirrors the backend `VOLATILE_EXECUTION_ENABLED` (default off). While off,
 * volatile sleeves are tracked-not-traded (testnet AMM pools are detached from
 * real marks); mainnet flips this on so volatiles become tradeable/rebalanceable.
 * The deployment must set both the API var and this `NEXT_PUBLIC_` var together.
 */
export const VOLATILE_EXECUTION_ENABLED = readFlag(
  "NEXT_PUBLIC_VOLATILE_EXECUTION_ENABLED",
);
