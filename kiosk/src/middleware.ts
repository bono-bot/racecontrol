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
    return NextResponse.next();
  }

  // Check for staff auth cookie (set by StaffLoginScreen on successful PIN validation)
  const staffJwt = request.cookies.get("kiosk_staff_jwt");

  if (!staffJwt?.value) {
    // No auth — redirect to lock screen (not staff page)
    const url = request.nextUrl.clone();
    url.pathname = "/kiosk";
    return NextResponse.redirect(url);
  }

  // Staff JWT present — allow through
  // (JWT validation happens server-side on API calls, not in middleware)
  return NextResponse.next();
}

export const config = {
  // Match all kiosk routes except static files and API routes
  matcher: [
    "/kiosk/staff/:path*",
    "/kiosk/control/:path*",
    "/kiosk/settings/:path*",
    "/kiosk/shutdown/:path*",
    "/kiosk/debug/:path*",
    "/kiosk/fleet/:path*",
  ],
};
