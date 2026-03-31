import { NextResponse } from "next/server";
import type { NextRequest } from "next/server";

/**
 * PWA middleware — protects /staff/* routes.
 *
 * Staff routes require pwa_staff_jwt cookie (set after PIN validation).
 * Customer routes (/, /book, /wallet, etc.) are unaffected.
 *
 * MMA VERIFY P1 fix: Without this middleware, /staff/diagnosis page HTML
 * would be served to any visitor, with only client-side PIN check.
 * Server-side check prevents page content from being leaked.
 */

const STAFF_ROUTES = ["/staff"];

export function middleware(request: NextRequest) {
  const { pathname } = request.nextUrl;

  const isStaffRoute = STAFF_ROUTES.some(
    (route) => pathname === route || pathname.startsWith(`${route}/`)
  );

  if (!isStaffRoute) {
    return NextResponse.next();
  }

  // Check for staff JWT cookie (set by client after PIN validation)
  const staffJwt = request.cookies.get("pwa_staff_jwt");

  if (!staffJwt?.value) {
    // No staff cookie — redirect to PWA home
    const url = request.nextUrl.clone();
    url.pathname = "/";
    return NextResponse.redirect(url);
  }

  // Validate JWT structure (3 base64url segments)
  const parts = staffJwt.value.split(".");
  if (parts.length !== 3) {
    const url = request.nextUrl.clone();
    url.pathname = "/";
    return NextResponse.redirect(url);
  }

  // Check expiration
  try {
    const payload = JSON.parse(atob(parts[1]));
    if (payload.exp && payload.exp < Math.floor(Date.now() / 1000)) {
      const url = request.nextUrl.clone();
      url.pathname = "/";
      const response = NextResponse.redirect(url);
      response.cookies.delete("pwa_staff_jwt");
      return response;
    }
  } catch {
    const url = request.nextUrl.clone();
    url.pathname = "/";
    return NextResponse.redirect(url);
  }

  return NextResponse.next();
}

export const config = {
  matcher: ["/staff/:path*"],
};
