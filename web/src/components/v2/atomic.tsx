'use client';

// Racing Point V2 — Atomic components (TypeScript port of design bundle's components.jsx)
// Source: claude.ai/design handoff bundle (PyiT2ipTf0VdYL8V9vd7-A) — 2026-05-02
// All components match design JSX 1:1 with TypeScript types.

import * as React from 'react';
import { RP, FONT, RADIUS, MOTION, t, type DriverClass } from './tokens';

// ── DriverClassBadge ────────────────────────────────────────
export function DriverClassBadge({
  tier = 'Apex',
  size = 'md',
}: {
  tier?: DriverClass;
  size?: 'sm' | 'md';
}) {
  const COLORS = {
    Rookie:   { bg: '#1E1E1E', fg: RP.classRookie,   stripe: RP.classRookie },
    Apex:     { bg: '#1E1A0E', fg: RP.classApex,     stripe: RP.classApex },
    Podium:   { bg: '#0E1A12', fg: RP.classPodium,   stripe: RP.classPodium },
    Champion: { bg: '#1E0A08', fg: RP.classChampion, stripe: RP.classChampion },
  };
  const c = COLORS[tier];
  const small = size === 'sm';
  return (
    <span style={{
      display: 'inline-flex', alignItems: 'center', gap: 6,
      padding: small ? '2px 7px 2px 0' : '4px 10px 4px 0',
      background: c.bg,
      color: c.fg,
      borderLeft: `3px solid ${c.stripe}`,
      paddingLeft: small ? 7 : 10,
      ...t('caption'),
      fontSize: small ? 9 : 10,
    }}>
      {tier}
    </span>
  );
}

// ── StatusDot ───────────────────────────────────────────────
export function StatusDot({
  state = 'green',
  pulse = false,
}: {
  state?: 'green' | 'amber' | 'red' | 'grey';
  pulse?: boolean;
}) {
  const COLORS = { green: RP.green, amber: RP.amber, red: RP.red, grey: RP.gunmetal };
  const c = COLORS[state];
  return (
    <span style={{
      display: 'inline-block', width: 8, height: 8, borderRadius: '50%',
      background: c,
      boxShadow: pulse ? `0 0 0 0 ${c}` : 'none',
      animation: pulse ? 'rp-pulse 1.6s ease-out infinite' : 'none',
    }} />
  );
}

// ── StatusTile ──────────────────────────────────────────────
export function StatusTile({
  label,
  value,
  sub,
  accent,
  mono = true,
  trailing,
}: {
  label: string;
  value: React.ReactNode;
  sub?: React.ReactNode;
  accent?: string;
  mono?: boolean;
  trailing?: React.ReactNode;
}) {
  return (
    <div style={{
      background: RP.card,
      border: `1px solid ${RP.border}`,
      borderRadius: RADIUS.sm,
      padding: '14px 16px',
      position: 'relative',
      overflow: 'hidden',
    }}>
      {accent && <div style={{ position: 'absolute', top: 0, left: 0, right: 0, height: 2, background: accent }} />}
      <div style={{ display: 'flex', alignItems: 'flex-start', justifyContent: 'space-between', gap: 8 }}>
        <div style={{ ...t('caption'), color: RP.inkDim }}>{label}</div>
        {trailing}
      </div>
      <div style={{
        ...(mono ? t('monoBig') : t('displayM')),
        color: RP.ink,
        marginTop: 8,
        fontVariantNumeric: 'tabular-nums',
      }}>{value}</div>
      {sub && <div style={{ ...t('bodySm'), color: RP.inkDim, marginTop: 4 }}>{sub}</div>}
    </div>
  );
}

