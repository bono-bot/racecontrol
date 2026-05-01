'use client';

// Racing Point V2 Kiosk — Operator console (mid-session OR post-session staff actions)
// Source: claude.ai/design handoff bundle (PyiT2ipTf0VdYL8V9vd7-A) — 2026-05-02

import * as React from 'react';
import { RP, FONT, t } from '../tokens';
import { Icon } from '../atomic';
import { KioskFrame } from './KioskFrame';
import { KioskField, type KioskGame } from './KioskShared';

const DEFAULT_GAME: KioskGame = { id: 'ac', code: 'AC', label: 'Assetto Corsa' };

export type KioskConsoleMode = 'live' | 'paused' | 'finished';

export function KioskConsole({
  mode = 'live',
  driverName = 'Uday Singh',
  game = DEFAULT_GAME,
  onPause,
  onAbort,
  onEnd,
  onResume,
  onSwitchGame,
}: {
  mode?: KioskConsoleMode;
  driverName?: string;
  game?: KioskGame;
  onPause?: () => void;
  onAbort?: () => void;
  onEnd?: () => void;
  onResume?: () => void;
  onSwitchGame?: () => void;
}) {
  return (
    <KioskFrame podId="POD-01" state={mode === 'live' ? 'live' : 'idle'}>
      <div style={{ height: '100%', display: 'flex', flexDirection: 'column', padding: '24px 40px' }}>
        <div style={{ ...t('caption'), fontSize: 10, color: mode === 'live' ? RP.amber : RP.red }}>
          ● OPERATOR CONSOLE · POD-01
        </div>
        <div style={{ ...t('displayM'), fontFamily: FONT.display, fontSize: 28, color: RP.ink, marginTop: 6 }}>
          {mode === 'live' ? 'Session in progress' : mode === 'paused' ? 'Session paused' : 'Session ended'}
        </div>

        <div style={{
          marginTop: 16, padding: '14px 18px',
          background: RP.base, border: `1px solid ${RP.border}`,
          display: 'grid', gridTemplateColumns: 'repeat(4, 1fr)', gap: 18,
        }}>
          <KioskField label="DRIVER" value={driverName} />
          <KioskField label="GAME" value={game.label} sub={game.id === 'ac' ? 'Spa · GT3' : '2021 F1 Practice'} />
          <KioskField label="ELAPSED" value="04:03" sub="Billed per minute" />
          <KioskField label="PROJECTED COST" value="₹405" sub="@ ₹100 / 15min" />
        </div>

        <div style={{ marginTop: 24, ...t('caption'), fontSize: 10, color: RP.gunmetal }}>STAFF ACTIONS</div>
        <div style={{ marginTop: 8, display: 'grid', gridTemplateColumns: 'repeat(2, 1fr)', gap: 10 }}>
          {mode === 'live' && (
            <>
              <ConsoleAction icon="pulse" label="Pause session"   desc="Customer can rest. Timer halts." onClick={onPause} accent={RP.amber} />
              <ConsoleAction icon="eye"   label="Blank screen"    desc="Hide telemetry (during dispute)." accent={RP.inkDim} />
              <ConsoleAction icon="x"     label="End & invoice"   desc="Stop session. Generate invoice for billed minutes." onClick={onAbort} accent={RP.red} />
            </>
          )}
          {mode === 'paused' && (
            <>
              <ConsoleAction icon="play"    label="Resume session" desc="Continue racing. Timer resumes." onClick={onResume} accent={RP.green} />
              <ConsoleAction icon="refresh" label="Switch game"    desc={`Save current ${game.label} results · keep timer · pick new title.`} onClick={onSwitchGame} accent={RP.amber} />
              <ConsoleAction icon="x"       label="End & invoice"  desc="End now. Invoice for billed minutes." onClick={onAbort} accent={RP.red} />
            </>
          )}
          {mode === 'finished' && (
            <>
              <ConsoleAction icon="check"   label="View results & lock"     desc="Show podium, save laps, lock pod." onClick={onEnd} accent={RP.green} />
              <ConsoleAction icon="refresh" label="Restart with same driver" desc="New 15-min session, same settings." accent={RP.inkDim} />
            </>
          )}
        </div>

        <div style={{ flex: 1 }} />
        <div style={{ ...t('mono'), fontSize: 10, color: RP.gunmetal, textAlign: 'right' }}>
          AUTO-CLOSE IN 60s · CUSTOMER CAN&apos;T SEE THIS PANEL
        </div>
      </div>
    </KioskFrame>
  );
}

function ConsoleAction({ icon, label, desc, onClick, accent }: {
  icon: string;
  label: string;
  desc: string;
  onClick?: () => void;
  accent: string;
}) {
  return (
    <div onClick={onClick} style={{
      padding: '14px 16px', background: RP.card, border: `1px solid ${RP.border}`,
      borderLeft: `2px solid ${accent}`,
      cursor: onClick ? 'pointer' : 'default',
      display: 'flex', alignItems: 'flex-start', gap: 12,
    }}>
      <Icon name={icon} size={18} color={accent} />
      <div>
        <div style={{ ...t('h2'), fontSize: 13, color: RP.ink }}>{label}</div>
        <div style={{ ...t('bodySm'), fontSize: 11, color: RP.inkDim, marginTop: 2 }}>{desc}</div>
      </div>
    </div>
  );
}
