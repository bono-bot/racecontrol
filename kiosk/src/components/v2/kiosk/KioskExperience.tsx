'use client';

// Racing Point V2 Kiosk — Pick session (per-game catalogue)
// Source: claude.ai/design handoff bundle (PyiT2ipTf0VdYL8V9vd7-A) — 2026-05-02

import * as React from 'react';
import { RP, FONT, t } from '../tokens';
import { Icon, PrimaryCTA, SecondaryCTA } from '../atomic';
import { KioskFrame } from './KioskFrame';
import { GameBadge, type KioskGame } from './KioskShared';

interface SessionEntry {
  cat: string;
  title: string;
  sub: string;
  dur: string;
  featured?: boolean;
}

const ALL_SESSIONS: Record<string, SessionEntry[]> = {
  ac: [
    { cat: 'Practice', title: 'Open Practice · 10 Min',     sub: 'Pitlane start, free roam',          dur: '10 MIN' },
    { cat: 'Race',     title: 'Quick Race · 5 Laps',        sub: 'Grid start, AI opponents',          dur: '~12 MIN' },
    { cat: 'Featured', title: 'Spa Hotlap Challenge',       sub: 'Single fast lap leaderboard',       dur: '5 MIN', featured: true },
    { cat: 'Race',     title: 'Sprint Race · 10 Laps',      sub: 'Standing start, GT3 grid',          dur: '~22 MIN' },
  ],
  iracing: [
    { cat: 'Race',     title: 'Rookie Race · 10 Laps',      sub: 'Joins next official rookie series', dur: '~22 MIN' },
    { cat: 'Practice', title: 'Open Practice · 10 Min',     sub: 'Any track, no AI',                  dur: '10 MIN' },
    { cat: 'Featured', title: '2021 F1 Practice Stint',     sub: 'No pit stops, open lap practice',   dur: '15 MIN', featured: true },
  ],
  lmu:    [{ cat: 'Race', title: 'Hypercar Sprint',          sub: 'Le Mans circuit',                   dur: '15 MIN' }],
  f125:   [
    { cat: 'Featured', title: 'Time Trial · Bahrain',       sub: 'Single fast lap',                   dur: '5 MIN', featured: true },
    { cat: 'Race',     title: 'Grand Prix · Short',          sub: 'AI grid, 5 laps',                   dur: '~15 MIN' },
  ],
  forza:  [{ cat: 'Free roam', title: 'Open Mexico',         sub: 'Drive anywhere, arcade physics',    dur: '15 MIN' }],
  acevo:  [{ cat: 'Practice', title: 'Realism Drive',        sub: 'Hyper-realistic test',              dur: '10 MIN' }],
};

const DEFAULT_GAME: KioskGame = { id: 'ac', code: 'AC', label: 'Assetto Corsa' };

export function KioskExperience({
  driverName = 'Uday Singh',
  game = DEFAULT_GAME,
  mode = 'sp',
  onContinue,
  onBack,
}: {
  driverName?: string;
  game?: KioskGame;
  mode?: 'sp' | 'mp';
  onContinue: () => void;
  onBack: () => void;
}) {
  const [sel, setSel] = React.useState(0);
  const visible = ALL_SESSIONS[game.id] || ALL_SESSIONS.ac;

  return (
    <KioskFrame podId="POD-01" state="idle">
      <div style={{ height: '100%', display: 'flex', flexDirection: 'column', padding: '24px 40px', overflow: 'auto' }}>
        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
          <div style={{ ...t('caption'), fontSize: 10, color: RP.gunmetal }}>FINAL STEP · STAFF</div>
          <SecondaryCTA onClick={onBack} shortcut="ESC">Back</SecondaryCTA>
        </div>
        <div style={{ ...t('displayM'), fontFamily: FONT.display, fontSize: 32, color: RP.ink, marginTop: 8 }}>Pick the session</div>
        <div style={{ ...t('body'), fontSize: 13, color: RP.inkDim, marginTop: 4 }}>
          Driver: <span style={{ color: RP.ink }}>{driverName}</span>{mode === 'mp' && ' · Multiplayer (TUESDAY-LEAGUE)'}
        </div>

        <div style={{
          marginTop: 16, padding: '10px 14px',
          background: RP.base, border: `1px solid ${RP.border}`, borderLeft: `2px solid ${RP.red}`,
          display: 'flex', alignItems: 'center', gap: 12,
        }}>
          <GameBadge code={game.code} small />
          <div style={{ flex: 1 }}>
            <div style={{ ...t('caption'), fontSize: 9, color: RP.gunmetal }}>SHOWING SESSIONS FOR</div>
            <div style={{ ...t('h2'), fontSize: 13, color: RP.ink, marginTop: 2 }}>{game.label}</div>
          </div>
          <div style={{ ...t('mono'), fontSize: 10, color: RP.inkDim }}>GO BACK TO CHANGE GAME</div>
        </div>

        <div style={{ marginTop: 14, display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 8, flex: 1, minHeight: 0 }}>
          {visible.map((e, i) => {
            const s = sel === i;
            return (
              <div key={i} onClick={() => setSel(i)} style={{
                padding: '14px 16px', border: `2px solid ${s ? RP.red : RP.border}`,
                background: s ? 'rgba(225,6,0,0.08)' : RP.card,
                cursor: 'pointer', display: 'flex', alignItems: 'center', gap: 12,
              }}>
                <div style={{ flex: 1, minWidth: 0 }}>
                  <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                    <div style={{ ...t('caption'), fontSize: 9, color: RP.inkDim }}>{e.cat.toUpperCase()}</div>
                    {e.featured && <div style={{ ...t('caption'), fontSize: 8, color: RP.amber, padding: '1px 5px', border: `1px solid ${RP.amber}` }}>★ FEATURED</div>}
                  </div>
                  <div style={{ ...t('h2'), fontFamily: FONT.display, fontSize: 16, color: RP.ink, marginTop: 2 }}>{e.title}</div>
                  <div style={{ ...t('bodySm'), fontSize: 11, color: RP.inkDim, marginTop: 2 }}>{e.sub}</div>
                </div>
                <div style={{ ...t('mono'), fontSize: 12, color: s ? RP.red : RP.inkDim, flexShrink: 0 }}>{e.dur}</div>
                <div style={{
                  width: 18, height: 18, borderRadius: '50%',
                  border: `2px solid ${s ? RP.red : RP.borderHi}`,
                  background: s ? RP.red : 'transparent', flexShrink: 0,
                  display: 'flex', alignItems: 'center', justifyContent: 'center',
                }}>{s && <Icon name="check" size={10} color={RP.ink} />}</div>
              </div>
            );
          })}
        </div>

        <div style={{ display: 'flex', alignItems: 'center', gap: 10, marginTop: 16 }}>
          <div style={{ ...t('mono'), fontSize: 10, color: RP.gunmetal }}>↑↓ NAVIGATE · ENTER TO START</div>
          <div style={{ flex: 1 }} />
          <PrimaryCTA onClick={onContinue} shortcut="ENTER">Start session →</PrimaryCTA>
        </div>
      </div>
    </KioskFrame>
  );
}
