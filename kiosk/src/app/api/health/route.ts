import { NextResponse } from "next/server";

export async function GET() {
  return NextResponse.json({
    status: "ok",
    service: "kiosk",
    version: "0.1.0",
  });
}
