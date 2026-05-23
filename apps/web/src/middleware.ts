import { NextResponse, type NextRequest } from "next/server";
import {
  buildLoginRedirectUrl,
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
    "/dashboard/:path*",
    "/wallet/:path*",
    "/wallets/:path*",
    "/portfolio/:path*",
    "/transactions/:path*",
    "/analytics/:path*",
    "/agent-logs/:path*",
    "/agent-studio/:path*",
    "/settings/:path*",
    "/tax/:path*",
    "/tax-center/:path*",
    "/rebalance/:path*",
    "/onboarding/:path*",
  ],
};
