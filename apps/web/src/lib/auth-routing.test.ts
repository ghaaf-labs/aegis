import { describe, expect, it } from "vitest";
import {
  buildLoginRedirectUrl,
  isProtectedAppPath,
  safeNextPath,
} from "./auth-routing";

describe("auth route protection", () => {
  it("marks product routes as protected", () => {
    expect(isProtectedAppPath("/dashboard")).toBe(true);
    expect(isProtectedAppPath("/dashboard/abc")).toBe(true);
    expect(isProtectedAppPath("/rebalance/plan-1")).toBe(true);
    expect(isProtectedAppPath("/tax")).toBe(true);
    expect(isProtectedAppPath("/tax-center")).toBe(true);
    expect(isProtectedAppPath("/settings/tax")).toBe(true);
  });

  it("leaves public and auth routes outside the app gate", () => {
    expect(isProtectedAppPath("/")).toBe(false);
    expect(isProtectedAppPath("/help")).toBe(false);
    expect(isProtectedAppPath("/explore")).toBe(false);
    expect(isProtectedAppPath("/login")).toBe(false);
  });

  it("does not treat login as protected app path", () => {
    expect(isProtectedAppPath("/login")).toBe(false);
  });

  it("does not accept external or recursive next destinations", () => {
    expect(safeNextPath("https://evil.example")).toBeNull();
    expect(safeNextPath("//evil.example/path")).toBeNull();
    expect(safeNextPath("/login?next=/dashboard")).toBeNull();
    expect(safeNextPath("/dashboard?tab=agent")).toBe("/dashboard?tab=agent");
  });

  it("normalizes the legacy wallet destination", () => {
    expect(safeNextPath("/wallet")).toBe("/wallets");
    expect(safeNextPath("/wallet?tab=cash")).toBe("/wallets?tab=cash");
  });

  it("normalizes the legacy tax destination", () => {
    expect(safeNextPath("/tax")).toBe("/tax-center");
    expect(safeNextPath("/tax?year=2026")).toBe("/tax-center?year=2026");
  });

  it("builds a login redirect with the original protected destination", () => {
    const redirect = buildLoginRedirectUrl(
      new URL("http://localhost:3000/dashboard/abc?x=1"),
      "session_expired",
    );

    expect(redirect.pathname).toBe("/login");
    expect(redirect.searchParams.get("next")).toBe("/dashboard/abc?x=1");
    expect(redirect.searchParams.get("reason")).toBe("session_expired");
  });

  it("builds wallet redirects with the canonical wallets destination", () => {
    const redirect = buildLoginRedirectUrl(
      new URL("http://localhost:3000/wallet"),
      "session_required",
    );

    expect(redirect.pathname).toBe("/login");
    expect(redirect.searchParams.get("next")).toBe("/wallets");
    expect(redirect.searchParams.get("reason")).toBe("session_required");
  });
});
