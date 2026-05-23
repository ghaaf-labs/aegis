import { afterEach, describe, expect, it, vi } from "vitest";
import { accountApi, agentApi, walletApi } from "./api";

describe("walletApi.logout", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
    window.localStorage.clear();
  });

  it("clears remembered email after a confirmed server logout", async () => {
    window.localStorage.setItem("aegis_email", "user@example.com");
    const fetchMock = vi
      .fn()
      .mockResolvedValueOnce(new Response(null, { status: 204 }));
    vi.stubGlobal("fetch", fetchMock);

    await expect(walletApi.logout()).resolves.toBeUndefined();

    expect(fetchMock).toHaveBeenCalledWith(
      "http://localhost:8080/auth/logout",
      {
        method: "POST",
        headers: { "X-Aegis-Request": "1" },
        credentials: "include",
      },
    );
    expect(fetchMock).toHaveBeenCalledTimes(1);
    expect(window.localStorage.getItem("aegis_email")).toBeNull();
  });

  it("does not pretend logout succeeded when the API rejects it", async () => {
    window.localStorage.setItem("aegis_email", "user@example.com");
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
    expect(window.localStorage.getItem("aegis_email")).toBe("user@example.com");
  });

  it("does not make logout depend on a second session probe", async () => {
    window.localStorage.setItem("aegis_email", "user@example.com");
    const fetchMock = vi
      .fn()
      .mockResolvedValueOnce(new Response(null, { status: 204 }));
    vi.stubGlobal("fetch", fetchMock);

    await walletApi.logout();

    expect(fetchMock).toHaveBeenCalledTimes(1);
    expect(window.localStorage.getItem("aegis_email")).toBeNull();
  });
});

describe("walletApi.session", () => {
  afterEach(() => {
    vi.useRealTimers();
    vi.unstubAllGlobals();
  });

  it("does not leave auth screens waiting forever when the API stalls", async () => {
    vi.useFakeTimers();
    const fetchMock = vi.fn((_url: string, init?: RequestInit) => {
      return new Promise((_resolve, reject) => {
        init?.signal?.addEventListener("abort", () => {
          reject(new DOMException("Aborted", "AbortError"));
        });
      });
    });
    vi.stubGlobal("fetch", fetchMock);

    const result = walletApi.session();
    const assertion = expect(result).rejects.toThrow(
      "Request timed out. Try again.",
    );
    await vi.advanceTimersByTimeAsync(8_000);

    await assertion;
    expect(fetchMock).toHaveBeenCalledWith(
      "http://localhost:8080/auth/session",
      expect.objectContaining({
        method: "GET",
        signal: expect.any(AbortSignal),
      }),
    );
  });
});

describe("accountApi.exportData", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("uses POST with the CSRF header because export queues an email", async () => {
    const fetchMock = vi.fn().mockResolvedValueOnce(
      new Response(
        JSON.stringify({
          status: "queued",
          deliveryEmail: "user@example.com",
          expiresAt: "2026-05-23T12:00:00Z",
        }),
        {
          status: 202,
          headers: { "Content-Type": "application/json" },
        },
      ),
    );
    vi.stubGlobal("fetch", fetchMock);

    await expect(accountApi.exportData()).resolves.toMatchObject({
      status: "queued",
      deliveryEmail: "user@example.com",
    });

    expect(fetchMock).toHaveBeenCalledWith(
      "http://localhost:8080/account/export",
      {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
          "X-Aegis-Request": "1",
        },
        credentials: "include",
        body: undefined,
      },
    );
  });
});

describe("agentApi.analyze", () => {
  afterEach(() => {
    vi.useRealTimers();
    vi.unstubAllGlobals();
  });

  it("returns a useful timeout error when recommendation generation hangs", async () => {
    vi.useFakeTimers();
    const fetchMock = vi.fn((_url: string, init?: RequestInit) => {
      return new Promise((_resolve, reject) => {
        init?.signal?.addEventListener("abort", () => {
          reject(new DOMException("Aborted", "AbortError"));
        });
      });
    });
    vi.stubGlobal("fetch", fetchMock);

    const result = agentApi.analyze("portfolio-1", 25);
    const assertion = expect(result).rejects.toThrow(
      "Request timed out. Try again.",
    );
    await vi.advanceTimersByTimeAsync(25);

    await assertion;
    expect(fetchMock).toHaveBeenCalledWith(
      "http://localhost:8080/agent/analyze",
      expect.objectContaining({
        method: "POST",
        signal: expect.any(AbortSignal),
      }),
    );
  });
});
