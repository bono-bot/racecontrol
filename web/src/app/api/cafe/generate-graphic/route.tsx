/**
 * POST /api/cafe/generate-graphic
 *
 * Generates a PNG graphic from a promo/menu/new-item template using satori + resvg-wasm.
 * Returns a binary PNG stream suitable for download or direct use in WhatsApp broadcasts.
 *
 * Brand identity: Racing Red #E10600, Asphalt Black #1A1A1A, Montserrat font.
 */

import React from "react";
import { NextRequest } from "next/server";
import path from "path";
import fs from "fs";
import satori, { type Font } from "satori";
import { initWasm, Resvg } from "@resvg/resvg-wasm";

// ─── Types ────────────────────────────────────────────────────────────────────

type TemplateType = "promo" | "daily_menu" | "new_item";

interface GenerateGraphicRequest {
  template: TemplateType;
  promo_id?: string;
  item_ids?: string[];
  promo_name?: string;
  promo_description?: string;
  price_label?: string;
  time_label?: string;
}

interface CafeItemSummary {
  id: string;
  name: string;
  selling_price_paise: number;
  is_available: boolean;
}

interface CafeItemsResponse {
  items: CafeItemSummary[];
}

// ─── WASM Init ────────────────────────────────────────────────────────────────

let wasmInitialized = false;

async function ensureWasm(): Promise<void> {
  if (!wasmInitialized) {
    // Read the WASM binary directly from the node_modules filesystem.
    // This avoids Turbopack's WASM module resolution issues.
    const wasmPath = path.join(
      process.cwd(),
      "node_modules/@resvg/resvg-wasm/index_bg.wasm"
    );
    const wasmBuffer: Buffer = fs.readFileSync(wasmPath);
    await initWasm(wasmBuffer);
    wasmInitialized = true;
  }
}

// ─── Font Loading ─────────────────────────────────────────────────────────────

let fontRegular: ArrayBuffer | null = null;
let fontBold: ArrayBuffer | null = null;
let fontExtraBold: ArrayBuffer | null = null;

async function loadFonts(): Promise<void> {
  if (fontRegular && fontBold && fontExtraBold) return;

  const [regular, bold, extrabold] = await Promise.all([
    fetch(
      "https://fonts.gstatic.com/s/montserrat/v26/JTUSjIg1_i6t8kCHKm459WlhyyTh89Y.woff"
    ).then((r) => r.arrayBuffer()),
    fetch(
      "https://fonts.gstatic.com/s/montserrat/v26/JTUSjIg1_i6t8kCHKm459Wlhyyv.woff"
    ).then((r) => r.arrayBuffer()),
    fetch(
      "https://fonts.gstatic.com/s/montserrat/v26/JTUSjIg1_i6t8kCHKm459Wdhyyv.woff"
    ).then((r) => r.arrayBuffer()),
  ]);

  fontRegular = regular;
  fontBold = bold;
  fontExtraBold = extrabold;
}

function getFontConfig(): Font[] {
  if (!fontRegular || !fontBold || !fontExtraBold) {
    throw new Error("Fonts not loaded");
  }
  return [
    { name: "Montserrat", data: fontRegular, weight: 400, style: "normal" },
    { name: "Montserrat", data: fontBold, weight: 700, style: "normal" },
    { name: "Montserrat", data: fontExtraBold, weight: 800, style: "normal" },
  ];
}

// ─── Item Fetching ────────────────────────────────────────────────────────────

async function fetchItems(itemIds: string[]): Promise<CafeItemSummary[]> {
  if (itemIds.length === 0) return [];

  const internalSecret = process.env.INTERNAL_API_SECRET ?? "";
  const baseUrl = process.env.RACECONTROL_INTERNAL_URL ?? "http://localhost:8080";

  const resp = await fetch(`${baseUrl}/api/v1/cafe/items`, {
    headers: {
      Authorization: `Bearer ${internalSecret}`,
    },
  });

  if (!resp.ok) {
    return [];
  }

  const data: CafeItemsResponse = (await resp.json()) as CafeItemsResponse;
  const allItems: CafeItemSummary[] = data.items ?? [];
  return allItems.filter((item: CafeItemSummary) => itemIds.includes(item.id));
}

// ─── Price Formatter ──────────────────────────────────────────────────────────

function formatPrice(paise: number): string {
  const rupees = Math.floor(paise / 100);
  const remainder = paise % 100;
  if (remainder === 0) return `₹${rupees}`;
  return `₹${rupees}.${String(remainder).padStart(2, "0")}`;
}

// ─── Template Builders ────────────────────────────────────────────────────────

