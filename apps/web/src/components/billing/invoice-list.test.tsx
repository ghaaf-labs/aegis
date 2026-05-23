import { describe, expect, it } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import type { Invoice } from "@/types";
import { InvoiceList } from "./invoice-list";

const INVOICES: Invoice[] = [
  {
    id: "inv-001",
    userId: "u-1",
    subscriptionId: "sub-1",
    tier: "pro",
    periodStart: "2026-04-01T00:00:00Z",
    periodEnd: "2026-05-01T00:00:00Z",
    subtotalUsdc: 19,
    totalUsdc: 23.45,
    status: "paid",
    lineItems: [
      {
        kind: "subscription",
        description: "Pro monthly",
        amountUsdc: 19,
      },
      {
        kind: "aum_fee",
        description: "AUM stream",
        amountUsdc: 4.45,
      },
    ],
    paidTxHash: "0xdeadbeef1234567890abcdef",
    paidAt: "2026-05-01T00:00:00Z",
    createdAt: "2026-05-01T00:00:00Z",
  },
  {
    id: "inv-002",
    userId: "u-1",
    subscriptionId: "sub-1",
    tier: "pro",
    periodStart: "2026-05-01T00:00:00Z",
    periodEnd: "2026-06-01T00:00:00Z",
    subtotalUsdc: 19,
    totalUsdc: 19,
    status: "past_due",
    lineItems: [],
    paidTxHash: null,
    paidAt: null,
    createdAt: "2026-06-01T00:00:00Z",
  },
];

describe("<InvoiceList />", () => {
  it("matches snapshot when empty", () => {
    const html = renderToStaticMarkup(<InvoiceList invoices={[]} />);
    expect(html).toMatchSnapshot();
    expect(html).toContain("No invoices yet");
    expect(html).toContain("Free plan");
  });

  it("matches snapshot when populated", () => {
    const html = renderToStaticMarkup(<InvoiceList invoices={INVOICES} />);
    expect(html).toMatchSnapshot();
  });

  it("renders status pills with appropriate tones", () => {
    const html = renderToStaticMarkup(<InvoiceList invoices={INVOICES} />);
    expect(html).toContain("PAID");
    expect(html).toContain("PAST DUE");
  });

  it("links paid invoices to the Arc explorer", () => {
    const html = renderToStaticMarkup(<InvoiceList invoices={INVOICES} />);
    expect(html).toContain(
      "https://testnet.arcscan.app/tx/0xdeadbeef1234567890abcdef",
    );
  });
});
