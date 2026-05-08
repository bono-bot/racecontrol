# Racing Point — Brand Assets

**Status:** Created 2026-05-08 IST per Captain ask "create a separate file or location for all logos that we create."

**Canonical source:** This directory IS the canonical store for Racing Point brand assets. Reference from this dir, do not duplicate into per-app `public/` folders unless framework requires (Next.js apps may copy at build time via script or symlink).

**Composes-with:** `comms-link/v2-skeleton/10-ui-design-system.md` (V2 design substrate — RATIFIED 2026-05-08) · `racecontrol/CLAUDE.md` Brand Identity section · Substrate-Pointer Convention (`racecontrol/CLAUDE.md` Doctrine Conventions section).

---

## Structure

```
brand-assets/
├── README.md           ← this file
└── logos/
    └── racing-point-logo-light.png   ← primary mark for dark backgrounds (50KB, source: Emergent CDN preserved 2026-05-08 IST)
```

Future expansion (add as variants arrive):
```
logos/
├── racing-point-logo-light.png       (current; for dark backgrounds)
├── racing-point-logo-dark.png        (TBD; for light backgrounds)
├── racing-point-logo-monogram.svg    (TBD; mark only, no wordmark)
├── racing-point-logo-wordmark.svg    (TBD; wordmark only)
└── racing-point-logo-lockup.svg      (TBD; full lockup as vector)
```

---

## File: `logos/racing-point-logo-light.png`

| Attribute | Value |
|---|---|
| Source | Emergent CDN — `customer-assets.emergentagent.com/job_racing-pod-display/artifacts/ddz4s1co_racing-point-logo-light.png` |
| Preserved | 2026-05-08 ~17:42 IST |
| Format | PNG, image/png |
| Size | 50,475 bytes |
| Tone | Light (white/light elements on transparent bg, designed for dark surfaces) |
| Use | Primary mark for V2 dark-themed surfaces (kiosk pod display, admin, POS, PWA dark sections) |

**Provenance:** Captain uploaded this asset directly to Emergent.sh during the V2.0 pod display MVP build (2026-05-08 16:30-17:30 IST window). Preserved here to (a) survive Emergent project retention windows and (b) be the canonical source for the existing `racecontrol/kiosk/public/rp-logo-blanking.png` and any future `<BrandLogo />` real-mark swap-in.

**Emergent build preservation:** V2.0 pod display MVP saved at commit `7adf3c12` (Captain Save-to-GitHub triggered 2026-05-08 ~18:35 IST). Reference build IDE at `https://vscode-449a0dc2-07cb-417a-a975-6a82a2184710.preview.emergentagent.com/`; live preview at `https://racing-pod-display.preview.emergentagent.com/`. The Emergent build references this asset at `/app/frontend/public/brand/racing-point-logo-light.png` (cached locally on Emergent infra; identical bytes to the file in this dir).

---

## Usage

**For Next.js apps in this monorepo** (kiosk / web / web-v2 / pwa / admin):

Two patterns acceptable:

1. **Build-time copy** (recommended for production): a build script copies the relevant variant into the app's `public/` dir during `next build`. Single source of truth, app-relative URL at runtime.
2. **Direct relative reference** (acceptable for development): import path from app code, e.g. `import logo from "../../brand-assets/logos/racing-point-logo-light.png"` — Next.js handles via `next/image`.

**For the V2 `<BrandLogo />` placeholder swap-in** (per `comms-link/v2-skeleton/10-ui-design-system.md`):

When SVG variants land, replace the `<PlaceholderMark />` child component in any `<BrandLogo />`-using app with an `<img src="/brand-assets/logos/racing-point-logo-monogram.svg" />` or equivalent. Single-file edit per app. The component contract (`variant` / `tone` / `size` props) does not change.

---

## Stale-at

Update when:
- New logo variants arrive (light/dark/monogram/wordmark SVG) — add to `logos/` and update structure table above
- Captain decides to migrate the existing `kiosk/public/rp-logo-blanking.png` into this dir (currently untouched; deployed kiosk reference still points to its existing location until migration phase)
- Branding refresh / new mark — preserve old in `logos/archive/<date>/` before replacing
- A build script lands that auto-copies these into per-app `public/` dirs

---

## Empirical anchor

Captain ask 2026-05-08 ~15:00 IST: *"let's create a separate file or location for all logos that we create."*

Logo asset upload to Emergent: 2026-05-08 ~17:24 IST (per CDN Last-Modified header on the source URL).

Preservation to local repo: 2026-05-08 ~17:42 IST (this commit).

Composes with the V2 unified brand theme ratify same session (`comms-link/V2-MASTER-STATE.md` §S-114; commits `5cdf4b1a` comms-link + `f9346911` + `6206de26` racecontrol).
