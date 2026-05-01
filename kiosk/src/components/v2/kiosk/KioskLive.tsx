'use client';

// Racing Point V2 Kiosk — Live session (AC HUD + non-AC HUD + operator overlays)
// Source: claude.ai/design handoff bundle (PyiT2ipTf0VdYL8V9vd7-A) — 2026-05-02

import * as React from 'react';
import { RP, FONT, t } from '../tokens';
import { Icon, PrimaryCTA, SecondaryCTA } from '../atomic';
import { KioskFrame } from './KioskFrame';
import { type KioskGame } from './KioskShared';

const RATE_PER_MIN = 100 / 15;
const fmtMoney = (v: number) => `₹${v.toFixed(0)}`;
const fmt = (s: number) =>
  `${String(Math.floor(s / 60)).padStart(2, '0')}:${String(s % 60).padStart(2, '0')}`;

const DEFAULT_GAME: KioskGame = { id: 'ac', code: 'AC', label: 'Assetto Corsa' };

export function KioskLive({
  game = DEFAULT_GAME,
  driverName = 'Uday Singh',
  onSwitchGame,
  onEnd,
}: {
  game?: KioskGame;
  driverName?: string;
  onSwitchGame: () => void;
  onEnd: () => void;
}) {
  const [elapsed, setElapsed] = React.useState(243);
  const [wallet, setWallet] = React.useState(420);
  const [paused, setPaused] = React.useState(false);
  const [topupOpen, setTopupOpen] = React.useState(false);
  const [confirmEnd, setConfirmEnd] = React.useState(false);

  React.useEffect(() => {
    if (paused) return;
    const id = setInterval(() => {
      setElapsed(e => e + 1);
      setWallet(w => Math.max(0, +(w - RATE_PER_MIN / 60).toFixed(2)));
    }, 1000);
    return () => clearInterval(id);
  }, [paused]);

  const minsLeft = wallet > 0 ? wallet / RATE_PER_MIN : 0;
  const lowBalance = wallet < 50;

  const onTopupConfirm = (amount: number) => {
    setWallet(w => +(w + amount).toFixed(2));
    setTopupOpen(false);
  };

  const inner = game.id === 'ac'
    ? <KioskLiveAC elapsed={elapsed} game={game} driverName={driverName} wallet={wallet} minsLeft={minsLeft} lowBalance={lowBalance} />
    : <KioskLiveSimple elapsed={elapsed} game={game} driverName={driverName} wallet={wallet} minsLeft={minsLeft} lowBalance={lowBalance} />;

  return (
    <KioskFrame podId="POD-01" state={paused ? 'idle' : 'live'}>
      <div style={{ height: '100%', display: 'flex', flexDirection: 'column', position: 'relative' }}>
        <div style={{ flex: 1, minHeight: 0 }}>{inner}</div>

        <KioskOperatorStrip
          onPause={() => setPaused(true)}
          onTopup={() => setTopupOpen(true)}
          onSwitchGame={() => { setPaused(true); }}
          onEnd={() => setConfirmEnd(true)}
          paused={paused}
          lowBalance={lowBalance}
        />

        {paused && !topupOpen && !confirmEnd && (
          <KioskPauseOverlay
            elapsed={fmt(elapsed)}
            wallet={wallet}
            game={game}
            onResume={() => setPaused(false)}
            onTopup={() => setTopupOpen(true)}
            onSwitchGame={onSwitchGame}
            onEnd={() => setConfirmEnd(true)}
          />
        )}

        {topupOpen && (
          <KioskTopupOverlay
            wallet={wallet}
            onClose={() => setTopupOpen(false)}
            onConfirm={onTopupConfirm}
          />
        )}

        {confirmEnd && (
          <KioskEndOverlay
            elapsed={fmt(elapsed)}
            wallet={wallet}
            onCancel={() => setConfirmEnd(false)}
            onConfirm={onEnd}
          />
        )}
      </div>
    </KioskFrame>
  );
}

