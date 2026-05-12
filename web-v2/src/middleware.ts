import { NextResponse } from "next/server";
import type { NextRequest } from "next/server";

// UI-SPEC v0.2 §9 Q-CUST-2 — returning-customer detection (AUTONOMOUS-LOCKED).
// First-visit: set `rp_returning=1` cookie (90d TTL · path=/ · sameSite=Lax · httpOnly · secure).
// Subsequent visits: page server component reads cookie via `next/headers` → swap Hero CTA.
const RP_RETURNING_COOKIE = "rp_returning";
const RP_RETURNING_TTL_SECONDS = 60 * 60 * 24 * 90; // 90 days

export function middleware(req: NextRequest) {
  const response = NextResponse.next();
  if (!req.cookies.get(RP_RETURNING_COOKIE)) {
    response.cookies.set({
      name: RP_RETURNING_COOKIE,
      value: "1",
      path: "/",
      sameSite: "lax",
      httpOnly: true,
      secure: true,
      maxAge: RP_RETURNING_TTL_SECONDS,
    });
  }
  return response;
}

// Run on page routes only — skip Next.js internals, API routes, static assets.
// `/` listed explicitly because the negative-lookahead pattern requires ≥1 char
// after `/`, leaving the V2 root uncovered (the highest-traffic surface).
// Anchor: UI-REVIEW.md §Q-CUST-2 BLOCK-1, 2026-05-12.
export const config = {
  matcher: ["/", "/((?!_next|api|favicon\\.ico|.*\\.png|.*\\.jpg|.*\\.svg).*)"],
};
