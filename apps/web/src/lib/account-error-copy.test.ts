import { describe, expect, it } from "vitest";
import {
  formatGatewayBalanceError,
  friendlyAccountError,
  walletStatusError,
} from "./account-error-copy";

describe("account error copy", () => {
  it("keeps wallet setup errors user-safe", () => {
    expect(walletStatusError(new Error("401: missing token"))).toBe(
      "Your sign-in expired. Enter your email again before checking account setup.",
    );
    expect(walletStatusError(new Error("Failed to fetch"))).toBe(
      "We could not check your account. Check your connection and try again.",
    );
  });

  it("keeps balance errors clear without leaking providers", () => {
    expect(formatGatewayBalanceError(new Error("returned no wallets"))).toBe(
      "We could not find a wallet for this account, so wallet cash is unknown.",
    );
    expect(formatGatewayBalanceError(new Error("Circle Gateway timeout"))).toBe(
      "Wallet balance check failed.",
    );
    expect(formatGatewayBalanceError(new Error("NetworkError"))).toBe(
      "We could not check balances. Check your connection and try again.",
    );
  });

  it("keeps account settings errors actionable", () => {
    expect(
      friendlyAccountError(new Error("export email is not configured")),
    ).toBe("We could not prepare your export email. Try again later.");
    expect(friendlyAccountError(new Error("balance cannot be verified"))).toBe(
      "We could not verify balances. Try again later.",
    );
    expect(friendlyAccountError(new Error("funds_present"))).toBe(
      "Move your funds out before closing your account.",
    );
  });
});
