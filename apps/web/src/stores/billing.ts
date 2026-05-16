import { create } from "zustand";
import { devtools } from "zustand/middleware";
import type { Invoice, PricingTier, Subscription, Tier } from "@/types";
import { billingApi } from "@/lib/api";

interface BillingState {
  subscription: Subscription | null;
  tiers: PricingTier[];
  invoices: Invoice[];
  loading: boolean;
  error: string | null;

  fetch: () => Promise<void>;
  upgrade: (tier: Tier) => Promise<Subscription>;
  cancel: (cancelAt?: string | null) => Promise<Subscription | null>;
  reset: () => void;
}

/** Derive the user's current tier. Falls back to "free" when no subscription
 * row exists — the API does not insert a Free row at signup, so we must not
 * crash the UI on a 204 response. */
export function currentTier(s: Pick<BillingState, "subscription">): Tier {
  return s.subscription?.tier ?? "free";
}

export const useBillingStore = create<BillingState>()(
  devtools(
    (set, get) => ({
      subscription: null,
      tiers: [],
      invoices: [],
      loading: false,
      error: null,

      fetch: async () => {
        set({ loading: true, error: null });
        try {
          const [subscription, tiers, invoices] = await Promise.all([
            billingApi.getSubscription().catch(() => null),
            billingApi.listTiers(),
            billingApi.listInvoices({ limit: 24 }).catch(() => [] as Invoice[]),
          ]);
          set({ subscription, tiers, invoices, loading: false });
        } catch (e) {
          set({
            loading: false,
            error: e instanceof Error ? e.message : "failed to load billing",
          });
        }
      },

      upgrade: async (tier) => {
        const next = await billingApi.createSubscription({ tier });
        set({ subscription: next });
        // Re-pull invoices because upgrade may have generated a prorated row.
        billingApi
          .listInvoices({ limit: 24 })
          .then((invoices) => set({ invoices }))
          .catch(() => {
            /* ignore — non-fatal */
          });
        return next;
      },

      cancel: async (cancelAt = null) => {
        const sub = get().subscription;
        if (!sub) return null;
        const next = await billingApi.patchSubscription(sub.id, {
          cancelAt: cancelAt ?? new Date(sub.currentPeriodEnd).toISOString(),
        });
        set({ subscription: next });
        return next;
      },

      reset: () =>
        set({
          subscription: null,
          tiers: [],
          invoices: [],
          loading: false,
          error: null,
        }),
    }),
    { name: "aegis-billing" },
  ),
);
