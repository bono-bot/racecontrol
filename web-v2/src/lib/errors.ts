import { NextResponse } from "next/server";

/*
 * BONO-AI proactive seed per R-41+R-42 narrative spec (Captain sweep-ratify 2026-05-19 ~14:39 IST).
 * When Replit mirrors the canonical reference handler bundle from coordinator/reference-handlers/
 * (private rp-v2-apps, branch replit/coordinator), THIS file is the supersede target — keep the
 * signatures stable so the replacement is a 1:1 drop-in.
 *
 * Invariant #7 of the 8 (R-41 §"The 8 pattern invariants"):
 *   ErrorBody discipline — customer `message` vs `staff_detail`, no stack traces leaked.
 */

export interface ErrorBody {
  error: string;
  message: string;
  staff_detail?: string;
  field?: string;
}

export type ErrorStatus = 400 | 401 | 403 | 404 | 409 | 422 | 429 | 500 | 503;

export class HandlerError extends Error {
  readonly status: ErrorStatus;
  readonly body: ErrorBody;

  constructor(status: ErrorStatus, body: ErrorBody) {
    super(body.error);
    this.status = status;
    this.body = body;
  }
}

export function errorResponse(status: ErrorStatus, body: ErrorBody): NextResponse {
  return NextResponse.json(body, { status });
}

type HandlerFn = (req: Request, ctx?: unknown) => Promise<NextResponse>;

export function safeHandler(fn: HandlerFn): HandlerFn {
  return async (req, ctx) => {
    try {
      return await fn(req, ctx);
    } catch (e) {
      if (e instanceof HandlerError) {
        return errorResponse(e.status, e.body);
      }
      const unhandled: ErrorBody = {
        error: "internal_error",
        message: "Something went wrong on our end. Please try again.",
        staff_detail: e instanceof Error ? e.message : String(e),
      };
      console.error("[web-v2 unhandled]", e);
      return errorResponse(500, unhandled);
    }
  };
}