// ── ActionButton ────────────────────────────────────────────
export function ActionButton({
  children,
  variant = 'primary',
  size = 'md',
  icon,
  onClick,
  fullWidth,
  disabled,
}: {
  children: React.ReactNode;
  variant?: 'primary' | 'secondary' | 'ghost' | 'danger' | 'success';
  size?: 'sm' | 'md' | 'lg' | 'xl';
  icon?: React.ReactNode;
  onClick?: () => void;
  fullWidth?: boolean;
  disabled?: boolean;
}) {
  const sizes = {
    sm: { h: 28, px: 10, fs: 11 },
    md: { h: 36, px: 14, fs: 12 },
    lg: { h: 48, px: 20, fs: 13 },  // POS touch target
    xl: { h: 64, px: 24, fs: 14 },  // Kiosk
  };
  const s = sizes[size];
  const variants = {
    primary:   { bg: RP.red,        fg: '#fff',     bd: RP.red,      hbg: RP.redDeep },
    secondary: { bg: 'transparent', fg: RP.ink,     bd: RP.borderHi, hbg: RP.cardHi },
    ghost:     { bg: 'transparent', fg: RP.inkDim,  bd: 'transparent', hbg: RP.cardHi },
    danger:    { bg: 'transparent', fg: RP.red,     bd: RP.red,      hbg: 'rgba(225,6,0,0.08)' },
    success:   { bg: RP.green,      fg: '#000',     bd: RP.green,    hbg: RP.greenDeep },
  };
  const v = variants[variant];
  return (
    <button
      onClick={onClick}
      disabled={disabled}
      style={{
        height: s.h,
        padding: `0 ${s.px}px`,
        background: v.bg,
        color: v.fg,
        border: `1px solid ${v.bd}`,
        borderRadius: RADIUS.lg,
        ...t('caption'),
        fontSize: s.fs,
        cursor: disabled ? 'not-allowed' : 'pointer',
        opacity: disabled ? 0.5 : 1,
        display: 'inline-flex', alignItems: 'center', justifyContent: 'center', gap: 8,
        transition: `all ${MOTION.fast}`,
        width: fullWidth ? '100%' : 'auto',
        whiteSpace: 'nowrap',
      }}
      onMouseEnter={(e) => { if (!disabled) e.currentTarget.style.background = v.hbg; }}
      onMouseLeave={(e) => { if (!disabled) e.currentTarget.style.background = v.bg; }}
    >
      {icon}
      {children}
    </button>
  );
}

// ── Icon (lucide-style line, 1.6px stroke) ──────────────────
const ICON_PATHS: Record<string, string> = {
  bell:     'M6 8a6 6 0 0 1 12 0c0 7 3 9 3 9H3s3-2 3-9 M10.3 21a1.94 1.94 0 0 0 3.4 0',
  pulse:    'M22 12h-4l-3 9L9 3l-3 9H2',
  user:     'M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2 M16 7a4 4 0 1 1-8 0 4 4 0 0 1 8 0',
  search:   'M21 21l-4.3-4.3 M11 19a8 8 0 1 1 0-16 8 8 0 0 1 0 16',
  settings: 'M12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6 M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 1 1-4 0v-.09a1.65 1.65 0 0 0-1-1.51 1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 1 1 0-4h.09a1.65 1.65 0 0 0 1.51-1 1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.65 1.65 0 0 0 1.82.33h.01a1.65 1.65 0 0 0 1-1.51V3a2 2 0 1 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82v.01a1.65 1.65 0 0 0 1.51 1H21a2 2 0 1 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z',
  grid:     'M3 3h7v7H3z M14 3h7v7h-7z M14 14h7v7h-7z M3 14h7v7H3z',
  zap:      'M13 2L3 14h9l-1 8 10-12h-9l1-8z',
  check:    'M20 6L9 17l-5-5',
  x:        'M18 6L6 18 M6 6l12 12',
  chevron:  'M9 18l6-6-6-6',
  chevronD: 'M6 9l6 6 6-6',
  arrow:    'M5 12h14 M12 5l7 7-7 7',
  play:     'M5 3l14 9-14 9z',
  refresh:  'M3 12a9 9 0 0 1 15-6.7L21 8 M21 3v5h-5 M21 12a9 9 0 0 1-15 6.7L3 16 M3 21v-5h5',
  power:    'M12 2v10 M5.6 7.6a9 9 0 1 0 12.8 0',
  skull:    'M9 22v-3 M15 22v-3 M16 13a4 4 0 1 0-8 0 M5 14v-3a7 7 0 1 1 14 0v3a3 3 0 0 1-3 3H8a3 3 0 0 1-3-3z',
  eye:      'M2 12s4-8 10-8 10 8 10 8-4 8-10 8-10-8-10-8z M12 9a3 3 0 1 0 0 6 3 3 0 0 0 0-6',
  phone:    'M22 16.92v3a2 2 0 0 1-2.18 2 19.79 19.79 0 0 1-8.63-3.07 19.5 19.5 0 0 1-6-6A19.79 19.79 0 0 1 2.12 4.18 2 2 0 0 1 4.11 2h3a2 2 0 0 1 2 1.72c.13.96.37 1.9.72 2.81a2 2 0 0 1-.45 2.11L8.09 9.91a16 16 0 0 0 6 6l1.27-1.27a2 2 0 0 1 2.11-.45c.91.35 1.85.59 2.81.72A2 2 0 0 1 22 16.92z',
  camera:   'M23 19a2 2 0 0 1-2 2H3a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h4l2-3h6l2 3h4a2 2 0 0 1 2 2z M12 17a4 4 0 1 0 0-8 4 4 0 0 0 0 8',
  flag:     'M4 15s1-1 4-1 5 2 8 2 4-1 4-1V3s-1 1-4 1-5-2-8-2-4 1-4 1z M4 22V15',
  trophy:   'M6 9H4.5a2.5 2.5 0 0 1 0-5H6 M18 9h1.5a2.5 2.5 0 0 0 0-5H18 M4 22h16 M10 14.66V17c0 .55.47.98.97 1.21C12.15 18.75 13 20.24 13 22 M14 14.66V17c0 .55-.47.98-.97 1.21C11.85 18.75 11 20.24 11 22 M18 2H6v7a6 6 0 0 0 12 0z',
  layers:   'M12 2L2 7l10 5 10-5z M2 17l10 5 10-5 M2 12l10 5 10-5',
  activity: 'M22 12h-4l-3 9L9 3l-3 9H2',
  clock:    'M12 2a10 10 0 1 0 0 20 10 10 0 0 0 0-20z M12 6v6l4 2',
  plus:     'M12 5v14 M5 12h14',
  minus:    'M5 12h14',
  filter:   'M22 3H2l8 9.46V19l4 2v-8.54z',
  download: 'M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4 M7 10l5 5 5-5 M12 15V3',
  wallet:   'M21 12V7H5a2 2 0 0 1 0-4h14v4 M3 5v14a2 2 0 0 0 2 2h16v-5 M18 12a2 2 0 0 0 0 4h4v-4z',
  car:      'M14 16H9m10 0h3v-3.15a1 1 0 0 0-.84-.99L16 11l-2.7-3.6a1 1 0 0 0-.8-.4H5.24a2 2 0 0 0-1.8 1.1l-.8 1.63A6 6 0 0 0 2 12.42V16h2 M6 18a2 2 0 1 0 0-4 2 2 0 0 0 0 4 M18 18a2 2 0 1 0 0-4 2 2 0 0 0 0 4',
  mail:     'M4 4h16c1.1 0 2 .9 2 2v12c0 1.1-.9 2-2 2H4c-1.1 0-2-.9-2-2V6c0-1.1.9-2 2-2z M22 6l-10 7L2 6',
  home:     'M3 9l9-7 9 7v11a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z M9 22V12h6v10',
  target:   'M12 22a10 10 0 1 0 0-20 10 10 0 0 0 0 20z M12 18a6 6 0 1 0 0-12 6 6 0 0 0 0 12z M12 14a2 2 0 1 0 0-4 2 2 0 0 0 0 4',
  lock:     'M5 11h14v10H5z M8 11V7a4 4 0 1 1 8 0v4',
  'arrow-right': 'M5 12h14 M12 5l7 7-7 7',
  gear:     'M12 8v8 M8 12h8 M4 12a8 8 0 1 0 16 0 8 8 0 0 0-16 0',
  gauge:    'M12 14l4-4 M21 12a9 9 0 1 1-18 0 9 9 0 0 1 18 0z M12 14a2 2 0 1 0 0-4 2 2 0 0 0 0 4',
  card:     'M2 7h20v10a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2z M2 11h20',
  alert:    'M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z M12 9v4 M12 17h.01',
};

