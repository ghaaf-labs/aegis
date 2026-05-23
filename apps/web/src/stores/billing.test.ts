import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Invoice, PricingTier, Subscription } from "@/types";

const PRO_TIER: PricingTier = {
  code: "pro",
  tier: "pro",
  name: "Pro",
  monthlyUsd: 19,
  aumCapUsd: null,
  portfolioCap: 5,
  portfoliosCap: 5,
  decisionsPerMonth: 240,
  decisionsCapMonthly: 240,
  models: "Haiku + Opus + GPT-5",
  perRebalanceBps: 15,
  aumAnnualBps: 25,
  features: [],
};

const PRO_SUB: Subscription = {
  id: "sub-1",
  userId: "user-1",
  tier: "pro",
  status: "active",
  currentPeriodStart: "2026-05-01T00:00:00Z",
  currentPeriodEnd: "2026-06-01T00:00:00Z",
  cancelAt: null,
  canceledAt: null,
  createdAt: "2026-05-01T00:00:00Z",
  updatedAt: "2026-05-01T00:00:00Z",
};

const INVOICE: Invoice = {
  id: "inv-1",
  userId: "user-1",
  subscriptionId: "sub-1",
  tier: "pro",
  periodStart: "2026-04-01T00:00:00Z",
  periodEnd: "2026-05-01T00:00:00Z",
  subtotalUsdc: 19,
  totalUsdc: 19,
  status: "paid",
  lineItems: [],
  paidTxHash: "0xabc",
  paidAt: "2026-05-01T00:00:00Z",
  createdAt: "2026-05-01T00:00:00Z",
};

vi.mock("@/lib/api", () => ({
  billingApi: {
    getSubscription: vi.fn(async () => null),
    listTiers: vi.fn(async () => [PRO_TIER]),
    listInvoices: vi.fn(async () => [] as Invoice[]),
    createSubscription: vi.fn(async () => PRO_SUB),
    patchSubscription: vi.fn(
      async (_id: string, body: { cancelAt?: string }) => ({
        ...PRO_SUB,
        cancelAt: body.cancelAt ?? null,
      }),
    ),
  },
}));

import { useBillingStore } from "./billing";

describe("useBillingStore", () => {
  beforeEach(() => {
    useBillingStore.getState().reset();
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it("fetch() populates subscription, tiers, and invoices", async () => {
    const { billingApi } = await import("@/lib/api");
    (billingApi.listInvoices as ReturnType<typeof vi.fn>).mockResolvedValueOnce(
      [INVOICE],
    );

    await useBillingStore.getState().fetch();
    const state = useBillingStore.getState();
    expect(state.tiers).toHaveLength(1);
    expect(state.tiers[0]?.tier).toBe("pro");
    expect(state.invoices).toHaveLength(1);
    expect(state.loading).toBe(false);
    expect(state.subscription).toBeNull();
  });

  it("fetch() keeps Free plan usable when billing tiers are unavailable", async () => {
    const { billingApi } = await import("@/lib/api");
    (billingApi.listTiers as ReturnType<typeof vi.fn>).mockRejectedValueOnce(
      new Error("404: That record was not found."),
    );

    await useBillingStore.getState().fetch();
    const state = useBillingStore.getState();
    expect(state.error).toBeNull();
    expect(state.tiers).toHaveLength(0);
    expect(state.subscription).toBeNull();
    expect(state.loading).toBe(false);
  });

  it("upgrade() calls the api and sets the new subscription", async () => {
    const result = await useBillingStore.getState().upgrade("pro");
    expect(result.tier).toBe("pro");
    expect(useBillingStore.getState().subscription?.tier).toBe("pro");

    const { billingApi } = await import("@/lib/api");
    expect(billingApi.createSubscription).toHaveBeenCalledWith({ tier: "pro" });
  });

  it("cancel() patches the subscription with cancelAt at period end", async () => {
    // seed
    await useBillingStore.getState().upgrade("pro");
    const next = await useBillingStore.getState().cancel();
    const expected = new Date(PRO_SUB.currentPeriodEnd).toISOString();
    expect(next?.cancelAt).toBe(expected);
    expect(useBillingStore.getState().subscription?.cancelAt).toBe(expected);
  });

  it("cancel() is a no-op when there is no subscription", async () => {
    expect(useBillingStore.getState().subscription).toBeNull();
    const result = await useBillingStore.getState().cancel();
    expect(result).toBeNull();
  });
});