// ── Non-AC HUD ────────────────────────────────────────────────
function KioskLiveSimple({ elapsed, game, driverName, wallet, minsLeft, lowBalance }: {
  elapsed: number;
  game: KioskGame;
  driverName: string;
  wallet: number;
  minsLeft: number;
  lowBalance: boolean;
}) {
  const [tick, setTick] = React.useState(0);
  React.useEffect(() => { const id = setInterval(() => setTick(x => x + 1), 100); return () => clearInterval(id); }, []);
  const phase = (tick % 60) / 60;
  const speed = Math.round(140 + 110 * Math.abs(Math.sin(phase * Math.PI * 2)));

  return (
    <div style={{ height: '100%', display: 'flex', flexDirection: 'column', padding: '24px 40px', background: '#000' }}>
      <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 14 }}>
          <div style={{ ...t('caption'), fontSize: 11, color: RP.green }}>● LIVE · {game.label.toUpperCase()}</div>
          <div style={{ ...t('mono'), fontSize: 12, color: RP.inkDim }}>{driverName.toUpperCase()}</div>
        </div>
        <WalletBadge wallet={wallet} minsLeft={minsLeft} lowBalance={lowBalance} />
      </div>

      <div style={{ marginTop: 18, display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 16 }}>
        <div style={{ padding: '24px 28px', background: RP.card, border: `1px solid ${RP.border}`, borderLeft: `3px solid ${RP.red}` }}>
          <div style={{ ...t('caption'), fontSize: 10, color: RP.gunmetal, letterSpacing: '0.12em' }}>SESSION TIME · COUNTING UP</div>
          <div style={{
            ...t('mono'), fontFamily: FONT.display, fontSize: 88, color: RP.red,
            marginTop: 4, lineHeight: 1, letterSpacing: '0.02em', fontWeight: 800,
          }}>{fmt(elapsed)}</div>
          <div style={{ ...t('mono'), fontSize: 11, color: RP.inkDim, marginTop: 6 }}>STARTED 16:42 · BILLED PER MINUTE</div>
        </div>
        <div style={{
          padding: '24px 28px', background: RP.card, border: `1px solid ${RP.border}`,
          borderLeft: `3px solid ${RP.green}`, display: 'flex', flexDirection: 'column',
        }}>
          <div style={{ ...t('caption'), fontSize: 10, color: RP.gunmetal, letterSpacing: '0.12em' }}>CURRENT SPEED</div>
          <div style={{ display: 'flex', alignItems: 'baseline', gap: 14, marginTop: 4 }}>
            <div style={{
              ...t('mono'), fontFamily: FONT.display, fontSize: 88, color: RP.ink,
              lineHeight: 1, letterSpacing: '-0.02em', fontWeight: 800,
            }}>{speed}</div>
            <div style={{ ...t('mono'), fontSize: 18, color: RP.inkDim, letterSpacing: '0.12em' }}>km/h</div>
          </div>
          <div style={{ height: 4, background: RP.border, marginTop: 8, position: 'relative' }}>
            <div style={{
              position: 'absolute', inset: 0, width: `${Math.min(100, speed / 320 * 100)}%`,
              background: RP.green, transition: 'width 0.1s linear',
            }} />
          </div>
          <div style={{ ...t('mono'), fontSize: 10, color: RP.gunmetal, marginTop: 6, letterSpacing: '0.06em' }}>0 ───────── 320 km/h</div>
        </div>
      </div>

      <div style={{ marginTop: 14, display: 'grid', gridTemplateColumns: 'repeat(4, 1fr)', gap: 10 }}>
        <LiveTile label="LAP" value="4" sub="of ~9 estimate" />
        <LiveTile label="LAST LAP" value="2:01.482" />
        <LiveTile label="BEST LAP" value="2:00.630" hi />
        <LiveTile label="POSITION" value="P1" hi />
      </div>

      <div style={{ flex: 1 }} />
    </div>
  );
}

// ── AC HUD: cockpit cluster + lap list ────────────────────────
interface LapRow {
  n: number;
  time: string;
  s1: string;
  s2: string;
  s3: string;
  valid: boolean;
  delta: string;
  pb?: boolean;
  reason?: string;
}

const LAPS: LapRow[] = [
  { n: 7, time: '1:48.640', s1: '24.1', s2: '38.4', s3: '46.1', valid: true,  delta: '+0.433' },
  { n: 6, time: '1:48.207', s1: '23.9', s2: '38.2', s3: '46.1', valid: true,  delta: 'PB',     pb: true },
  { n: 5, time: '1:50.118', s1: '24.6', s2: '39.0', s3: '46.5', valid: false, delta: '+1.911', reason: 'Track limits T9' },
  { n: 4, time: '1:48.992', s1: '24.0', s2: '38.5', s3: '46.4', valid: true,  delta: '+0.785' },
  { n: 3, time: '1:49.640', s1: '24.2', s2: '38.7', s3: '46.7', valid: true,  delta: '+1.433' },
  { n: 2, time: '1:51.480', s1: '24.8', s2: '39.4', s3: '47.2', valid: false, delta: '+3.273', reason: 'All 4 wheels off' },
  { n: 1, time: '1:53.220', s1: '25.6', s2: '40.1', s3: '47.5', valid: true,  delta: '+5.013' },
];

