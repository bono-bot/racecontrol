import { NextResponse } from "next/server";

/*
 * POST /v2/api/v2/marketing/whatsapp-optin
 *
 * V0 STUB — acknowledges receipt with 202. Future Phase: proxy to api-gateway
 * canonical endpoint per UI-SPEC v0.2 §9 Q-CUST-4 (a) CAPTAIN-RATIFIED 2026-05-12:
 *   POST {api-gateway}/api/v2/marketing/whatsapp-optin
 *   payload: { phone, consent_text, consent_ts, source: "v2-landing" }
 *   audit-log per §S-158: customer_consent_change action_type with phone_hash + version + source_surface
 *
 * Tracked separately: in-flight ledger entry `legal-amplifier-queue-qcust4-qcust7-wording-20260512-1110-IST`.
 * api-gateway endpoint construction is a separate child phase (not part of page.tsx authoring scope).
 */

interface OptInPayload {
  phone?: unknown;
  consent_text?: unknown;
  consent_ts?: unknown;
  source?: unknown;
}

function isValidPayload(body: OptInPayload): body is {
  phone: string;
  consent_text: string;
  consent_ts: string;
  source: string;
} {
  return (
    typeof body.phone === "string" &&
    body.phone.trim().length >= 8 &&
    typeof body.consent_text === "string" &&
    body.consent_text.length > 0 &&
    typeof body.consent_ts === "string" &&
    typeof body.source === "string"
  );
}

export async function POST(request: Request) {
  let body: OptInPayload;
  try {
    body = (await request.json()) as OptInPayload;
  } catch {
    return NextResponse.json(
      { error: "invalid_json" },
      { status: 400 }
    );
  }

  if (!isValidPayload(body)) {
    return NextResponse.json(
      { error: "invalid_payload" },
      { status: 400 }
    );
  }

  // V0 stub: log to server stdout for now; future proxy to api-gateway will
  // populate whatsapp-bot DB + §S-158 audit_log.
  // eslint-disable-next-line no-console
  console.log(
    "[whatsapp-optin v0 stub]",
    JSON.stringify({
      source: body.source,
      consent_ts: body.consent_ts,
      // phone intentionally NOT logged in stub to avoid stub-time PII leak in
      // server logs; real api-gateway path stores in DB with phone_hash audit.
    })
  );

  return NextResponse.json(
    {
      status: "accepted",
      note: "v0 stub — api-gateway proxy pending (separate child phase)",
    },
    { status: 202 }
  );
}
