import { NextResponse } from "next/server";

/**
 * WS-HARDEN: Cache-busting middleware for HTML pages.
 *
 * Prevents browser from caching HTML responses (which contain JS chunk references).
 * After a deploy with new chunks, stale cached HTML would reference old JS files
 * that no longer exist, causing the app to break or use stale WS code.
 *
 * Static assets (_next/static/) have content-hash filenames and are cached separately.
 */
export function middleware() {
  const response = NextResponse.next();
  response.headers.set("Cache-Control", "no-cache, no-store, must-revalidate");
  response.headers.set("Pragma", "no-cache");
  return response;
}

export const config = {
  matcher: [
    "/((?!_next/static|_next/image|favicon.ico|api/).*)",
  ],
};