function KioskLiveAC({ elapsed, game, driverName, wallet, minsLeft, lowBalance }: {
  elapsed: number;
  game: KioskGame;
  driverName: string;
  wallet: number;
  minsLeft: number;
  lowBalance: boolean;
}) {
  const [tick, setTick] = React.useState(0);
  React.useEffect(() => { const id = setInterval(() => setTick(x => x + 1), 80); return () => clearInterval(id); }, []);
  const phase = (tick % 80) / 80;
  const rpmNorm = 0.35 + 0.6 * Math.abs(Math.sin(phase * Math.PI * 2));
  const rpm = Math.round(rpmNorm * 9000);
  const speed = Math.round(80 + rpmNorm * 200);
  const gear = ['1','2','3','4','5','6','7'][Math.min(6, Math.floor(rpmNorm * 7))];
  const throttle = Math.max(0, Math.sin(phase * Math.PI * 2));
  const brake = Math.max(0, -Math.sin(phase * Math.PI * 2));

  const PB = '1:48.207';
  const [showInvalid, setShowInvalid] = React.useState(true);
  const visible = showInvalid ? LAPS : LAPS.filter(l => l.valid);

  return (
    <div style={{ height: '100%', display: 'flex', flexDirection: 'column', padding: '16px 28px', background: '#000' }}>
      <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 14 }}>
          <div style={{ ...t('caption'), fontSize: 10, color: RP.green }}>● LIVE · {game.label.toUpperCase()}</div>
          <div style={{ ...t('mono'), fontSize: 11, color: RP.inkDim }}>{driverName.toUpperCase()}</div>
          <div style={{ ...t('mono'), fontSize: 11, color: RP.inkDim }}>SPA-FRANCORCHAMPS · GT3</div>
        </div>
        <div style={{ display: 'flex', alignItems: 'center', gap: 14 }}>
          <div>
            <div style={{ ...t('caption'), fontSize: 9, color: RP.gunmetal, letterSpacing: '0.12em' }}>ELAPSED</div>
            <div style={{ ...t('mono'), fontSize: 22, color: RP.red, lineHeight: 1, marginTop: 2 }}>{fmt(elapsed)}</div>
          </div>
          <WalletBadge wallet={wallet} minsLeft={minsLeft} lowBalance={lowBalance} />
        </div>
      </div>

      <div style={{ marginTop: 14, display: 'grid', gridTemplateColumns: '1fr 1.1fr 1fr', gap: 18, alignItems: 'center' }}>
        <Dial label="RPM ×1000" value={rpm} max={9000} redline={7800} big={String(Math.floor(rpm / 1000))} />
        <CenterCluster gear={gear} throttle={throttle} brake={brake} />
        <SpeedoNumeric speed={speed} />
      </div>

      <div style={{ marginTop: 14, flex: 1, minHeight: 0, display: 'flex', flexDirection: 'column' }}>
        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: 6 }}>
          <div style={{ ...t('caption'), fontSize: 10, color: RP.gunmetal }}>LAP HISTORY · PB {PB}</div>
          <div style={{ display: 'flex', gap: 6 }}>
            <FilterChip active={showInvalid}  onClick={() => setShowInvalid(true)}  label="ALL LAPS" />
            <FilterChip active={!showInvalid} onClick={() => setShowInvalid(false)} label="VALID ONLY" />
          </div>
        </div>
        <div style={{ flex: 1, minHeight: 0, overflow: 'auto', border: `1px solid ${RP.border}`, background: RP.card }}>
          <div style={{
            display: 'grid', gridTemplateColumns: '50px 1fr 70px 70px 70px 110px 110px',
            ...t('caption'), fontSize: 9, color: RP.gunmetal, padding: '6px 12px',
            borderBottom: `1px solid ${RP.border}`, background: RP.base,
          }}>
            <div>LAP</div><div>TIME</div><div>S1</div><div>S2</div><div>S3</div><div>Δ TO PB</div><div>VALID</div>
          </div>
          {visible.map(l => (
            <div key={l.n} style={{
              display: 'grid', gridTemplateColumns: '50px 1fr 70px 70px 70px 110px 110px',
              padding: '8px 12px', borderBottom: `1px solid ${RP.border}`,
              background: l.pb ? 'rgba(225,6,0,0.06)' : 'transparent',
              opacity: l.valid ? 1 : 0.55,
            }}>
              <div style={{ ...t('mono'), fontSize: 12, color: RP.inkDim }}>{String(l.n).padStart(2, '0')}</div>
              <div style={{
                ...t('mono'), fontSize: 13,
                color: l.pb ? RP.red : (l.valid ? RP.ink : RP.inkDim),
                textDecoration: l.valid ? 'none' : 'line-through',
              }}>
                {l.time}
                {l.reason && <span style={{ ...t('mono'), fontSize: 9, color: RP.amber, marginLeft: 8, textDecoration: 'none' }}>· {l.reason}</span>}
              </div>
              <div style={{ ...t('mono'), fontSize: 11, color: RP.inkDim }}>{l.s1}</div>
              <div style={{ ...t('mono'), fontSize: 11, color: RP.inkDim }}>{l.s2}</div>
              <div style={{ ...t('mono'), fontSize: 11, color: RP.inkDim }}>{l.s3}</div>
              <div style={{
                ...t('mono'), fontSize: 11,
                color: l.pb ? RP.red : (l.delta.startsWith('+') ? RP.amber : RP.green),
              }}>{l.delta}</div>
              <div style={{
                ...t('mono'), fontSize: 10,
                color: l.valid ? RP.green : RP.red, letterSpacing: '0.06em',
              }}>
                {l.valid ? '● VALID' : '✕ INVALID'}
              </div>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}

// ── Dial / CenterCluster / SpeedoNumeric ──────────────────────
function Dial({ label, value, max, redline, big }: {
  label: string;
  value: number;
  max: number;
  redline?: number;
  big: string;
}) {
  const SIZE = 160;
  const R = 64;
  const STROKE = 10;
  const CIRC = 2 * Math.PI * R;
  const ARC_FRAC = 0.75;
  const norm = Math.max(0, Math.min(1, value / max));
  const dash = CIRC * ARC_FRAC * norm;
  const gap = CIRC - dash;
  const redNorm = redline ? redline / max : null;
  const inRed = redNorm !== null && norm >= redNorm;

  return (
    <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'center' }}>
      <div style={{ ...t('caption'), fontSize: 9, color: RP.gunmetal, marginBottom: 6 }}>{label}</div>
      <div style={{ position: 'relative', width: SIZE, height: SIZE }}>
        <svg width={SIZE} height={SIZE} style={{ transform: 'rotate(135deg)' }}>
          <circle cx={SIZE/2} cy={SIZE/2} r={R} fill="none" stroke={RP.border} strokeWidth={STROKE}
            strokeDasharray={`${CIRC * ARC_FRAC} ${CIRC}`} />
          {redNorm !== null && (
            <circle cx={SIZE/2} cy={SIZE/2} r={R} fill="none" stroke={RP.amber} strokeWidth={STROKE}
              strokeDasharray={`${CIRC * ARC_FRAC * (1 - redNorm)} ${CIRC}`}
              strokeDashoffset={-CIRC * ARC_FRAC * redNorm}
              opacity={0.35} />
          )}
          <circle cx={SIZE/2} cy={SIZE/2} r={R} fill="none"
            stroke={inRed ? RP.red : RP.green} strokeWidth={STROKE}
            strokeDasharray={`${dash} ${gap}`}
            style={{ transition: 'stroke-dasharray 0.08s linear' }} />
        </svg>
        <div style={{
          position: 'absolute', inset: 0, display: 'flex', flexDirection: 'column',
          alignItems: 'center', justifyContent: 'center',
        }}>
          <div style={{ ...t('mono'), fontFamily: FONT.display, fontSize: 36, color: inRed ? RP.red : RP.ink, lineHeight: 1 }}>{big}</div>
          <div style={{ ...t('mono'), fontSize: 9, color: RP.gunmetal, marginTop: 4 }}>{value} / {max}</div>
        </div>
      </div>
    </div>
  );
}

