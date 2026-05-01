'use client';

// Racing Point V2 Kiosk — Driver registration (name + autocomplete)
// Source: claude.ai/design handoff bundle (PyiT2ipTf0VdYL8V9vd7-A) — 2026-05-02

import * as React from 'react';
import { RP, FONT, t } from '../tokens';
import { Icon, PrimaryCTA, SecondaryCTA } from '../atomic';
import { KioskFrame } from './KioskFrame';

interface Suggestion {
  name: string;
  mail: string;
  cls: string;
  laps: number;
}

const SUGGESTIONS: Suggestion[] = [
  { name: 'Uday Singh',  mail: 'u***gh@rac****oint.in', cls: 'Apex',   laps: 142 },
  { name: 'Uday Sharma', mail: 'us****ma@gmail.com',    cls: 'Rookie', laps: 8 },
  { name: 'Udayan K.',   mail: 'ud***@yah***.com',      cls: 'Rookie', laps: 22 },
];

export function KioskRegister({ onContinue, onBack }: {
  onContinue: (driverName: string) => void;
  onBack: () => void;
}) {
  const [name, setName] = React.useState('Uday Singh');

  return (
    <KioskFrame podId="POD-01" state="idle">
      <div style={{ height: '100%', display: 'flex', flexDirection: 'column', padding: '24px 40px', overflow: 'auto' }}>
        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
          <div style={{ ...t('caption'), fontSize: 10, color: RP.gunmetal }}>STEP 1 OF 3 · STAFF</div>
          <SecondaryCTA onClick={onBack} shortcut="ESC">Cancel</SecondaryCTA>
        </div>
        <div style={{ ...t('displayM'), fontFamily: FONT.display, fontSize: 32, color: RP.ink, marginTop: 8 }}>Who&apos;s driving?</div>
        <div style={{ ...t('body'), fontSize: 13, color: RP.inkDim, marginTop: 4 }}>
          Type the driver&apos;s name. Existing drivers match as you type. New drivers are created on the fly.
        </div>

        <div style={{ marginTop: 24, maxWidth: 720 }}>
          <div style={{ ...t('caption'), fontSize: 10, color: RP.gunmetal, marginBottom: 6 }}>DRIVER NAME</div>
          <input
            value={name}
            onChange={e => setName(e.target.value)}
            autoFocus
            style={{
              width: '100%', background: RP.card, border: `2px solid ${RP.red}`, padding: '14px 16px',
              ...t('h2'), fontSize: 20, color: RP.ink, fontFamily: FONT.body, outline: 'none',
            }}
          />
        </div>

        <div style={{ ...t('caption'), fontSize: 10, color: RP.gunmetal, marginTop: 22, marginBottom: 8 }}>MATCHES · 3</div>
        <div style={{ display: 'flex', flexDirection: 'column', gap: 6, maxWidth: 720 }}>
          {SUGGESTIONS.map((s, i) => (
            <div key={i} style={{
              padding: '12px 16px', background: i === 0 ? 'rgba(225,6,0,0.08)' : RP.card,
              border: `1px solid ${i === 0 ? RP.red : RP.border}`, cursor: 'pointer',
              display: 'flex', alignItems: 'center', gap: 14,
            }}>
              <div style={{ flex: 1 }}>
                <div style={{ ...t('h2'), fontSize: 14, color: RP.ink }}>{s.name}</div>
                <div style={{ ...t('mono'), fontSize: 10, color: RP.inkDim, marginTop: 2 }}>{s.mail}</div>
              </div>
              <div style={{ ...t('caption'), fontSize: 9, color: RP.amber }}>{s.cls.toUpperCase()}</div>
              <div style={{ ...t('mono'), fontSize: 10, color: RP.gunmetal }}>{s.laps} LAPS</div>
              {i === 0 && <Icon name="check" size={14} color={RP.red} />}
            </div>
          ))}
          <div style={{
            padding: '10px 16px', ...t('bodySm'), fontSize: 11, color: RP.inkDim,
            border: `1px dashed ${RP.border}`, cursor: 'pointer',
          }}>
            + Create new driver &quot;{name}&quot;
          </div>
        </div>

        <div style={{ flex: 1 }} />
        <div style={{ display: 'flex', alignItems: 'center', gap: 10, marginTop: 16 }}>
          <div style={{ ...t('mono'), fontSize: 10, color: RP.gunmetal }}>↑↓ NAVIGATE · ENTER TO CONTINUE</div>
          <div style={{ flex: 1 }} />
          <PrimaryCTA onClick={() => onContinue(name)} shortcut="ENTER">Continue → Pick game</PrimaryCTA>
        </div>
      </div>
    </KioskFrame>
  );
}