export function Icon({
  name,
  size = 16,
  color = 'currentColor',
  strokeWidth = 1.6,
}: {
  name: string;
  size?: number;
  color?: string;
  strokeWidth?: number;
}) {
  const d = ICON_PATHS[name];
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke={color}
      strokeWidth={strokeWidth}
      strokeLinecap="round"
      strokeLinejoin="round"
    >
      {d &&
        d.split(' M').map((p, i) => (
          <path key={i} d={i === 0 ? p : 'M' + p} />
        ))}
    </svg>
  );
}

// ── AlertRow ────────────────────────────────────────────────
export function AlertRow({
  severity = 'amber',
  code,
  msg,
  time,
  source,
}: {
  severity?: 'red' | 'amber' | 'blue' | 'green';
  code?: string;
  msg: React.ReactNode;
  time?: string;
  source?: string;
}) {
  const COLORS = { red: RP.red, amber: RP.amber, blue: RP.blue, green: RP.green };
  const c = COLORS[severity];
  return (
    <div style={{
      display: 'grid',
      gridTemplateColumns: '8px 64px 1fr 60px',
      gap: 10,
      alignItems: 'center',
      padding: '8px 12px',
      borderBottom: `1px solid ${RP.border}`,
      fontSize: 12,
    }}>
      <div style={{ width: 4, height: 28, background: c, marginLeft: 0 }} />
      <div style={{ ...t('mono'), color: RP.inkDim, fontSize: 11 }}>{code}</div>
      <div style={{ color: RP.ink, ...t('body'), fontSize: 12, lineHeight: 1.3 }}>
        {msg}
        {source && <span style={{ color: RP.gunmetal, marginLeft: 8 }}>· {source}</span>}
      </div>
      <div style={{ ...t('mono'), fontSize: 11, color: RP.inkDim, textAlign: 'right' }}>{time}</div>
    </div>
  );
}

// ── PodTile (mini, used in cockpit live grid) ───────────────
export type PodState = 'idle' | 'ready' | 'racing' | 'paused' | 'fault';

