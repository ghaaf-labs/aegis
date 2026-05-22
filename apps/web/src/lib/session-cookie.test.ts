import { afterEach, describe, expect, it } from "vitest";
import { defaultSessionCookieName, sessionCookieName } from "./session-cookie";

describe("session cookie naming", () => {
  const previous = process.env.SESSION_COOKIE_NAME;

  afterEach(() => {
    if (previous === undefined) {
      delete process.env.SESSION_COOKIE_NAME;
    } else {
      process.env.SESSION_COOKIE_NAME = previous;
    }
  });

  it("uses the local cookie name for localhost origins", () => {
    expect(
      defaultSessionCookieName({
        publicBaseUrl: "http://localhost:3000",
        apiBaseUrl: "http://localhost:8080",
        corsAllowOrigin: "http://localhost:3000",
      }),
    ).toBe("aegis_session");
  });

  it("uses the __Host cookie name for secure deployments", () => {
    expect(
      defaultSessionCookieName({
        publicBaseUrl: "https://aegis.example",
        apiBaseUrl: "https://aegis.example/api",
        corsAllowOrigin: "https://aegis.example",
      }),
    ).toBe("__Host-aegis_session");
  });

  it("honors an explicit deployment override", () => {
    process.env.SESSION_COOKIE_NAME = "custom_session";
    expect(sessionCookieName()).toBe("custom_session");
  });
});
