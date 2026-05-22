import { afterEach, describe, expect, it, vi } from "vitest";
import { walletApi } from "./api";

describe("walletApi.logout", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
    window.localStorage.clear();
  });

  it("clears legacy client token after a confirmed server logout", async () => {
    window.localStorage.setItem("aegis.jwt", "legacy-token");
    const fetchMock = vi
      .fn()
      .mockResolvedValueOnce(new Response(null, { status: 204 }))
      .mockResolvedValueOnce(
        new Response(JSON.stringify({ code: "UNAUTHORIZED" }), {
          status: 401,
          headers: { "Content-Type": "application/json" },
        }),
      );
    vi.stubGlobal("fetch", fetchMock);

    await expect(walletApi.logout()).resolves.toBeUndefined();

    expect(fetchMock).toHaveBeenCalledWith(
      "http://localhost:8080/auth/logout",
      { method: "POST", credentials: "include" },
    );
    expect(fetchMock).toHaveBeenCalledWith("http://localhost:8080/auth/me", {
      cache: "no-store",
      credentials: "include",
    });
    expect(window.localStorage.getItem("aegis.jwt")).toBeNull();
  });

  it("does not pretend logout succeeded when the API rejects it", async () => {
    window.localStorage.setItem("aegis.jwt", "legacy-token");
    vi.stubGlobal(
      "fetch",
      vi.fn(
        async () =>
          new Response(JSON.stringify({ message: "session store down" }), {
            status: 503,
            headers: { "Content-Type": "application/json" },
          }),
      ),
    );

    await expect(walletApi.logout()).rejects.toThrow("503: session store down");
    expect(window.localStorage.getItem("aegis.jwt")).toBe("legacy-token");
  });

  it("does not clear local auth hints when the server still accepts the session", async () => {
    window.localStorage.setItem("aegis.jwt", "legacy-token");
    window.localStorage.setItem("aegis_email", "user@example.com");
    vi.stubGlobal(
      "fetch",
      vi
        .fn()
        .mockResolvedValueOnce(new Response(null, { status: 204 }))
        .mockResolvedValueOnce(
          new Response(JSON.stringify({ email: "user@example.com" }), {
            status: 200,
            headers: { "Content-Type": "application/json" },
          }),
        ),
    );

    await expect(walletApi.logout()).rejects.toThrow(
      "logout failed: server still accepts this browser session",
    );
    expect(window.localStorage.getItem("aegis.jwt")).toBe("legacy-token");
    expect(window.localStorage.getItem("aegis_email")).toBe("user@example.com");
  });
});
