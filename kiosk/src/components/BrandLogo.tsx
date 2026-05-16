"use client";

/**
 * BrandLogo — Racing Point brand mark
 *
 * Renders the canonical Racing Point Esports logotype from
 * /brand-assets/logos/racing-point-logo-light.png (staged into
 * kiosk/public/brand/racing-point-logo-light.png at build/dev time).
 *
 * Component contract (matches the brand-assets/README.md expectation for
 * a future <BrandLogo /> swap-in):
 *
 *   - size:     "xs" | "sm" | "md" | "lg" | "xl"  height presets (px)
 *   - tone:     "light" | "dark"  white-on-dark vs dark-on-light;
 *               dark variant is TBD per brand-assets/README.md, falls
 *               back to light with a silent no-op (no console noise).
 *   - subtitle: optional small-caps tagline rendered below the mark in
 *               text-rp-grey uppercase tracking-widest (V2 substrate).
 *   - className: passthrough for layout adjustments (margin, alignment).
 *   - alt:      override default alt; recipe-decorative use should pass "".
 *   - priority: hint for above-the-fold critical assets (no-op for <img>;
 *               retained for forward compat when a <picture>/srcset variant
 *               or <Image> migration lands).
 *
 * Why plain <img> and not next/image: the kiosk is a fixed-resolution
 * display app, not a responsive web surface. The existing brand asset
 * usage at src/app/blanking/page.tsx already uses <img> with eslint-
 * disable for the same reason. Plain <img> avoids next/image's loader
 * config requirements and basePath auto-prefix surprises.
 */

const NATIVE_W = 600;
const NATIVE_H = 273;
const ASPECT = NATIVE_H / NATIVE_W; // ~0.455

const SIZE_HEIGHT_PX: Record<BrandLogoSize, number> = {
  xs: 24,   // tiny — inline w/ body text
  sm: 32,   // header bar (KioskHeader)
  md: 48,   // page-level branding (shutdown, demo, root)
  lg: 80,   // hero on dense forms (register)
  xl: 112,  // hero on lock screen (StaffLoginScreen idle)
};

const SUBTITLE_CLASS: Record<BrandLogoSize, string> = {
  xs: "text-[9px] tracking-[0.2em] mt-1",
  sm: "text-[10px] tracking-widest mt-1.5",
  md: "text-xs tracking-widest mt-2",
  lg: "text-sm tracking-widest mt-3",
  xl: "text-lg tracking-widest mt-3",
};

export type BrandLogoSize = "xs" | "sm" | "md" | "lg" | "xl";
export type BrandLogoTone = "light" | "dark";

export interface BrandLogoProps {
  /** Size preset — controls height (px), width auto-derived from native 600×273 aspect ratio. Default "md". */
  size?: BrandLogoSize;
  /** Color treatment — "light" for dark backgrounds (default, current canonical), "dark" reserved (falls back to light). */
  tone?: BrandLogoTone;
  /** Optional small-caps tagline rendered under the mark in text-rp-grey uppercase. */
  subtitle?: string;
  /** Hint for critical above-the-fold renders. No-op for plain <img>; retained for forward compat. */
  priority?: boolean;
  /** Layout-passthrough class names (e.g. centering, margin). */
  className?: string;
  /** Override default alt text. Pass "" for decorative duplicates (e.g. blanking-page reflection). */
  alt?: string;
}

export function BrandLogo({
  size = "md",
  tone = "light",
  subtitle,
  priority: _priority = false,
  className = "",
  alt = "Racing Point Esports",
}: BrandLogoProps) {
  // tone="dark" reserved per brand-assets/README.md (dark variant TBD).
  // Silent fallback to light — no console noise on a kiosk display.
  void tone;

  const heightPx = SIZE_HEIGHT_PX[size];
  const widthPx = Math.round(heightPx / ASPECT);

  return (
    <span className={`inline-flex flex-col items-center align-baseline ${className}`}>
      {/* eslint-disable-next-line @next/next/no-img-element */}
      <img
        src="/kiosk/brand/racing-point-logo-light.png"
        alt={alt}
        width={widthPx}
        height={heightPx}
        draggable={false}
        className="select-none object-contain block"
        style={{ height: heightPx, width: widthPx }}
      />
      {alt ? <span className="sr-only">{alt}</span> : null}
      {subtitle ? (
        <span
          className={`text-rp-grey uppercase font-semibold ${SUBTITLE_CLASS[size]}`}
        >
          {subtitle}
        </span>
      ) : null}
    </span>
  );
}

export default BrandLogo;
