'use client';

// Racing Point V2 Kiosk — shared chrome wrapper
// Source: claude.ai/design handoff bundle (PyiT2ipTf0VdYL8V9vd7-A) — 2026-05-02
// Top bar: Racing Point logo + pod id + state dot + optional logout chip.
// Footer: VMS v2.0 venue label + mouse+keyboard hint.

import * as React from 'react';
import { RP, FONT, t } from '../tokens';
import { StatusDot } from '../atomic';

export type KioskFrameState = 'idle' | 'live' | 'fault';

export const KioskCtx = React.createContext<{ onLogout: (() => void) | null }>({
  onLogout: null,
});

export function KioskFrame({
  podId = 'POD-01',
  state = 'idle',
  hideStatus,
  onLogout,
  children,
}: {
  podId?: string;
  state?: KioskFrameState;
  hideStatus?: boolean;
  onLogout?: () => void;
  children: React.ReactNode;
}) {
  const ctx = React.useContext(KioskCtx);
  const logout = onLogout ?? ctx.onLogout;

  return (
    <div style={{
      width: '100%', height: '100%', background: RP.asphalt, color: RP.ink,
      fontFamily: FONT.body, display: 'flex', flexDirection: 'column',
      position: 'relative', overflow: 'hidden',
    }}>
      {/* Subtle racing-stripe overlay */}
      <div style={{
        position: 'absolute', inset: 0, opacity: 0.04, pointerEvents: 'none',
        background: `repeating-linear-gradient(115deg, transparent 0 80px, ${RP.red} 80px 81px, transparent 81px 200px)`,
      }} />

      {/* Top status strip */}
      <div style={{
        height: 40, background: RP.base, borderBottom: `1px solid ${RP.border}`,
        display: 'flex', alignItems: 'center', padding: '0 20px', gap: 14,
        flexShrink: 0, zIndex: 1,
      }}>
        {/* Logo placeholder — design has a PNG; we use text mark until logo asset is wired. */}
        <span style={{
          ...t('caption'), fontFamily: FONT.display, fontSize: 13, color: RP.red,
          letterSpacing: '0.15em',
        }}>
          RACING<span style={{ color: RP.ink }}>POINT</span>
        </span>
        <div style={{ flex: 1 }} />
        {!hideStatus && (
          <>
            <div style={{ ...t('mono'), fontSize: 11, color: RP.inkDim }}>{podId}</div>
            <StatusDot
              state={state === 'live' ? 'green' : state === 'fault' ? 'red' : 'grey'}
              pulse={state === 'live'}
            />
            <div style={{
              ...t('caption'), fontSize: 10,
              color: state === 'live' ? RP.green : state === 'fault' ? RP.red : RP.inkDim,
            }}>
              {state === 'live' ? 'SESSION LIVE' : state === 'fault' ? 'FAULT' : 'IDLE'}
            </div>
          </>
        )}
        {logout && (
          <button
            onClick={logout}
            title="Lock pod and return to PIN gate"
            style={{
              marginLeft: 14, background: 'transparent', color: RP.inkDim,
              border: `1px solid ${RP.border}`, padding: '4px 10px', cursor: 'pointer',
              ...t('mono'), fontSize: 10, letterSpacing: '0.08em',
              display: 'inline-flex', alignItems: 'center', gap: 6,
            }}
          >
            <span style={{
              width: 6, height: 6, borderRadius: '50%', background: RP.amber,
            }} />
            LOG OUT
          </button>
        )}
      </div>

      {/* Body */}
      <div style={{ flex: 1, minHeight: 0, position: 'relative', zIndex: 1 }}>
        {children}
      </div>

      {/* Footer */}
      <div style={{
        height: 28, background: RP.base, borderTop: `1px solid ${RP.border}`,
        display: 'flex', alignItems: 'center', justifyContent: 'space-between',
        padding: '0 20px', flexShrink: 0,
        ...t('mono'), fontSize: 9, color: RP.gunmetal, zIndex: 1,
      }}>
        <span>VMS v2.0 · Bandlaguda · Kiosk</span>
        <span>STAFF-OPERATED · MOUSE + KEYBOARD</span>
      </div>
    </div>
  );
}
