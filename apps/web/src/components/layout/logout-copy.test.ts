import { describe, expect, it } from "vitest";
import { logoutFailureMessage, logoutRedirect } from "./logout-copy";

describe("logout copy", () => {
  it("keeps logout failures plain and user-safe", () => {
    expect(
      logoutFailureMessage(new Error("verification failed after logout")),
    ).toBe("We could not finish signing you out. Try again.");
    expect(
      logoutFailureMessage(new Error("server still accepts session")),
    ).toBe("Sign out is still finishing. Try again.");
    expect(logoutFailureMessage(new Error("network down"))).toBe(
      "We could not sign you out. Check your connection and try again.",
    );
  });

  it("keeps the signed-out redirect stable", () => {
    expect(logoutRedirect()).toBe("/login?signedOut=1");
  });
});
