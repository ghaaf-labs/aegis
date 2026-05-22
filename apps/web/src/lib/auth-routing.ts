const AUTH_PATHS = ["/login"];

const PROTECTED_APP_PREFIXES = [
  "/dashboard",
  "/wallet",
  "/wallets",
  "/portfolio",
  "/transactions",
  "/analytics",
  "/agent-logs",
  "/agent-studio",
  "/settings",
  "/tax-center",
  "/rebalance",
  "/onboarding",
];

export function isProtectedAppPath(pathname: string) {
  return PROTECTED_APP_PREFIXES.some(
    (prefix) => pathname === prefix || pathname.startsWith(`${prefix}/`),
  );
}

export function safeNextPath(path: string | null | undefined) {
  if (!path || !path.startsWith("/") || path.startsWith("//")) return null;
  const pathname = path.split(/[?#]/)[0] ?? path;
  if (
    AUTH_PATHS.some(
      (authPath) =>
        pathname === authPath || pathname.startsWith(`${authPath}/`),
    )
  ) {
    return null;
  }
  if (path.length > 2048) return null;
  return path;
}

export function buildLoginRedirectUrl(currentUrl: URL, reason: string) {
  const redirectUrl = new URL("/login", currentUrl.origin);
  const next = safeNextPath(`${currentUrl.pathname}${currentUrl.search}`);
  if (next) redirectUrl.searchParams.set("next", next);
  redirectUrl.searchParams.set("reason", reason);
  return redirectUrl;
}
