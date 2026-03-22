# Technology Stack

**Project:** v19.0 Cafe Inventory, Ordering & Marketing
**Researched:** 2026-03-22
**Overall confidence:** HIGH

## Existing Stack (Do Not Change)

Already in place -- v19.0 builds on top of these:

| Technology | Version | Purpose |
|------------|---------|---------|
| Next.js | 16.1.6 | Web dashboard (:3200), admin panel |
| React | 19.2.3 | UI framework (web + PWA) |
| Tailwind CSS | 4.x | Styling |
| Rust/Axum | stable | Backend API server (:8080) |
| SQLite (via sqlx) | -- | Database |
| sharp | latest | Image processing (already in PWA) |
| WhatsApp (Evolution API) | -- | Customer/staff messaging |
| Gmail OAuth | -- | Email alerts |

## New Libraries Required

### 1. PDF Import: `pdf-parse`

| | |
|---|---|
| **Package** | `pdf-parse` |
| **Version** | `^2.4.5` |
| **Confidence** | HIGH |
| **Purpose** | Extract menu items from PDF uploads (name, price, category, cost price) |
| **Why this** | Pure TypeScript, cross-platform, zero native deps. v2.x is a complete rewrite with table extraction support. Works in Node.js 20+. Simple API: `const data = await pdfParse(buffer)` returns text that can be line-parsed for menu data. |
| **Why not alternatives** | `unpdf` -- overkill for simple text extraction. `pdfreader` -- lower-level coordinate-based API adds complexity for a menu PDF that's essentially a text list. `pdf2json` -- JSON output format adds unnecessary parsing step. |

**Install:** `npm install pdf-parse`

### 2. Spreadsheet Import: `exceljs`

| | |
|---|---|
| **Package** | `exceljs` |
| **Version** | `^4.4.0` |
| **Confidence** | HIGH |
| **Purpose** | Parse .xlsx/.csv menu uploads into structured item data |
| **Why this** | Read + write support (useful for export later), streaming for large files, TypeScript types included, handles .xlsx and .csv. Actively maintained with 3M+ weekly downloads. Row iteration API is clean: `worksheet.eachRow((row) => ...)`. |
| **Why not alternatives** | `SheetJS (xlsx)` -- community edition has license restrictions for commercial use. ExcelJS is MIT-licensed with no commercial gotchas. `node-xlsx` -- read-only wrapper around SheetJS, same license concern. `csv-parse` -- CSV only, need Excel support too. |

**Install:** `npm install exceljs`

### 3. Marketing Image Generation: `satori` + `@resvg/resvg-js`

| | |
|---|---|
| **Packages** | `satori` + `@resvg/resvg-js` |
| **Versions** | `satori@^0.19.2` + `@resvg/resvg-js@^2.6.2` |
| **Confidence** | HIGH |
| **Purpose** | Generate promo graphics, menu images, Instagram story cards, in-store digital posters |
| **Why this** | Satori converts JSX/HTML+CSS to SVG (Vercel-backed, battle-tested in OG image generation). resvg-js converts SVG to PNG via Rust bindings -- fast and high quality. Together they let you define marketing templates as React-like JSX and render to PNG. No headless browser needed. |
| **Why not alternatives** | `puppeteer/playwright` -- headless Chrome is a 200MB+ dependency, slow startup, memory-hungry. Overkill for templated graphics. `canvas (node-canvas)` -- imperative drawing API, painful for layout-heavy designs. `sharp` alone -- can composite images and overlay SVG text, but building complex layouts (multi-element promo cards) in raw SVG is tedious. Satori gives you CSS flexbox layout in SVG. |

**Install:** `npm install satori @resvg/resvg-js`

**Note:** Satori requires font files loaded as ArrayBuffer. Use Montserrat (body) and a bold display font for headers. Download .ttf/.woff files and load at startup.

### 4. Promo Scheduling: `croner`