/** 1080×1920 Promo template */
function buildPromoTemplate(
  promoName: string,
  description: string | undefined,
  priceLabel: string | undefined,
  timeLabel: string | undefined
): React.ReactElement {
  return (
    <div
      style={{
        display: "flex",
        flexDirection: "column",
        width: 1080,
        height: 1920,
        backgroundColor: "#1A1A1A",
        fontFamily: "Montserrat",
      }}
    >
      {/* Top strip */}
      <div
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          backgroundColor: "#E10600",
          height: 120,
          width: "100%",
        }}
      >
        <span
          style={{
            color: "#FFFFFF",
            fontSize: 36,
            fontWeight: 800,
            letterSpacing: 3,
            textTransform: "uppercase",
          }}
        >
          RACING POINT ESPORTS & CAFE
        </span>
      </div>

      {/* Center content */}
      <div
        style={{
          display: "flex",
          flexDirection: "column",
          alignItems: "center",
          justifyContent: "center",
          flex: 1,
          padding: "80px 100px",
        }}
      >
        <div
          style={{
            display: "flex",
            flexDirection: "column",
            alignItems: "center",
            backgroundColor: "#222222",
            borderRadius: 24,
            padding: "80px 80px",
            width: "100%",
            gap: 32,
          }}
        >
          {/* Promo name */}
          <span
            style={{
              color: "#FFFFFF",
              fontSize: 64,
              fontWeight: 700,
              textAlign: "center",
            }}
          >
            {promoName}
          </span>

          {/* Price label */}
          {priceLabel !== undefined && priceLabel !== "" && (
            <span
              style={{
                color: "#E10600",
                fontSize: 88,
                fontWeight: 800,
                textAlign: "center",
              }}
            >
              {priceLabel}
            </span>
          )}

          {/* Time label badge */}
          {timeLabel !== undefined && timeLabel !== "" && (
            <div
              style={{
                display: "flex",
                alignItems: "center",
                backgroundColor: "#2A2A00",
                borderRadius: 100,
                padding: "12px 32px",
              }}
            >
              <span
                style={{
                  color: "#FFD700",
                  fontSize: 32,
                  fontWeight: 600,
                }}
              >
                {timeLabel}
              </span>
            </div>
          )}

          {/* Description */}
          {description !== undefined && description !== "" && (
            <span
              style={{
                color: "#888888",
                fontSize: 36,
                fontWeight: 400,
                textAlign: "center",
                lineHeight: 1.5,
              }}
            >
              {description}
            </span>
          )}
        </div>
      </div>

      {/* Bottom strip */}
      <div
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          backgroundColor: "#E10600",
          height: 100,
          width: "100%",
        }}
      >
        <span
          style={{
            color: "#FFFFFF",
            fontSize: 36,
            fontWeight: 500,
          }}
        >
          Book your session today
        </span>
      </div>
    </div>
  ) as React.ReactElement;
}

/** 1080×1080 Daily menu template */
function buildDailyMenuTemplate(
  items: CafeItemSummary[]
): React.ReactElement {
  const displayItems = items.slice(0, 6);
  const hasItems = displayItems.length > 0;

  return (
    <div
      style={{
        display: "flex",
        flexDirection: "column",
        width: 1080,
        height: 1080,
        backgroundColor: "#1A1A1A",
        fontFamily: "Montserrat",
        padding: "80px 80px",
        gap: 48,
      }}
    >
      {/* Title */}
      <div
        style={{
          display: "flex",
          justifyContent: "center",
        }}
      >
        <span
          style={{
            color: "#E10600",
            fontSize: 80,
            fontWeight: 800,
            textTransform: "uppercase",
            letterSpacing: 4,
          }}
        >
          TODAY&apos;S MENU
        </span>
      </div>

      {/* Divider */}
      <div
        style={{
          display: "flex",
          height: 4,
          backgroundColor: "#E10600",
          borderRadius: 2,
        }}
      />

      {/* Items or placeholder */}
      {hasItems ? (
        <div
          style={{
            display: "flex",
            flexDirection: "column",
            gap: 32,
            flex: 1,
          }}
        >
          {displayItems.map((item: CafeItemSummary) => (
            <div
              key={item.id}
              style={{
                display: "flex",
                justifyContent: "space-between",
                alignItems: "center",
                padding: "24px 0",
                borderBottom: "1px solid #333333",
              }}
            >
              <span
                style={{
                  color: "#FFFFFF",
                  fontSize: 40,
                  fontWeight: 500,
                }}
              >
                {item.name}
              </span>
              <span
                style={{
                  color: "#888888",
                  fontSize: 36,
                  fontWeight: 400,
                }}
              >
                {formatPrice(item.selling_price_paise)}
              </span>
            </div>
          ))}
        </div>
      ) : (
        <div
          style={{
            display: "flex",
            flex: 1,
            alignItems: "center",
            justifyContent: "center",
          }}
        >
          <span
            style={{
              color: "#888888",
              fontSize: 36,
              fontWeight: 400,
              textAlign: "center",
            }}
          >
            {"Today's Selection — Visit us to see what's fresh!"}
          </span>
        </div>
      )}

      {/* Footer */}
      <div
        style={{
          display: "flex",
          justifyContent: "center",
        }}
      >
        <span
          style={{
            color: "#5A5A5A",
            fontSize: 28,
            fontWeight: 400,
          }}
        >
          RACING POINT ESPORTS &amp; CAFE
        </span>
      </div>
    </div>
  ) as React.ReactElement;
}

