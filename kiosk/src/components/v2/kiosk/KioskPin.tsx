'use client';

// Racing Point V2 Kiosk — PIN gate (step 0 of 13)
// Source: claude.ai/design handoff bundle (PyiT2ipTf0VdYL8V9vd7-A) — 2026-05-02
// 4-digit PIN, keyboard-first: digits 0-9 type, Backspace deletes, Enter submits, Escape clears.
// Auto-submits on 4th digit. Wrong PIN shakes the panel and clears. After 3 fails, shows
// remaining-attempts indicator (real lockout TBD — not in design demo).
//
// Mode 'unlock'  — staff unlocking pod from idle. "Forgot PIN?" link present.
// Mode 'console' — mid-session operator override. No recovery path; "CALL MANAGER" hint.

import * as React from 'react';
import { RP, FONT, t } from '../tokens';
import { KioskFrame } from './KioskFrame';

const CORRECT_PIN = '4271'; // demo PIN — replace with server-side validation when wiring

export function KioskPin({
  mode = 'unlock',
  onSuccess,
  onForgot,
}: {
  mode?: 'unlock' | 'console';
  onSuccess: () => void;
  onForgot?: () => void;
}) {
  const [pin, setPin] = React.useState('');
  const [shake, setShake] = React.useState(false);
  const [fails, setFails] = React.useState(0);

  const tryPin = React.useCallback((p: string) => {
    if (p.length !== 4) return;
    if (p === CORRECT_PIN) {
      onSuccess();
    } else {
      setShake(true);
      setFails(f => f + 1);
      window.setTimeout(() => {
        setShake(false);
        setPin('');
      }, 400);
    }
  }, [onSuccess]);

  React.useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key >= '0' && e.key <= '9') {
        setPin(p => (p + e.key).slice(0, 4));
      } else if (e.key === 'Backspace') {
        setPin(p => p.slice(0, -1));
      } else if (e.key === 'Enter') {
        setPin(p => { tryPin(p); return p; });
      } else if (e.key === 'Escape') {
        setPin('');
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [tryPin]);

  React.useEffect(() => {
    if (pin.length === 4) tryPin(pin);
  }, [pin, tryPin]);

  const heading = mode === 'console' ? 'Operator console' : 'Staff PIN required';
  const sub = mode === 'console'
    ? 'Customer cannot pause or end the session — staff only.'
    : 'Pod is locked. An operator must unlock to start a session.';

  return (
    <KioskFrame
      podId="POD-01"
      state={mode === 'console' ? 'live' : 'idle'}
      hideStatus={mode === 'unlock'}
    >
      <div style={{
        height: '100%', display: 'flex', alignItems: 'center', justifyContent: 'center',
        padding: 40,
      }}>
        <div style={{
          width: 480, textAlign: 'center',
          animation: shake ? 'rp-shake 0.4s' : 'none',
        }}>
          <div style={{
            ...t('caption'), fontSize: 11,
            color: mode === 'console' ? RP.amber : RP.gunmetal,
          }}>
            {mode === 'console' ? '● MID-SESSION OVERRIDE' : 'POD-01 · STANDBY'}
          </div>
          <div style={{
            ...t('displayM'), fontFamily: FONT.display, fontSize: 36,
            color: RP.ink, marginTop: 10,
          }}>
            {heading}
          </div>
          <div style={{
            ...t('body'), fontSize: 14, color: RP.inkDim, marginTop: 8,
          }}>
            {sub}
          </div>

          {/* PIN dots */}
          <div style={{ marginTop: 36, display: 'flex', gap: 14, justifyContent: 'center' }}>
            {[0, 1, 2, 3].map(i => (
              <div key={i} style={{
                width: 56, height: 64,
                border: `2px solid ${pin.length > i ? RP.red : RP.border}`,
                background: RP.card,
                display: 'flex', alignItems: 'center', justifyContent: 'center',
                ...t('mono'), fontSize: 28, color: RP.ink,
              }}>
                {pin.length > i ? '●' : ''}
              </div>
            ))}
          </div>

          <div style={{
            ...t('mono'), fontSize: 10, color: RP.gunmetal, marginTop: 18,
            letterSpacing: '0.12em',
          }}>
            TYPE 4-DIGIT PIN · ENTER TO CONFIRM · ESC TO CLEAR
          </div>

          {fails > 0 && (
            <div style={{
              ...t('caption'), fontSize: 10, color: RP.red, marginTop: 14,
            }}>
              ● INCORRECT PIN · {Math.max(0, 3 - fails)} attempts remaining
            </div>
          )}

          <div style={{
            marginTop: 36, display: 'flex', justifyContent: 'center', gap: 12,
          }}>
            {mode === 'unlock' ? (
              <button
                onClick={onForgot}
                style={{
                  background: 'transparent', color: RP.inkDim, border: 'none',
                  cursor: 'pointer',
                  ...t('bodySm'), fontSize: 12,
                  textDecoration: 'underline', textUnderlineOffset: 4,
                }}
              >
                Forgot PIN?
              </button>
            ) : (
              <div style={{
                ...t('mono'), fontSize: 10, color: RP.gunmetal,
                letterSpacing: '0.08em',
              }}>
                FORGOT PIN MID-SESSION? · CALL MANAGER
              </div>
            )}
          </div>
        </div>
      </div>
    </KioskFrame>
  );
}