function CenterCluster({ gear, throttle, brake }: {
  gear: string;
  throttle: number;
  brake: number;
}) {
  return (
    <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', gap: 8 }}>
      <div style={{ ...t('caption'), fontSize: 9, color: RP.gunmetal }}>GEAR</div>
      <div style={{
        width: 130, height: 130, border: `3px solid ${RP.red}`,
        display: 'flex', alignItems: 'center', justifyContent: 'center',
        background: 'rgba(225,6,0,0.04)',
      }}>
        <div style={{ fontFamily: FONT.display, fontSize: 92, color: RP.ink, lineHeight: 1, fontWeight: 800 }}>{gear}</div>
      </div>
      <div style={{ display: 'flex', gap: 6, width: '100%', maxWidth: 200 }}>
        <PedalBar label="THR" value={throttle} color={RP.green} />
        <PedalBar label="BRK" value={brake}    color={RP.red} />
      </div>
    </div>
  );
}

function PedalBar({ label, value, color }: { label: string; value: number; color: string }) {
  return (
    <div style={{ flex: 1 }}>
      <div style={{ display: 'flex', justifyContent: 'space-between', marginBottom: 2 }}>
        <span style={{ ...t('mono'), fontSize: 9, color: RP.gunmetal }}>{label}</span>
        <span style={{ ...t('mono'), fontSize: 9, color: RP.inkDim }}>{Math.round(value * 100)}%</span>
      </div>
      <div style={{ height: 6, background: RP.border, position: 'relative' }}>
        <div style={{
          position: 'absolute', inset: 0, width: `${value * 100}%`, background: color,
          transition: 'width 0.08s linear',
        }} />
      </div>
    </div>
  );
}

