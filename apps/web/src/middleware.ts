import { NextResponse, type NextRequest } from "next/server";
import {
  buildLoginRedirectUrl,
  isLegacyAuthPath,
  isProtectedAppPath,
  safeNextPath,
} from "@/lib/auth-routing";
import { sessionCookieName } from "@/lib/session-cookie";

const API_URL = process.env.NEXT_PUBLIC_API_URL ?? "http://localhost:8080";
const SESSION_COOKIE_NAME = sessionCookieName({
  publicBaseUrl: process.env.NEXT_PUBLIC_APP_URL ?? "http://localhost:3000",
  apiBaseUrl: API_URL,
  corsAllowOrigin: process.env.NEXT_PUBLIC_APP_URL ?? "http://localhost:3000",
});

export async function middleware(request: NextRequest) {
  const pathname = request.nextUrl.pathname;
  if (isLegacyAuthPath(pathname)) {
    return NextResponse.redirect(legacyAuthRedirectUrl(request.nextUrl));
  }

  if (pathname === "/login") {
    const token = request.cookies.get(SESSION_COOKIE_NAME)?.value;
    if (!token) return NextResponse.next();

    const sessionCheck = await checkServerSession(token);
    if (sessionCheck === "active") {
      return NextResponse.redirect(postAuthRedirectUrl(request.nextUrl));
    }
    return NextResponse.next();
  }

  if (!isProtectedAppPath(pathname)) return NextResponse.next();

  const token = request.cookies.get(SESSION_COOKIE_NAME)?.value;
  if (!token) {
    return NextResponse.redirect(
      buildLoginRedirectUrl(request.nextUrl, "session_required"),
    );
  }

  const sessionCheck = await checkServerSession(token);
  if (sessionCheck !== "active") {
    return NextResponse.redirect(
      buildLoginRedirectUrl(
        request.nextUrl,
        sessionCheck === "unavailable"
          ? "session_check_failed"
          : "session_expired",
      ),
    );
  }

  return NextResponse.next();
}

function postAuthRedirectUrl(currentUrl: URL) {
  const next = safeNextPath(currentUrl.searchParams.get("next"));
  return new URL(next ?? "/dashboard", currentUrl.origin);
}

function legacyAuthRedirectUrl(currentUrl: URL) {
  const loginUrl = new URL("/login", currentUrl.origin);
  const ref = currentUrl.searchParams.get("ref")?.trim();
  const email = currentUrl.searchParams.get("email")?.trim();
  const signedOut = currentUrl.searchParams.get("signedOut");
  const reason = currentUrl.searchParams.get("reason");
  const next = safeNextPath(currentUrl.searchParams.get("next"));

  if (ref) loginUrl.searchParams.set("ref", ref);
  if (email) loginUrl.searchParams.set("email", email);
  if (signedOut === "1") loginUrl.searchParams.set("signedOut", signedOut);
  if (reason) loginUrl.searchParams.set("reason", reason);
  if (next) loginUrl.searchParams.set("next", next);

  return loginUrl;
}

async function checkServerSession(token: string) {
  try {
    const response = await fetch(`${API_URL}/auth/session`, {
      cache: "no-store",
      headers: {
        Cookie: `${SESSION_COOKIE_NAME}=${token}`,
      },
    });
    return response.ok ? "active" : "rejected";
  } catch {
    return "unavailable";
  }
}

export const config = {
  matcher: [
    "/login",
    "/signup/:path*",
    "/sign-up/:path*",
    "/signin/:path*",
    "/sign-in/:path*",
    "/register/:path*",
    "/dashboard/:path*",
    "/wallet/:path*",
    "/wallets/:path*",
    "/portfolio/:path*",
    "/transactions/:path*",
    "/analytics/:path*",
    "/agent-logs/:path*",
    "/agent-studio/:path*",
    "/settings/:path*",
    "/tax-center/:path*",
    "/rebalance/:path*",
    "/onboarding/:path*",
  ],
};
