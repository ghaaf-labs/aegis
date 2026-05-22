interface SessionCookieEnv {
  publicBaseUrl?: string;
  apiBaseUrl?: string;
  corsAllowOrigin?: string;
}

export function sessionCookieName(env: SessionCookieEnv = {}) {
  if (process.env.SESSION_COOKIE_NAME?.trim()) {
    return process.env.SESSION_COOKIE_NAME.trim();
  }
  return defaultSessionCookieName(env);
}

export function defaultSessionCookieName(env: SessionCookieEnv = {}) {
  const origins = [
    env.publicBaseUrl ?? "http://localhost:3000",
    env.apiBaseUrl ?? "http://localhost:8080",
    ...(env.corsAllowOrigin ?? "").split(","),
  ].filter((origin) => origin.trim().length > 0);

  return origins.every(isLocalHttpOrigin)
    ? "aegis_session"
    : "__Host-aegis_session";
}

function isLocalHttpOrigin(origin: string) {
  const trimmed = origin.trim();
  return (
    trimmed.startsWith("http://localhost") ||
    trimmed.startsWith("http://127.0.0.1") ||
    trimmed.startsWith("http://[::1]")
  );
}