function SpeedoNumeric({ speed }: { speed: number }) {
  return (
    <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', gap: 6 }}>
      <div style={{ ...t('caption'), fontSize: 9, color: RP.gunmetal }}>SPEED</div>
      <div style={{
        width: 160, height: 130,
        border: `1px solid ${RP.border}`, background: RP.card,
        display: 'flex', flexDirection: 'column', alignItems: 'center', justifyContent: 'center',
        position: 'relative',
      }}>
        <div style={{
          fontFamily: FONT.display, fontSize: 72, lineHeight: 1, color: RP.ink, fontWeight: 800,
          letterSpacing: '-0.02em',
        }}>{speed}</div>
        <div style={{ ...t('caption'), fontSize: 11, color: RP.gunmetal, marginTop: 4, letterSpacing: '0.12em' }}>km/h</div>
        <div style={{
          position: 'absolute', left: 8, right: 8, bottom: 6, height: 2, background: RP.border,
        }}>
          <div style={{
            height: '100%', width: `${Math.min(100, speed / 320 * 100)}%`,
            background: RP.red, transition: 'width 0.08s linear',
          }} />
        </div>
      </div>
    </div>
  );
}

function FilterChip({ active, onClick, label }: { active: boolean; onClick: () => void; label: string }) {
  return (
    <div onClick={onClick} style={{
      padding: '4px 10px', cursor: 'pointer',
      border: `1px solid ${active ? RP.red : RP.border}`,
      background: active ? 'rgba(225,6,0,0.08)' : 'transparent',
      ...t('mono'), fontSize: 10, color: active ? RP.red : RP.inkDim, letterSpacing: '0.06em',
    }}>{label}</div>
  );
}

function LiveTile({ label, value, sub, hi }: {
  label: string;
  value: React.ReactNode;
  sub?: React.ReactNode;
  hi?: boolean;
}) {
  return (
    <div style={{
      padding: '14px 16px', background: RP.base, border: `1px solid ${RP.border}`,
      borderLeft: `2px solid ${hi ? RP.red : RP.border}`,
    }}>
      <div style={{ ...t('caption'), fontSize: 9, color: RP.gunmetal }}>{label}</div>
      <div style={{ ...t('mono'), fontSize: 24, color: hi ? RP.red : RP.ink, marginTop: 4 }}>{value}</div>
      {sub && <div style={{ ...t('bodySm'), fontSize: 10, color: RP.inkDim, marginTop: 2 }}>{sub}</div>}
    </div>
  );
}

// ── WalletBadge ───────────────────────────────────────────────
function WalletBadge({ wallet, minsLeft, lowBalance }: {
  wallet: number;
  minsLeft: number;
  lowBalance: boolean;
}) {
  return (
    <div style={{
      padding: '8px 14px', background: lowBalance ? 'rgba(225,6,0,0.1)' : RP.base,
      border: `1px solid ${lowBalance ? RP.red : RP.border}`,
      display: 'flex', alignItems: 'center', gap: 12,
    }}>
      <div>
        <div style={{ ...t('caption'), fontSize: 9, color: RP.gunmetal, letterSpacing: '0.12em' }}>WALLET BALANCE</div>
        <div style={{
          ...t('mono'), fontFamily: FONT.display, fontSize: 22,
          color: lowBalance ? RP.red : RP.ink, lineHeight: 1, marginTop: 2,
        }}>
          {fmtMoney(wallet)}
        </div>
      </div>
      <div style={{ width: 1, height: 32, background: RP.border }} />
      <div>
        <div style={{ ...t('caption'), fontSize: 9, color: RP.gunmetal }}>TIME LEFT</div>
        <div style={{
          ...t('mono'), fontSize: 13,
          color: lowBalance ? RP.amber : RP.inkDim, lineHeight: 1, marginTop: 2,
        }}>
          {minsLeft >= 1 ? `~${Math.floor(minsLeft)} min` : `${Math.round(minsLeft * 60)}s`}
        </div>
      </div>
    </div>
  );
}

// ── Operator strip ────────────────────────────────────────────
function KioskOperatorStrip({ onPause, onTopup, onSwitchGame, onEnd, paused, lowBalance }: {
  onPause: () => void;
  onTopup: () => void;
  onSwitchGame: () => void;
  onEnd: () => void;
  paused: boolean;
  lowBalance: boolean;
}) {
  return (
    <div style={{
      flexShrink: 0, height: 56, background: '#0a0a0a',
      borderTop: `1px solid ${lowBalance ? RP.red : RP.border}`,
      display: 'flex', alignItems: 'stretch', padding: '0 12px', gap: 1,
    }}>
      <div style={{
        display: 'flex', alignItems: 'center', paddingRight: 16,
        ...t('caption'), fontSize: 9, color: RP.gunmetal, letterSpacing: '0.12em',
      }}>
        <span style={{ width: 6, height: 6, borderRadius: '50%', background: RP.amber, marginRight: 8 }} />
        STAFF CONSOLE
      </div>
      <OpAction icon="pulse"   label={paused ? 'Paused' : 'Pause'} onClick={onPause} accent={paused ? RP.amber : RP.inkDim} disabled={paused} />
      <OpAction icon="plus"    label="Top up wallet" onClick={onTopup} accent={lowBalance ? RP.red : RP.green} pulse={lowBalance} />
      <OpAction icon="refresh" label="Switch game"   onClick={onSwitchGame} accent={RP.inkDim} />
      <div style={{ flex: 1 }} />
      <OpAction icon="x"       label="End session"   onClick={onEnd} accent={RP.red} />
    </div>
  );
}

