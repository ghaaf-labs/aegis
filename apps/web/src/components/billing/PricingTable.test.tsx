import { describe, expect, it } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import { DEFAULT_PRICING_TIERS, PricingTable } from "./PricingTable";

describe("<PricingTable />", () => {
  it("renders three tiers with correct names and monthly prices", () => {
    const html = renderToStaticMarkup(<PricingTable />);
    expect(html).toContain("FREE");
    expect(html).toContain("PRO");
    expect(html).toContain("BUSINESS");
    expect(html).toContain("$0");
    expect(html).toContain("$19");
    expect(html).toContain("$199");
  });

  it("surfaces per-rebalance bps and AUM bps from the tier matrix", () => {
    const html = renderToStaticMarkup(<PricingTable />);
    expect(html).toContain("25 bps"); // free per-rebalance / pro aum
    expect(html).toContain("15 bps"); // pro per-rebalance / business aum
    expect(html).toContain("10 bps"); // business per-rebalance
  });

  it("marks the user's current tier with a YOUR PLAN pill", () => {
    const html = renderToStaticMarkup(<PricingTable currentTier="pro" />);
    expect(html).toContain("YOUR PLAN");
  });

  it("DEFAULT_PRICING_TIERS mirrors §2.1 numbers exactly", () => {
    const find = (slug: "free" | "pro" | "business") => {
      const t = DEFAULT_PRICING_TIERS.find((x) => x.tier === slug);
      if (!t) throw new Error(`missing tier ${slug}`);
      return t;
    };
    expect(find("free").monthlyUsd).toBe(0);
    expect(find("free").aumCapUsd).toBe(5000);
    expect(find("free").decisionsPerMonth).toBe(5);
    expect(find("pro").monthlyUsd).toBe(19);
    expect(find("pro").decisionsPerMonth).toBe(240);
    expect(find("pro").aumAnnualBps).toBe(25);
    expect(find("business").monthlyUsd).toBe(199);
    expect(find("business").aumAnnualBps).toBe(15);
  });
});
