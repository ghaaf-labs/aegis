import { NextResponse, type NextRequest } from "next/server";
import { buildLoginRedirectUrl, isProtectedAppPath } from "@/lib/auth-routing";

const API_URL = process.env.NEXT_PUBLIC_API_URL ?? "http://localhost:8080";
const SESSION_COOKIE_NAME = process.env.SESSION_COOKIE_NAME ?? "aegis_jwt";

export async function middleware(request: NextRequest) {
  const pathname = request.nextUrl.pathname;
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

async function checkServerSession(token: string) {
  try {
    const response = await fetch(`${API_URL}/auth/me`, {
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