function OpAction({ icon, label, onClick, accent, disabled, pulse }: {
  icon: string;
  label: string;
  onClick: () => void;
  accent: string;
  disabled?: boolean;
  pulse?: boolean;
}) {
  return (
    <button
      onClick={!disabled ? onClick : undefined}
      disabled={disabled}
      style={{
        background: 'transparent', border: 'none',
        borderLeft: `1px solid ${RP.border}`, borderRight: `1px solid ${RP.border}`,
        padding: '0 18px', cursor: disabled ? 'not-allowed' : 'pointer',
        display: 'flex', alignItems: 'center', gap: 8, color: accent,
        ...t('mono'), fontSize: 11, letterSpacing: '0.06em', fontWeight: 600,
        opacity: disabled ? 0.5 : 1,
        animation: pulse ? 'rp-pulse 1.5s ease-in-out infinite' : 'none',
      }}
    >
      <Icon name={icon} size={14} color={accent} />
      <span>{label.toUpperCase()}</span>
    </button>
  );
}

// ── Pause overlay ─────────────────────────────────────────────
function KioskPauseOverlay({ elapsed, wallet, game, onResume, onTopup, onSwitchGame, onEnd }: {
  elapsed: string;
  wallet: number;
  game: KioskGame;
  onResume: () => void;
  onTopup: () => void;
  onSwitchGame: () => void;
  onEnd: () => void;
}) {
  return (
    <Overlay>
      <div style={{
        padding: '32px 40px', background: RP.card, border: `1px solid ${RP.border}`,
        borderTop: `3px solid ${RP.amber}`, minWidth: 520,
      }}>
        <div style={{ ...t('caption'), fontSize: 10, color: RP.amber, letterSpacing: '0.12em' }}>● SESSION PAUSED · TIMER STOPPED</div>
        <div style={{ ...t('displayM'), fontFamily: FONT.display, fontSize: 32, color: RP.ink, marginTop: 8, lineHeight: 1.1 }}>Take a break</div>
        <div style={{ ...t('body'), fontSize: 13, color: RP.inkDim, marginTop: 6 }}>Wallet won&apos;t burn while paused. Resume any time.</div>

        <div style={{
          display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 8, marginTop: 18,
          padding: '14px 16px', background: RP.base, border: `1px solid ${RP.border}`,
        }}>
          <div>
            <div style={{ ...t('caption'), fontSize: 9, color: RP.gunmetal }}>ELAPSED</div>
            <div style={{ ...t('mono'), fontSize: 18, color: RP.ink, marginTop: 2 }}>{elapsed}</div>
          </div>
          <div>
            <div style={{ ...t('caption'), fontSize: 9, color: RP.gunmetal }}>WALLET</div>
            <div style={{ ...t('mono'), fontSize: 18, color: RP.ink, marginTop: 2 }}>{fmtMoney(wallet)}</div>
          </div>
        </div>

        <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 8, marginTop: 14 }}>
          <PauseAction icon="play"    label="Resume session" desc="Continue racing." onClick={onResume} accent={RP.green} primary />
          <PauseAction icon="plus"    label="Top up wallet"  desc="Add more credit." onClick={onTopup} accent={RP.green} />
          <PauseAction icon="refresh" label="Switch game"    desc={`Save current ${game.label} progress, pick a new title.`} onClick={onSwitchGame} accent={RP.amber} />
          <PauseAction icon="x"       label="End & invoice"  desc="Stop session now." onClick={onEnd} accent={RP.red} />
        </div>
      </div>
    </Overlay>
  );
}

function PauseAction({ icon, label, desc, onClick, accent, primary }: {
  icon: string;
  label: string;
  desc: string;
  onClick: () => void;
  accent: string;
  primary?: boolean;
}) {
  return (
    <button onClick={onClick} style={{
      background: primary ? 'rgba(45,209,116,0.06)' : RP.card,
      border: `1px solid ${primary ? accent : RP.border}`,
      borderLeft: `3px solid ${accent}`,
      padding: '14px 16px', cursor: 'pointer', textAlign: 'left',
      display: 'flex', gap: 12, alignItems: 'flex-start', fontFamily: 'inherit',
    }}>
      <Icon name={icon} size={16} color={accent} />
      <div>
        <div style={{ ...t('h2'), fontSize: 13, color: RP.ink }}>{label}</div>
        <div style={{ ...t('bodySm'), fontSize: 11, color: RP.inkDim, marginTop: 2, lineHeight: 1.3 }}>{desc}</div>
      </div>
    </button>
  );
}