export function PodTile({
  id,
  state = 'idle',
  driver,
  game,
  elapsed,
  remaining,
}: {
  id: string;
  state?: PodState;
  driver?: string;
  game?: string;
  elapsed?: string;
  remaining?: string;
}) {
  const STATES = {
    idle:   { c: RP.gunmetal, label: 'IDLE' },
    ready:  { c: RP.blue,     label: 'READY' },
    racing: { c: RP.green,    label: 'RACING' },
    paused: { c: RP.amber,    label: 'PAUSED' },
    fault:  { c: RP.red,      label: 'FAULT' },
  };
  const s = STATES[state];
  return (
    <div style={{
      background: RP.card,
      border: `1px solid ${RP.border}`,
      borderTop: `2px solid ${s.c}`,
      padding: '10px 12px',
      display: 'flex', flexDirection: 'column', gap: 6,
      minHeight: 86,
    }}>
      <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
        <div style={{ ...t('h3'), fontSize: 11, color: RP.ink }}>{id}</div>
        <div style={{ ...t('caption'), fontSize: 9, color: s.c }}>{s.label}</div>
      </div>
      <div style={{ ...t('bodyB'), fontSize: 12, color: driver ? RP.ink : RP.gunmetal, lineHeight: 1.2 }}>
        {driver || '—'}
      </div>
      <div style={{ ...t('bodySm'), fontSize: 10, color: RP.inkDim, lineHeight: 1.2 }}>
        {game || ''}
      </div>
      {(elapsed || remaining) && (
        <div style={{ display: 'flex', justifyContent: 'space-between', ...t('mono'), fontSize: 10, color: RP.inkDim, marginTop: 2 }}>
          <span>+{elapsed}</span>
          <span style={{ color: s.c }}>−{remaining}</span>
        </div>
      )}
    </div>
  );
}

// ── PrimaryCTA / SecondaryCTA (kiosk-specific large buttons) ──
export function PrimaryCTA({
  children,
  onClick,
  shortcut = 'ENTER',
  disabled,
}: {
  children: React.ReactNode;
  onClick?: () => void;
  shortcut?: string;
  disabled?: boolean;
}) {
  return (
    <button
      onClick={onClick}
      disabled={disabled}
      style={{
        background: disabled ? RP.borderHi : RP.red,
        color: RP.ink,
        border: 'none',
        padding: '14px 28px',
        cursor: disabled ? 'not-allowed' : 'pointer',
        ...t('h2'),
        fontFamily: FONT.display,
        fontSize: 16,
        letterSpacing: '0.08em',
        display: 'flex',
        alignItems: 'center',
        gap: 14,
        opacity: disabled ? 0.4 : 1,
      }}
    >
      <span>{children}</span>
      <span style={{
        ...t('mono'), fontSize: 10, opacity: 0.7, padding: '3px 7px',
        border: `1px solid rgba(255,255,255,0.3)`,
      }}>{shortcut}</span>
    </button>
  );
}

export function SecondaryCTA({
  children,
  onClick,
  shortcut,
}: {
  children: React.ReactNode;
  onClick?: () => void;
  shortcut?: string;
}) {
  return (
    <button
      onClick={onClick}
      style={{
        background: 'transparent',
        color: RP.inkDim,
        border: `1px solid ${RP.border}`,
        padding: '12px 20px',
        cursor: 'pointer',
        ...t('caption'),
        fontSize: 11,
        letterSpacing: '0.08em',
        display: 'flex',
        alignItems: 'center',
        gap: 10,
      }}
    >
      <span>{children}</span>
      {shortcut && (
        <span style={{
          ...t('mono'), fontSize: 9, color: RP.gunmetal,
          padding: '2px 5px', border: `1px solid ${RP.border}`,
        }}>{shortcut}</span>
      )}
    </button>
  );
}

// Inject keyframes once on first import (client-side only)
if (typeof document !== 'undefined' && !document.getElementById('rp-anim')) {
  const s = document.createElement('style');
  s.id = 'rp-anim';
  s.textContent = `
    @keyframes rp-pulse {
      0% { box-shadow: 0 0 0 0 currentColor; }
      70% { box-shadow: 0 0 0 6px transparent; }
      100% { box-shadow: 0 0 0 0 transparent; }
    }
    @keyframes rp-blink {
      0%, 50% { opacity: 1; } 51%, 100% { opacity: 0.3; }
    }
    @keyframes rp-shake {
      0%, 100% { transform: translateX(0); }
      20%, 60% { transform: translateX(-10px); }
      40%, 80% { transform: translateX(10px); }
    }
    @keyframes rp-slide {
      0% { transform: translateX(-100%); }
      100% { transform: translateX(250%); }
    }
  `;
  document.head.appendChild(s);
}
