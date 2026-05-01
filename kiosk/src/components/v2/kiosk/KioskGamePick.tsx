'use client';

// Racing Point V2 Kiosk — Pick game (8 titles; AC = full setup branch)
// Source: claude.ai/design handoff bundle (PyiT2ipTf0VdYL8V9vd7-A) — 2026-05-02

import * as React from 'react';
import { RP, FONT, t } from '../tokens';
import { Icon, PrimaryCTA, SecondaryCTA } from '../atomic';
import { KioskFrame } from './KioskFrame';
import { ALL_GAMES, GameBadge, type KioskGame } from './KioskShared';

export function KioskGamePick({
  driverName = 'Uday Singh',
  onPickAc,
  onPickSimple,
  onBack,
}: {
  driverName?: string;
  onPickAc: (game: KioskGame) => void;
  onPickSimple: (game: KioskGame) => void;
  onBack: () => void;
}) {
  const [sel, setSel] = React.useState(0);
  const game = ALL_GAMES[sel];

  return (
    <KioskFrame podId="POD-01" state="idle">
      <div style={{ height: '100%', display: 'flex', flexDirection: 'column', padding: '20px 36px', overflow: 'auto' }}>
        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
          <div style={{ ...t('caption'), fontSize: 10, color: RP.gunmetal }}>STEP 2 OF 3 · STAFF</div>
          <SecondaryCTA onClick={onBack} shortcut="ESC">Back</SecondaryCTA>
        </div>
        <div style={{ ...t('displayM'), fontFamily: FONT.display, fontSize: 28, color: RP.ink, marginTop: 6 }}>Pick the game</div>
        <div style={{ ...t('body'), fontSize: 12, color: RP.inkDim, marginTop: 2 }}>
          Driver: <span style={{ color: RP.ink }}>{driverName}</span> · Apex class · 142 laps
        </div>

        <div style={{ marginTop: 14, display: 'grid', gridTemplateColumns: 'repeat(2, 1fr)', gap: 8, flex: 1, minHeight: 0 }}>
          {ALL_GAMES.map((g, i) => {
            const s = sel === i;
            return (
              <div key={g.id} onClick={() => g.installed && setSel(i)} style={{
                padding: '12px 14px', cursor: g.installed ? 'pointer' : 'not-allowed',
                border: `2px solid ${s ? RP.red : RP.border}`,
                background: s ? 'rgba(225,6,0,0.08)' : RP.card,
                opacity: g.installed ? 1 : 0.45,
                display: 'flex', alignItems: 'center', gap: 12,
              }}>
                <GameBadge code={g.code} />
                <div style={{ flex: 1, minWidth: 0 }}>
                  <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                    <div style={{ ...t('h2'), fontFamily: FONT.display, fontSize: 14, color: RP.ink, letterSpacing: '0.04em' }}>
                      {g.label.toUpperCase()}
                    </div>
                    {g.branch === 'ac' && (
                      <div style={{ ...t('caption'), fontSize: 8, color: RP.amber, padding: '1px 5px', border: `1px solid ${RP.amber}` }}>★ FULL SETUP</div>
                    )}
                  </div>
                  <div style={{ ...t('bodySm'), fontSize: 11, color: RP.inkDim, marginTop: 2 }}>{g.sub}</div>
                </div>
                {!g.installed && (
                  <div style={{ ...t('caption'), fontSize: 9, color: RP.amber, letterSpacing: '0.08em' }}>NOT INSTALLED</div>
                )}
                {s && <Icon name="check" size={16} color={RP.red} />}
              </div>
            );
          })}
        </div>

        <div style={{ display: 'flex', alignItems: 'center', gap: 10, marginTop: 14 }}>
          <div style={{ ...t('mono'), fontSize: 10, color: RP.gunmetal }}>
            {game.branch === 'ac'
              ? 'AC: NEXT STEP IS MODE + TRANSMISSION + AIDS'
              : 'NON-AC: STARTS SESSION IMMEDIATELY'}
          </div>
          <div style={{ flex: 1 }} />
          <PrimaryCTA
            onClick={() => game.branch === 'ac' ? onPickAc(game) : onPickSimple(game)}
            shortcut="ENTER"
            disabled={!game.installed}
          >
            {game.branch === 'ac' ? 'Continue → Configure' : 'Start session →'}
          </PrimaryCTA>
        </div>
      </div>
    </KioskFrame>
  );
}
