// Racing Point V2 — Design tokens (TypeScript port of design bundle's tokens.jsx)
// Source: claude.ai/design handoff bundle (PyiT2ipTf0VdYL8V9vd7-A) — 2026-05-02
// Approach: lift design tokens as TS constants; components use inline styles to match
// design JSX 1:1 (no Tailwind class translation for v2.0; refactor to classes later).

export const RP = {
  // ── Brand ──────────────────────────────────────────────────
  red:        '#E10600',  // Racing Red — primary action
  redDeep:    '#B40500',  // pressed / hover-down
  redGlow:    '#FF1A0E',  // alert pulse, telemetry highlights

  // ── Surfaces (dark-first) ──────────────────────────────────
  asphalt:    '#0D0D0D',  // base bg
  base:       '#1A1A1A',  // panel bg
  card:       '#1C1C1C',  // elevated surface
  cardHi:     '#262626',  // hover / focus surface
  border:     '#2A2A2A',  // subtle separator
  borderHi:   '#3A3A3A',  // emphasized separator

  // ── Text ───────────────────────────────────────────────────
  ink:        '#F2F2F2',
  inkDim:     '#A8A8A8',
  gunmetal:   '#5A5A5A',
  inkFaint:   '#3F3F3F',

  // ── Semantic ───────────────────────────────────────────────
  green:      '#00D26A',
  greenDeep:  '#00A352',
  amber:      '#FFB000',
  amberDeep:  '#CC8C00',
  redSem:     '#E10600',
  blue:       '#3B82F6',

  // ── Telemetry channel colors (for charts) ─────────────────
  trThrottle: '#00D26A',
  trBrake:    '#E10600',
  trSpeed:    '#FFB000',
  trGhost:    '#888888',
  trSelf:     '#FFFFFF',

  // ── Driver class palette (Rookie / Apex / Podium / Champion) ──
  classRookie:   '#5A5A5A',
  classApex:     '#FFB000',
  classPodium:   '#00D26A',
  classChampion: '#E10600',
} as const;

// ── Spacing (4px base) ───────────────────────────────────────
export const SP = {
  0: 0, 1: 4, 2: 8, 3: 12, 4: 16, 5: 20, 6: 24, 8: 32, 10: 40, 12: 48, 16: 64, 20: 80,
} as const;

// ── Border radius ────────────────────────────────────────────
export const RADIUS = {
  none: 0,
  sm: 2,
  md: 4,
  lg: 6,
  pill: 999,
} as const;

// ── Type scale ───────────────────────────────────────────────
export const FONT = {
  display: '"Chakra Petch", "Eurostile", "Bank Gothic", system-ui, sans-serif',
  body: '"Montserrat", system-ui, -apple-system, sans-serif',
  mono: '"JetBrains Mono", ui-monospace, "SF Mono", Menlo, monospace',
} as const;

type TypeToken = {
  font: string;
  size: number;
  weight: number;
  tracking: string;
  height: number;
  upper?: boolean;
};

export const TYPE: Record<string, TypeToken> = {
  displayXL: { font: FONT.display, size: 64, weight: 700, tracking: '-0.02em', height: 1.0 },
  displayL:  { font: FONT.display, size: 48, weight: 700, tracking: '-0.01em', height: 1.05 },
  displayM:  { font: FONT.display, size: 32, weight: 600, tracking: '0',       height: 1.1  },
  h1:        { font: FONT.display, size: 28, weight: 700, tracking: '0.01em',  height: 1.15, upper: true },
  h2:        { font: FONT.display, size: 20, weight: 600, tracking: '0.04em',  height: 1.2,  upper: true },
  h3:        { font: FONT.display, size: 14, weight: 600, tracking: '0.08em',  height: 1.2,  upper: true },
  body:      { font: FONT.body,    size: 14, weight: 400, tracking: '0',       height: 1.5  },
  bodyB:     { font: FONT.body,    size: 14, weight: 600, tracking: '0',       height: 1.5  },
  bodySm:    { font: FONT.body,    size: 12, weight: 400, tracking: '0',       height: 1.4  },
  caption:   { font: FONT.body,    size: 11, weight: 600, tracking: '0.06em',  height: 1.2,  upper: true },
  mono:      { font: FONT.mono,    size: 13, weight: 500, tracking: '0',       height: 1.3  },
  monoBig:   { font: FONT.mono,    size: 24, weight: 600, tracking: '-0.02em', height: 1.0  },
};

// Helper to apply a type token as a style object
export function t(key: keyof typeof TYPE): React.CSSProperties {
  const x = TYPE[key];
  return {
    fontFamily: x.font,
    fontSize: x.size,
    fontWeight: x.weight,
    letterSpacing: x.tracking,
    lineHeight: x.height,
    ...(x.upper ? { textTransform: 'uppercase' as const } : {}),
    margin: 0,
  };
}

// ── Elevation ────────────────────────────────────────────────
export const ELEV = {
  flat:  '0 1px 0 rgba(255,255,255,0.02) inset',
  card:  '0 1px 0 rgba(255,255,255,0.03) inset, 0 4px 12px rgba(0,0,0,0.4)',
  dialog:'0 1px 0 rgba(255,255,255,0.04) inset, 0 24px 60px rgba(0,0,0,0.6)',
  glow:  (color: string) => `0 0 0 1px ${color}, 0 0 24px -4px ${color}`,
} as const;

// ── Animation timings ───────────────────────────────────────
export const MOTION = {
  fast: '150ms cubic-bezier(0.4, 0, 0.2, 1)',
  std:  '250ms cubic-bezier(0.4, 0, 0.2, 1)',
  slow: '400ms cubic-bezier(0.4, 0, 0.2, 1)',
  enter: 'cubic-bezier(0, 0, 0.2, 1)',
  exit:  'cubic-bezier(0.4, 0, 1, 1)',
} as const;

// ── Driver class types ──────────────────────────────────────
export type DriverClass = 'Rookie' | 'Apex' | 'Podium' | 'Champion';

// Need React import for CSSProperties type
import type * as React from 'react';
