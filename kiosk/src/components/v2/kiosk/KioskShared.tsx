'use client';

// Racing Point V2 Kiosk — shared types + small components used by 2+ screens
// Source: claude.ai/design handoff bundle (PyiT2ipTf0VdYL8V9vd7-A) — 2026-05-02

import * as React from 'react';
import { RP, FONT, t } from '../tokens';

export interface KioskGame {
  id: string;
  code: string;
  label: string;
  sub?: string;
  installed?: boolean;
  branch?: 'ac' | 'simple';
}

export const ALL_GAMES: KioskGame[] = [
  { id: 'ac',     code: 'AC',  label: 'Assetto Corsa',       sub: 'Mods, modes, full setup',                installed: true,  branch: 'ac' },
  { id: 'acevo',  code: 'EVO', label: 'Assetto Corsa EVO',   sub: 'Hyper-realistic physics · Early Access', installed: true,  branch: 'simple' },
  { id: 'acrally',code: 'RAL', label: 'Assetto Corsa Rally', sub: 'Stage rally · Loose surfaces',           installed: false, branch: 'simple' },
  { id: 'iracing',code: 'iR',  label: 'iRacing',             sub: 'Online championships · Subscription',    installed: true,  branch: 'simple' },
  { id: 'lmu',    code: 'LMU', label: 'Le Mans Ultimate',    sub: 'Endurance prototypes · Hypercar',        installed: true,  branch: 'simple' },
  { id: 'f125',   code: 'F1',  label: 'F1 25',               sub: 'Career · Time trial · Grand Prix',       installed: true,  branch: 'simple' },
  { id: 'nascar', code: 'NSC', label: 'NASCAR',              sub: 'Stock cars · Ovals',                     installed: false, branch: 'simple' },
  { id: 'forza',  code: 'FH5', label: 'Forza Horizon 5',     sub: 'Open-world arcade',                      installed: true,  branch: 'simple' },
];

export function GameBadge({ code, small }: { code: string; small?: boolean }) {
  const COLORS: Record<string, string> = { iR: '#ff5500', ACC: '#1e90ff', rF2: '#9c27b0', AMS2: '#00d26a' };
  return (
    <div style={{
      width: small ? 28 : 44, height: small ? 18 : 36,
      background: RP.base, border: `1px solid ${COLORS[code] || RP.border}`,
      display: 'flex', alignItems: 'center', justifyContent: 'center',
      fontFamily: FONT.mono, fontSize: small ? 9 : 11, fontWeight: 700,
      color: COLORS[code] || RP.inkDim, letterSpacing: '0.04em',
      flexShrink: 0,
    }}>{code}</div>
  );
}

export function KioskField({ label, value, sub, big }: {
  label: string;
  value: React.ReactNode;
  sub?: React.ReactNode;
  big?: boolean;
}) {
  return (
    <div style={{ textAlign: 'left' }}>
      <div style={{ ...t('caption'), fontSize: 9, color: RP.gunmetal }}>{label}</div>
      <div style={{ ...t('mono'), fontSize: big ? 22 : 13, color: big ? RP.red : RP.ink, marginTop: 4 }}>{value}</div>
      {sub && <div style={{ ...t('mono'), fontSize: 9, color: RP.inkDim, marginTop: 2 }}>{sub}</div>}
    </div>
  );
}
