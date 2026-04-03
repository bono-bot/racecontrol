import { NextResponse } from "next/server";
import type { NextRequest } from "next/server";

/**
 * Kiosk server-side auth middleware (SEC-P2-9 fix)
 *
 * Protects staff-only routes at the server level BEFORE any page renders.
 * Eliminates the "flash of unauthorized content" where staff UI was briefly
 * visible to customers before client-side useEffect redirect fired.
 *
 * Protected routes: /staff, /control, /settings, /shutdown, /debug, /fleet
 * Public routes: / (lock screen), /book, /spectator, /pod/[number]
 *
 * Auth check: looks for kiosk_staff_jwt cookie (set on staff login).
 * If missing, redirects to /kiosk (lock screen).
 */

// Routes that require staff authentication
const STAFF_ROUTES = ["/staff", "/control", "/settings", "/shutdown", "/debug", "/fleet"];

export function middleware(request: NextRequest) {
  const { pathname } = request.nextUrl;

  // Strip basePath (/kiosk) for route matching
  const path = pathname.replace(/^\/kiosk/, "") || "/";

  // Check if this is a staff-only route
  const isStaffRoute = STAFF_ROUTES.some(
    (route) => path === route || path.startsWith(`${route}/`)
  );

  if (!isStaffRoute) {
    // WS-HARDEN: Prevent browser caching HTML pages — ensures fresh JS chunk
    // references are loaded after deploys. Static assets (_next/static/) have
    // content-hash filenames and are cached separately by the browser.
    const response = NextResponse.next();
    response.headers.set("Cache-Control", "no-cache, no-store, must-revalidate");
    response.headers.set("Pragma", "no-cache");
    return response;
  }

  // MMA iter1: Validate JWT structure, not just cookie existence.
  // Full cryptographic validation happens server-side on API calls.
  // Middleware checks: cookie exists + is valid JWT format (3 base64 segments)
  // + not expired (exp claim decoded without signature verification).
  const staffJwt = request.cookies.get("kiosk_staff_jwt");

  if (!staffJwt?.value) {
    const url = request.nextUrl.clone();
    url.pathname = "/kiosk";
    return NextResponse.redirect(url);
  }

  // Validate JWT structure: must be 3 dot-separated base64url segments
  const parts = staffJwt.value.split(".");
  if (parts.length !== 3) {
    // Malformed — not a real JWT (forged cookie)
    const url = request.nextUrl.clone();
    url.pathname = "/kiosk";
    return NextResponse.redirect(url);
  }

  // Check exp claim (decode payload without signature verification)
  try {
    const payload = JSON.parse(atob(parts[1]));
    if (payload.exp && payload.exp < Math.floor(Date.now() / 1000)) {
      // Expired JWT — force re-login
      const url = request.nextUrl.clone();
      url.pathname = "/kiosk";
      const response = NextResponse.redirect(url);
      response.cookies.delete("kiosk_staff_jwt");
      return response;
    }
  } catch {
    // Can't decode payload — reject
    const url = request.nextUrl.clone();
    url.pathname = "/kiosk";
    return NextResponse.redirect(url);
  }

  const response = NextResponse.next();
  response.headers.set("Cache-Control", "no-cache, no-store, must-revalidate");
  response.headers.set("Pragma", "no-cache");
  return response;
}

export const config = {
  // Match all kiosk routes except static files (_next/static) and API routes
  // Broader matcher ensures cache-busting headers apply to ALL HTML pages
  matcher: [
    "/((?!_next/static|_next/image|favicon.ico|api/).*)",
  ],
};