/** 1080×1920 New item template */
function buildNewItemTemplate(
  itemName: string,
  description: string | undefined
): React.ReactElement {
  return (
    <div
      style={{
        display: "flex",
        flexDirection: "column",
        width: 1080,
        height: 1920,
        backgroundColor: "#1A1A1A",
        fontFamily: "Montserrat",
      }}
    >
      {/* Top strip */}
      <div
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          backgroundColor: "#E10600",
          height: 120,
          width: "100%",
        }}
      >
        <span
          style={{
            color: "#FFFFFF",
            fontSize: 36,
            fontWeight: 800,
            letterSpacing: 3,
            textTransform: "uppercase",
          }}
        >
          RACING POINT ESPORTS & CAFE
        </span>
      </div>

      {/* Center content */}
      <div
        style={{
          display: "flex",
          flexDirection: "column",
          alignItems: "center",
          justifyContent: "center",
          flex: 1,
          padding: "80px 100px",
          gap: 40,
        }}
      >
        {/* New arrival badge */}
        <span
          style={{
            color: "#E10600",
            fontSize: 64,
            fontWeight: 800,
            textTransform: "uppercase",
            letterSpacing: 6,
          }}
        >
          NEW ARRIVAL
        </span>

        {/* Item name */}
        <span
          style={{
            color: "#FFFFFF",
            fontSize: 72,
            fontWeight: 700,
            textAlign: "center",
          }}
        >
          {itemName}
        </span>

        {/* Description */}
        {description !== undefined && description !== "" && (
          <span
            style={{
              color: "#888888",
              fontSize: 40,
              fontWeight: 400,
              textAlign: "center",
              lineHeight: 1.5,
            }}
          >
            {description}
          </span>
        )}
      </div>

      {/* Bottom strip */}
      <div
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          backgroundColor: "#E10600",
          height: 100,
          width: "100%",
        }}
      >
        <span
          style={{
            color: "#FFFFFF",
            fontSize: 36,
            fontWeight: 500,
          }}
        >
          Book your session today
        </span>
      </div>
    </div>
  ) as React.ReactElement;
}

// ─── SVG → PNG ────────────────────────────────────────────────────────────────

async function svgToPng(svg: string): Promise<Buffer> {
  await ensureWasm();
  const resvg = new Resvg(svg);
  const pngData = resvg.render();
  const pngUint8: Uint8Array = pngData.asPng();
  return Buffer.from(pngUint8);
}

// ─── Route Handler ────────────────────────────────────────────────────────────

export async function POST(request: NextRequest): Promise<Response> {
  let body: GenerateGraphicRequest;

  try {
    body = (await request.json()) as GenerateGraphicRequest;
  } catch {
    return new Response(JSON.stringify({ error: "Invalid JSON body" }), {
      status: 400,
      headers: { "Content-Type": "application/json" },
    });
  }

  const { template } = body;

  if (!["promo", "daily_menu", "new_item"].includes(template)) {
    return new Response(
      JSON.stringify({
        error: 'template must be one of: promo, daily_menu, new_item',
      }),
      {
        status: 400,
        headers: { "Content-Type": "application/json" },
      }
    );
  }

  try {
    await loadFonts();

    let width: number;
    let height: number;
    let element: React.ReactElement;

    if (template === "promo") {
      width = 1080;
      height = 1920;
      const promoName = body.promo_name ?? "Special Offer";
      element = buildPromoTemplate(
        promoName,
        body.promo_description,
        body.price_label,
        body.time_label
      );
    } else if (template === "daily_menu") {
      width = 1080;
      height = 1080;
      const itemIds = body.item_ids ?? [];
      const items = await fetchItems(itemIds);
      element = buildDailyMenuTemplate(items);
    } else {
      // new_item
      width = 1080;
      height = 1920;
      const itemName = body.promo_name ?? "New Item";
      element = buildNewItemTemplate(itemName, body.promo_description);
    }

    const svg = await satori(element, {
      width,
      height,
      fonts: getFontConfig(),
    });

    const pngBuffer: Buffer = await svgToPng(svg);

    return new Response(pngBuffer.buffer as ArrayBuffer, {
      headers: {
        "Content-Type": "image/png",
        "Content-Disposition": 'attachment; filename="promo.png"',
        "Content-Length": String(pngBuffer.byteLength),
      },
    });
  } catch (err: unknown) {
    const message = err instanceof Error ? err.message : "Unknown error";
    return new Response(JSON.stringify({ error: message }), {
      status: 500,
      headers: { "Content-Type": "application/json" },
    });
  }
}
