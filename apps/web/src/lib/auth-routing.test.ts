import { describe, expect, it } from "vitest";
import {
  buildLoginRedirectUrl,
  isLegacyAuthPath,
  isProtectedAppPath,
  safeNextPath,
} from "./auth-routing";

describe("auth route protection", () => {
  it("marks product routes as protected", () => {
    expect(isProtectedAppPath("/dashboard")).toBe(true);
    expect(isProtectedAppPath("/dashboard/abc")).toBe(true);
    expect(isProtectedAppPath("/rebalance/plan-1")).toBe(true);
    expect(isProtectedAppPath("/settings/tax")).toBe(true);
  });

  it("leaves public and auth routes outside the app gate", () => {
    expect(isProtectedAppPath("/")).toBe(false);
    expect(isProtectedAppPath("/help")).toBe(false);
    expect(isProtectedAppPath("/strategies")).toBe(false);
    expect(isProtectedAppPath("/login")).toBe(false);
    expect(isProtectedAppPath("/signup")).toBe(false);
    expect(isProtectedAppPath("/register")).toBe(false);
  });

  it("treats non-canonical auth paths as legacy aliases", () => {
    expect(isLegacyAuthPath("/signup")).toBe(true);
    expect(isLegacyAuthPath("/signup/referral")).toBe(true);
    expect(isLegacyAuthPath("/sign-up")).toBe(true);
    expect(isLegacyAuthPath("/signin")).toBe(true);
    expect(isLegacyAuthPath("/sign-in")).toBe(true);
    expect(isLegacyAuthPath("/register")).toBe(true);
    expect(isLegacyAuthPath("/login")).toBe(false);
  });

  it("does not accept external or recursive next destinations", () => {
    expect(safeNextPath("https://evil.example")).toBeNull();
    expect(safeNextPath("//evil.example/path")).toBeNull();
    expect(safeNextPath("/login?next=/dashboard")).toBeNull();
    expect(safeNextPath("/signup")).toBeNull();
    expect(safeNextPath("/register?next=/dashboard")).toBeNull();
    expect(safeNextPath("/sign-in")).toBeNull();
    expect(safeNextPath("/dashboard?tab=agent")).toBe("/dashboard?tab=agent");
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
});
