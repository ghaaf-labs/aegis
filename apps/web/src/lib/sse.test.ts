import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { defaultSseUrl } from "./sse";

// These tests cover the URL-resolution helper. The `useEventSource` hook
// itself is exercised in component-level tests once we add a mocked
// EventSource fixture; for Sprint 1 we only assert the env wiring.

describe("defaultSseUrl", () => {
  const originalEnv = { ...process.env };
  beforeEach(() => {
    vi.resetModules();
  });
  afterEach(() => {
    process.env = { ...originalEnv };
  });

  it("prefers NEXT_PUBLIC_SSE_URL when set", () => {
    process.env.NEXT_PUBLIC_SSE_URL = "https://api.example.com/sse";
    expect(defaultSseUrl()).toBe("https://api.example.com/sse");
  });

  it("derives from NEXT_PUBLIC_API_URL when SSE_URL is absent", () => {
    delete process.env.NEXT_PUBLIC_SSE_URL;
    process.env.NEXT_PUBLIC_API_URL = "https://api.example.com/";
    expect(defaultSseUrl()).toBe("https://api.example.com/sse");
  });

  it("falls back to localhost when nothing is set", () => {
    delete process.env.NEXT_PUBLIC_SSE_URL;
    delete process.env.NEXT_PUBLIC_API_URL;
    expect(defaultSseUrl()).toBe("http://localhost:8080/sse");
  });
});