// ── Top-up overlay ────────────────────────────────────────────
function KioskTopupOverlay({ wallet, onClose, onConfirm }: {
  wallet: number;
  onClose: () => void;
  onConfirm: (amount: number) => void;
}) {
  const [amount, setAmount] = React.useState(200);
  const [method, setMethod] = React.useState<'cash' | 'upi' | 'card'>('upi');
  const [stage, setStage] = React.useState<'pick' | 'processing' | 'done'>('pick');

  const PRESETS = [100, 200, 500, 1000];
  const METHODS = [
    { id: 'cash' as const, label: 'Cash',  desc: 'Hand cash to staff', icon: 'wallet' },
    { id: 'upi'  as const, label: 'UPI',   desc: 'PhonePe / GPay / Paytm', icon: 'phone' },
    { id: 'card' as const, label: 'Card',  desc: 'Tap or swipe on POS', icon: 'card' },
  ];

  const confirm = () => {
    setStage('processing');
    setTimeout(() => { setStage('done'); }, method === 'cash' ? 600 : 1400);
    setTimeout(() => { onConfirm(amount); }, method === 'cash' ? 1000 : 2000);
  };

  return (
    <Overlay>
      <div style={{
        padding: '28px 36px', background: RP.card, border: `1px solid ${RP.border}`,
        borderTop: `3px solid ${RP.green}`, minWidth: 560, maxWidth: 600,
      }}>
        {stage === 'pick' && (
          <>
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start' }}>
              <div>
                <div style={{ ...t('caption'), fontSize: 10, color: RP.green, letterSpacing: '0.12em' }}>● TOP UP WALLET</div>
                <div style={{ ...t('displayM'), fontFamily: FONT.display, fontSize: 28, color: RP.ink, marginTop: 6 }}>Add credit</div>
                <div style={{ ...t('mono'), fontSize: 11, color: RP.inkDim, marginTop: 4 }}>CURRENT BALANCE · {fmtMoney(wallet)}</div>
              </div>
              <button onClick={onClose} style={{
                background: 'transparent', border: `1px solid ${RP.border}`, color: RP.inkDim,
                padding: '6px 12px', cursor: 'pointer',
                ...t('mono'), fontSize: 10, letterSpacing: '0.08em',
              }}>CANCEL ESC</button>
            </div>

            <div style={{ marginTop: 18 }}>
              <div style={{ ...t('caption'), fontSize: 10, color: RP.gunmetal }}>AMOUNT</div>
              <div style={{ display: 'grid', gridTemplateColumns: 'repeat(4, 1fr)', gap: 6, marginTop: 6 }}>
                {PRESETS.map(p => (
                  <button key={p} onClick={() => setAmount(p)} style={{
                    padding: '14px 8px', cursor: 'pointer', fontFamily: 'inherit',
                    background: amount === p ? 'rgba(225,6,0,0.08)' : RP.base,
                    border: `1px solid ${amount === p ? RP.red : RP.border}`,
                    color: amount === p ? RP.red : RP.ink,
                    ...t('mono'), fontSize: 18, fontWeight: 700,
                  }}>{fmtMoney(p)}</button>
                ))}
              </div>
              <div style={{ ...t('mono'), fontSize: 10, color: RP.gunmetal, marginTop: 6 }}>= ~{Math.round(amount / RATE_PER_MIN)} MIN OF DRIVING</div>
            </div>

            <div style={{ marginTop: 16 }}>
              <div style={{ ...t('caption'), fontSize: 10, color: RP.gunmetal }}>PAYMENT METHOD</div>
              <div style={{ display: 'grid', gridTemplateColumns: 'repeat(3, 1fr)', gap: 6, marginTop: 6 }}>
                {METHODS.map(m => (
                  <button key={m.id} onClick={() => setMethod(m.id)} style={{
                    padding: '14px 12px', cursor: 'pointer', fontFamily: 'inherit', textAlign: 'left',
                    background: method === m.id ? 'rgba(45,209,116,0.06)' : RP.base,
                    border: `1px solid ${method === m.id ? RP.green : RP.border}`,
                    borderLeft: `3px solid ${method === m.id ? RP.green : RP.border}`,
                    display: 'flex', flexDirection: 'column', gap: 4,
                  }}>
                    <div style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
                      <Icon name={m.icon} size={14} color={method === m.id ? RP.green : RP.inkDim} />
                      <span style={{ ...t('h2'), fontSize: 13, color: RP.ink }}>{m.label}</span>
                    </div>
                    <span style={{ ...t('bodySm'), fontSize: 10, color: RP.inkDim }}>{m.desc}</span>
                  </button>
                ))}
              </div>
            </div>

            <div style={{
              marginTop: 18, padding: '14px 16px', background: RP.base, border: `1px solid ${RP.border}`,
              display: 'flex', justifyContent: 'space-between', alignItems: 'center',
            }}>
              <div>
                <div style={{ ...t('caption'), fontSize: 9, color: RP.gunmetal }}>NEW BALANCE AFTER TOP-UP</div>
                <div style={{
                  ...t('mono'), fontFamily: FONT.display, fontSize: 24, color: RP.green, marginTop: 2,
                }}>{fmtMoney(wallet + amount)}</div>
              </div>
              <PrimaryCTA onClick={confirm} shortcut="ENTER">Charge {fmtMoney(amount)} →</PrimaryCTA>
            </div>
          </>
        )}

        {stage === 'processing' && (
          <div style={{ padding: '32px 0', textAlign: 'center' }}>
            <div style={{ ...t('caption'), fontSize: 10, color: RP.amber, letterSpacing: '0.12em' }}>● PROCESSING · {method.toUpperCase()}</div>
            <div style={{ ...t('displayM'), fontFamily: FONT.display, fontSize: 28, color: RP.ink, marginTop: 10 }}>
              {method === 'cash' ? 'Take cash from customer' : method === 'upi' ? 'Customer scanning QR' : 'Tap card on terminal'}
            </div>
            <div style={{ marginTop: 18, height: 4, background: RP.border, position: 'relative', overflow: 'hidden' }}>
              <div style={{
                position: 'absolute', height: '100%', width: '40%',
                background: RP.green, animation: 'rp-slide 1.4s linear infinite',
              }} />
            </div>
            <div style={{ ...t('mono'), fontSize: 11, color: RP.inkDim, marginTop: 14 }}>{fmtMoney(amount)} · {method.toUpperCase()}</div>
          </div>
        )}

        {stage === 'done' && (
          <div style={{ padding: '32px 0', textAlign: 'center' }}>
            <Icon name="check" size={42} color={RP.green} />
            <div style={{ ...t('displayM'), fontFamily: FONT.display, fontSize: 28, color: RP.green, marginTop: 10 }}>Topped up</div>
            <div style={{ ...t('body'), fontSize: 13, color: RP.inkDim, marginTop: 6 }}>+{fmtMoney(amount)} added · resuming session</div>
          </div>
        )}
      </div>
    </Overlay>
  );
}