| | |
|---|---|
| **Package** | `croner` |
| **Version** | `^10.0.1` |
| **Confidence** | HIGH |
| **Purpose** | Schedule happy hour activation/deactivation, promo start/end times, low-stock alert checks |
| **Why this** | TypeScript-native, zero dependencies, DST-aware, handles timezone (IST). Used by PM2 and Uptime Kuma in production. Cron expression + callback pattern is simple. Has `protect: true` to prevent overlapping runs. |
| **Why not alternatives** | `node-cron` -- no built-in timezone handling, no TypeScript types (needs @types), no overlap protection. `node-schedule` -- heavier, less actively maintained. For this use case (4-5 scheduled checks), croner is the right weight. |

**Install:** `npm install croner`

### 5. Thermal Receipt Printing: `node-thermal-printer`

| | |
|---|---|
| **Package** | `node-thermal-printer` |
| **Version** | `^4.6.0` |
| **Confidence** | MEDIUM |
| **Purpose** | Print cafe order receipts on existing POS thermal printer |
| **Why this** | Supports Epson, Star, Brother, Custom printers. Network (TCP), USB, and serial connections. Builder-pattern API: `printer.println("Item").drawLine().cut()`. QR code support built in. Active maintenance (last publish: 1 month ago). |
| **Why not alternatives** | `escpos` -- last published 6 years ago (v3.0.0-alpha.6), effectively abandoned. `esc-pos-encoder` -- officially deprecated, replaced by `@point-of-sale/receipt-printer-encoder` which is more complex. `@point-of-sale/receipt-printer-encoder` -- lower level, requires you to manage the connection layer separately. |

**MEDIUM confidence because:** Receipt printing depends on the specific printer model at POS. Need to verify the printer make/model and connection type (USB vs network) during implementation. The library itself is solid, but hardware integration always needs live testing.

**Install:** `npm install node-thermal-printer`

### 6. Unique ID Generation: `nanoid`

| | |
|---|---|
| **Package** | `nanoid` |
| **Version** | Already in PWA (`pwa/node_modules/nanoid`) |
| **Confidence** | HIGH |
| **Purpose** | Generate receipt numbers and order IDs (e.g., `ORD-V1StGXR8_Z`) |
| **Why this** | Already a transitive dependency. Compact, URL-safe, collision-resistant. Custom alphabet support for human-readable order IDs. |
| **Why not alternatives** | `uuid` -- 36 chars is too long for receipt numbers. `cuid2` -- fine but nanoid is already present. |

**Install:** Already available. Use directly: `import { nanoid, customAlphabet } from 'nanoid'`

## Libraries NOT Needed (Already Have or Built-in)

| Capability | Already Covered By | Notes |
|---|---|---|
| HTTP API | Rust/Axum backend | Cafe API endpoints added to existing server |
| Database | SQLite via sqlx | New tables: `cafe_items`, `cafe_orders`, `cafe_order_items`, `cafe_stock`, `cafe_promos` |
| WhatsApp alerts | comms-link + Evolution API | Low-stock alerts use existing `send-message.js` |
| Email alerts | Gmail OAuth (existing) | Reuse existing email infrastructure |
| Image optimization | sharp (in PWA already) | Resize/compress marketing images after satori generates them |
| Toast notifications | sonner (in PWA already) | Order confirmation, stock alerts in UI |
| Charts/reports | recharts (in PWA already) | Sales analytics if needed |

## Architecture Decision: Where New Code Lives

| Component | Location | Rationale |
|---|---|---|
| Cafe API endpoints | `crates/racecontrol/src/cafe.rs` | Rust backend -- consistent with existing billing/wallet code, same auth, same DB |
| Menu/order data | SQLite tables | Same DB as drivers, sessions, wallet -- enables JOIN for unified billing |
| Admin UI (menu mgmt, inventory, promos) | `web/src/app/cafe/` | Next.js admin dashboard -- consistent with existing admin pages |
| Customer menu browsing + ordering | `pwa/src/app/cafe/` | PWA -- consistent with existing customer-facing features |
| PDF/spreadsheet import | `web/src/app/api/cafe/import/` | Next.js API route -- handles file upload, parses, POSTs to Rust backend |
| Marketing image generation | `web/src/app/api/cafe/marketing/` | Next.js API route -- satori runs server-side, returns PNG |
| Receipt printing | `web/src/lib/receipt-printer.ts` | Shared lib called from POS order flow |
| Promo scheduler | `web/src/lib/promo-scheduler.ts` | Runs in Next.js server process, activates/deactivates promos on schedule |

