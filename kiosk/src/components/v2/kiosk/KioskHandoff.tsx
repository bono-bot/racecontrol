'use client';

// Racing Point V2 Kiosk — Pod-handed-off confirmation
// Source: claude.ai/design handoff bundle (PyiT2ipTf0VdYL8V9vd7-A) — 2026-05-02

import * as React from 'react';
import { RP, FONT, t } from '../tokens';
import { PrimaryCTA } from '../atomic';
import { KioskFrame } from './KioskFrame';
import { KioskField, type KioskGame } from './KioskShared';

const DEFAULT_GAME: KioskGame = { id: 'ac', code: 'AC', label: 'Assetto Corsa' };

export function KioskHandoff({
  driverName = 'Uday Singh',
  game = DEFAULT_GAME,
  mode,
  onContinue,
}: {
  driverName?: string;
  game?: KioskGame;
  mode?: 'sp' | 'mp';
  onContinue: () => void;
}) {
  const sub = game.id === 'ac'
    ? (mode === 'mp' ? 'Multiplayer · joining lobby' : 'Single player · custom setup')
    : 'Press Start to begin';

  return (
    <KioskFrame podId="POD-01" state="live">
      <div style={{
        height: '100%', display: 'flex', flexDirection: 'column',
        alignItems: 'center', justifyContent: 'center', padding: 40, textAlign: 'center',
      }}>
        <div style={{ ...t('caption'), fontSize: 11, color: RP.green }}>● POD READY · HAND TO DRIVER</div>
        <div style={{
          ...t('displayL'), fontFamily: FONT.display, fontSize: 56, color: RP.ink,
          marginTop: 14, lineHeight: 1, letterSpacing: '0.02em', whiteSpace: 'nowrap',
        }}>READY TO RACE</div>
        <div style={{ ...t('h2'), fontSize: 16, color: RP.inkDim, marginTop: 14, fontWeight: 400 }}>
          {game.label} is launching on the racing rig.
        </div>

        <div style={{
          marginTop: 32, display: 'grid', gridTemplateColumns: 'repeat(3, auto)', gap: 32,
          padding: '18px 28px', border: `1px solid ${RP.border}`, background: RP.base,
        }}>
          <KioskField label="DRIVER" value={driverName} />
          <KioskField label="GAME" value={game.label} sub={sub} />
          <KioskField label="WALLET" value="₹420" sub="≈ 63 min · ₹100/15min" big />
        </div>

        <div style={{ ...t('mono'), fontSize: 10, color: RP.gunmetal, marginTop: 28, letterSpacing: '0.06em' }}>
          WALLET BURNS PER MINUTE WHEN YOU PRESS START · STAFF CAN PAUSE OR TOP UP ANY TIME
        </div>
        <div style={{ marginTop: 16 }}>
          <PrimaryCTA onClick={onContinue} shortcut="ENTER">Start session →</PrimaryCTA>
        </div>
      </div>
    </KioskFrame>
  );
}
