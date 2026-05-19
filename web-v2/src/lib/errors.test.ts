/**
 * errors.ts — HandlerError + safeHandler invariant tests.
 *
 * R-41 invariant #7 (ErrorBody discipline): customer `message` vs
 * `staff_detail`, no stack traces leaked.
 */

import { describe, it, expect } from "vitest";
import { NextResponse } from "next/server";
import { HandlerError, errorResponse, safeHandler } from "./errors";

describe("errors · HandlerError", () => {
  it("carries status + body", () => {
    const err = new HandlerError(403, {
      error: "forbidden",
      message: "You don't have permission.",
      staff_detail: "actor=customer required=staff",
    });
    expect(err.status).toBe(403);
    expect(err.body.error).toBe("forbidden");
    expect(err.body.staff_detail).toContain("actor=customer");
  });

  it("extends Error so try/catch instanceof works", () => {
    const err = new HandlerError(400, { error: "bad", message: "x" });
    expect(err).toBeInstanceOf(Error);
    expect(err).toBeInstanceOf(HandlerError);
  });
});

describe("errors · errorResponse", () => {
  it("returns NextResponse with the given status", async () => {
    const r = errorResponse(429, { error: "throttled", message: "wait" });
    expect(r).toBeInstanceOf(NextResponse);
    expect(r.status).toBe(429);
    const body = await r.json();
    expect(body.error).toBe("throttled");
  });
});

describe("errors · safeHandler", () => {
  it("passes through successful responses unchanged", async () => {
    const wrapped = safeHandler(async () => NextResponse.json({ ok: true }, { status: 200 }));
    const r = await wrapped(new Request("http://x"));
    expect(r.status).toBe(200);
    const body = await r.json();
    expect(body.ok).toBe(true);
  });

  it("converts thrown HandlerError into its declared response", async () => {
    const wrapped = safeHandler(async () => {
      throw new HandlerError(404, { error: "not_found", message: "missing" });
    });
    const r = await wrapped(new Request("http://x"));
    expect(r.status).toBe(404);
    const body = await r.json();
    expect(body.error).toBe("not_found");
  });

  it("converts unknown thrown Error into 500 with staff_detail but customer-safe message", async () => {
    const wrapped = safeHandler(async () => {
      throw new Error("connection refused at 127.0.0.1:5432");
    });
    const r = await wrapped(new Request("http://x"));
    expect(r.status).toBe(500);
    const body = await r.json();
    expect(body.error).toBe("internal_error");
    expect(body.message).not.toContain("127.0.0.1");
    expect(body.message).not.toContain("connection refused");
    expect(body.staff_detail).toContain("connection refused");
  });

  it("converts thrown non-Error values into 500 with stringified staff_detail", async () => {
    const wrapped = safeHandler(async () => {
      throw "string-not-error";
    });
    const r = await wrapped(new Request("http://x"));
    expect(r.status).toBe(500);
    const body = await r.json();
    expect(body.staff_detail).toContain("string-not-error");
  });
});