## Alternatives Considered (Full Matrix)

| Category | Recommended | Alternative | Why Not |
|----------|-------------|-------------|---------|
| PDF parsing | pdf-parse | unpdf, pdfreader | Simpler API, sufficient for text-based menu PDFs |
| Spreadsheet | exceljs | SheetJS (xlsx) | SheetJS community license is restrictive for commercial use |
| Image gen | satori + resvg-js | puppeteer, canvas | No headless browser overhead; JSX templating is natural for React devs |
| Scheduling | croner | node-cron, node-schedule | TypeScript-native, timezone-aware, overlap protection |
| Receipt print | node-thermal-printer | escpos, esc-pos-encoder | Actively maintained, multi-printer support, high-level API |
| Order IDs | nanoid | uuid, cuid2 | Already in dependency tree, compact output |

## Installation Commands

```bash
# In web/ directory (admin dashboard + API routes)
cd C:/Users/bono/racingpoint/racecontrol/web
npm install pdf-parse exceljs satori @resvg/resvg-js croner node-thermal-printer

# In pwa/ directory (customer-facing, if menu browsing needs any new deps)
# No new deps needed -- PWA consumes API, no server-side processing
```

## Dev Dependencies (if needed)

```bash
# Types for pdf-parse (if not bundled in v2.x)
npm install -D @types/pdf-parse
```

## Version Pinning Strategy

Pin major versions with caret (`^`) to get patches automatically:
- `pdf-parse@^2.4.5` -- major v2 rewrite, don't drift to v3 accidentally
- `exceljs@^4.4.0` -- stable, mature
- `satori@^0.19.2` -- pre-1.0, but Vercel maintains it actively
- `@resvg/resvg-js@^2.6.2` -- stable, Rust bindings
- `croner@^10.0.1` -- v10 is current major
- `node-thermal-printer@^4.6.0` -- v4 is current major

## Risk Assessment

| Library | Risk | Mitigation |
|---------|------|------------|
| satori (pre-1.0) | API changes between minor versions | Pin to `~0.19.2` if stability is critical; test image output in CI |
| node-thermal-printer | Hardware-dependent | Test with actual POS printer early; have fallback to browser `window.print()` for receipt |
| pdf-parse | Menu PDFs vary wildly in format | Build a review step: parse -> show preview -> admin confirms before import |
| @resvg/resvg-js | Native binary (Rust/NAPI) | Windows x64 prebuilt exists (`@resvg/resvg-js-win32-x64-msvc`); already proven pattern with sharp |

## Sources

- [pdf-parse npm](https://www.npmjs.com/package/pdf-parse) -- v2.4.5, pure TypeScript rewrite
- [exceljs npm](https://www.npmjs.com/package/exceljs) -- v4.4.0, MIT license
- [satori GitHub](https://github.com/vercel/satori) -- v0.19.2, HTML/CSS to SVG
- [@resvg/resvg-js npm](https://www.npmjs.com/package/@resvg/resvg-js) -- v2.6.2, SVG to PNG
- [croner npm](https://www.npmjs.com/package/croner) -- v10.0.1, TypeScript cron scheduler
- [node-thermal-printer npm](https://www.npmjs.com/package/node-thermal-printer) -- v4.6.0, ESC/POS thermal printing
- [PkgPulse: PDF parsing comparison](https://www.pkgpulse.com/blog/unpdf-vs-pdf-parse-vs-pdfjs-dist-pdf-parsing-extraction-nodejs-2026)
- [PkgPulse: Scheduling comparison](https://www.pkgpulse.com/blog/node-cron-vs-node-schedule-vs-croner-task-scheduling-nodejs-2026)
- [npm-compare: Excel libraries](https://npm-compare.com/excel4node,exceljs,xlsx,xlsx-populate)
- [Satori + resvg image generation guide](https://anasrin.dev/blog/generate-image-from-html-using-satori-and-resvg/)
- [sharp compositing docs](https://sharp.pixelplumbing.com/api-composite/)