// ── End-session confirm overlay ───────────────────────────────
function KioskEndOverlay({ elapsed, wallet, onCancel, onConfirm }: {
  elapsed: string;
  wallet: number;
  onCancel: () => void;
  onConfirm: () => void;
}) {
  return (
    <Overlay>
      <div style={{
        padding: '28px 36px', background: RP.card, border: `1px solid ${RP.red}`,
        borderTop: `3px solid ${RP.red}`, minWidth: 480,
      }}>
        <div style={{ ...t('caption'), fontSize: 10, color: RP.red, letterSpacing: '0.12em' }}>● END SESSION · IRREVERSIBLE</div>
        <div style={{ ...t('displayM'), fontFamily: FONT.display, fontSize: 28, color: RP.ink, marginTop: 8 }}>End the session now?</div>
        <div style={{ ...t('body'), fontSize: 13, color: RP.inkDim, marginTop: 6 }}>Driver will see results, lap stats, and a QR to view the lap on their phone.</div>

        <div style={{
          display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 8, marginTop: 16,
          padding: '14px 16px', background: RP.base, border: `1px solid ${RP.border}`,
        }}>
          <div>
            <div style={{ ...t('caption'), fontSize: 9, color: RP.gunmetal }}>SESSION TIME</div>
            <div style={{ ...t('mono'), fontSize: 18, color: RP.ink, marginTop: 2 }}>{elapsed}</div>
          </div>
          <div>
            <div style={{ ...t('caption'), fontSize: 9, color: RP.gunmetal }}>WALLET REMAINING</div>
            <div style={{ ...t('mono'), fontSize: 18, color: RP.ink, marginTop: 2 }}>{fmtMoney(wallet)}</div>
          </div>
        </div>

        <div style={{ display: 'flex', justifyContent: 'flex-end', gap: 8, marginTop: 18 }}>
          <SecondaryCTA onClick={onCancel} shortcut="ESC">Cancel</SecondaryCTA>
          <button onClick={onConfirm} style={{
            background: RP.red, color: RP.ink, border: 'none', padding: '14px 24px', cursor: 'pointer',
            ...t('h2'), fontFamily: FONT.display, fontSize: 14, letterSpacing: '0.08em',
          }}>END SESSION →</button>
        </div>
      </div>
    </Overlay>
  );
}

function Overlay({ children }: { children: React.ReactNode }) {
  return (
    <div style={{
      position: 'absolute', inset: 0, background: 'rgba(0,0,0,0.78)',
      display: 'flex', alignItems: 'center', justifyContent: 'center', zIndex: 50,
      backdropFilter: 'blur(2px)',
    }}>
      {children}
    </div>
  );
}
