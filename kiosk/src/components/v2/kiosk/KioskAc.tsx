'use client';

// Racing Point V2 Kiosk — AC subtree: Mode pick (SP/MP) + Lobby join + SP setup
// Source: claude.ai/design handoff bundle (PyiT2ipTf0VdYL8V9vd7-A) — 2026-05-02

import * as React from 'react';
import { RP, FONT, t } from '../tokens';
import { Icon, PrimaryCTA, SecondaryCTA } from '../atomic';
import { KioskFrame } from './KioskFrame';

// ── KioskAcMode (SP/MP picker) ────────────────────────────────
export function KioskAcMode({ onSinglePlayer, onMultiplayer, onBack }: {
  onSinglePlayer: () => void;
  onMultiplayer: () => void;
  onBack: () => void;
}) {
  const [sel, setSel] = React.useState<'sp' | 'mp'>('sp');
  return (
    <KioskFrame podId="POD-01" state="idle">
      <div style={{ height: '100%', display: 'flex', flexDirection: 'column', padding: '24px 40px' }}>
        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
          <div style={{ ...t('caption'), fontSize: 10, color: RP.gunmetal }}>STEP 3 OF 5 · ASSETTO CORSA</div>
          <SecondaryCTA onClick={onBack} shortcut="ESC">Back</SecondaryCTA>
        </div>
        <div style={{ ...t('displayM'), fontFamily: FONT.display, fontSize: 32, color: RP.ink, marginTop: 8 }}>Single-player or multiplayer?</div>
        <div style={{ ...t('body'), fontSize: 13, color: RP.inkDim, marginTop: 4 }}>
          AC is the only title that supports head-to-head between pods.
        </div>

        <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 16, marginTop: 28, flex: 1, minHeight: 0 }}>
          <ModeCard
            sel={sel === 'sp'} onClick={() => setSel('sp')}
            tag="SOLO" title="Single-player"
            desc="Drive alone. Pick transmission, aids and circuit. Best for first-timers and time trials."
            bullets={['No lobby setup', 'Full transmission + aid choice', 'Personal lap leaderboard']}
          />
          <ModeCard
            sel={sel === 'mp'} onClick={() => setSel('mp')} accent={RP.amber}
            tag="HEAD-TO-HEAD" title="Multiplayer"
            desc="Join a named lobby that staff has set up. Race other pods live."
            bullets={['Staff hosts the lobby externally', 'Pod joins by lobby name', 'Shared session settings']}
          />
        </div>

        <div style={{ display: 'flex', alignItems: 'center', gap: 10, marginTop: 20 }}>
          <div style={{ ...t('mono'), fontSize: 10, color: RP.gunmetal }}>← → SELECT · ENTER TO CONTINUE</div>
          <div style={{ flex: 1 }} />
          <PrimaryCTA
            onClick={() => sel === 'sp' ? onSinglePlayer() : onMultiplayer()}
            shortcut="ENTER"
          >Continue →</PrimaryCTA>
        </div>
      </div>
    </KioskFrame>
  );
}

function ModeCard({ sel, onClick, tag, title, desc, bullets, accent = RP.red }: {
  sel: boolean;
  onClick: () => void;
  tag: string;
  title: string;
  desc: string;
  bullets: string[];
  accent?: string;
}) {
  return (
    <div onClick={onClick} style={{
      padding: '24px 28px', cursor: 'pointer',
      border: `2px solid ${sel ? accent : RP.border}`,
      background: sel ? (accent === RP.amber ? 'rgba(255,176,0,0.08)' : 'rgba(225,6,0,0.06)') : RP.card,
      display: 'flex', flexDirection: 'column',
    }}>
      <div style={{ ...t('caption'), fontSize: 10, color: accent, letterSpacing: '0.12em' }}>● {tag}</div>
      <div style={{ ...t('displayM'), fontFamily: FONT.display, fontSize: 28, color: RP.ink, marginTop: 8 }}>{title}</div>
      <div style={{ ...t('body'), fontSize: 13, color: RP.inkDim, marginTop: 10, lineHeight: 1.5 }}>{desc}</div>
      <div style={{ marginTop: 18, display: 'flex', flexDirection: 'column', gap: 6 }}>
        {bullets.map((b, i) => (
          <div key={i} style={{ ...t('bodySm'), fontSize: 11, color: RP.inkDim, display: 'flex', gap: 8 }}>
            <span style={{ color: accent }}>·</span> {b}
          </div>
        ))}
      </div>
    </div>
  );
}

// ── KioskAcLobby (multiplayer lobby join) ────────────────────
interface Lobby {
  name: string;
  host: string;
  pods: string[];
  track: string;
  car: string;
  open: boolean;
}

const LOBBIES: Lobby[] = [
  { name: 'TUESDAY-LEAGUE',   host: 'Staff (Aman)',     pods: ['POD-02','POD-03','POD-04'],            track: 'Spa-Francorchamps', car: 'GT3',    open: true },
  { name: 'CORPORATE-NIGHT',  host: 'Staff (Reception)', pods: ['POD-05'],                             track: 'Monza',             car: 'GT3',    open: true },
  { name: 'PRACTICE-OPEN',    host: 'Staff (Aman)',     pods: [],                                      track: 'Mugello',           car: 'F.Open', open: true },
  { name: 'LOCKED-FINAL',     host: 'Staff (Reception)', pods: ['POD-02','POD-03','POD-04','POD-05'],   track: 'Imola',             car: 'GT3',    open: false },
];

export function KioskAcLobby({ onJoin, onBack }: {
  onJoin: () => void;
  onBack: () => void;
}) {
  const [sel, setSel] = React.useState(0);
  const [ai, setAi] = React.useState(false);
  const [aiDiff, setAiDiff] = React.useState('medium');

  return (
    <KioskFrame podId="POD-01" state="idle">
      <div style={{ height: '100%', display: 'flex', flexDirection: 'column', padding: '20px 36px', overflow: 'auto' }}>
        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
          <div style={{ ...t('caption'), fontSize: 10, color: RP.amber }}>● STEP 4 OF 5 · MULTIPLAYER LOBBY</div>
          <SecondaryCTA onClick={onBack} shortcut="ESC">Back</SecondaryCTA>
        </div>
        <div style={{ ...t('displayM'), fontFamily: FONT.display, fontSize: 28, color: RP.ink, marginTop: 6 }}>Join a lobby</div>
        <div style={{ ...t('body'), fontSize: 12, color: RP.inkDim, marginTop: 2 }}>
          Lobbies are created by staff externally. Pick the room name they told the driver.
        </div>

        <div style={{ marginTop: 14, display: 'flex', flexDirection: 'column', gap: 8, flex: 1, minHeight: 0 }}>
          {LOBBIES.map((l, i) => {
            const s = sel === i;
            const dim = !l.open;
            return (
              <div key={i} onClick={() => !dim && setSel(i)} style={{
                padding: '14px 18px', cursor: dim ? 'not-allowed' : 'pointer',
                border: `2px solid ${s ? RP.red : RP.border}`,
                background: s ? 'rgba(225,6,0,0.08)' : RP.card,
                opacity: dim ? 0.45 : 1,
                display: 'grid', gridTemplateColumns: '180px 1fr 140px 100px auto', gap: 16, alignItems: 'center',
              }}>
                <div>
                  <div style={{ ...t('caption'), fontSize: 9, color: RP.gunmetal }}>LOBBY NAME</div>
                  <div style={{ ...t('mono'), fontSize: 14, color: RP.ink, marginTop: 2, letterSpacing: '0.06em' }}>{l.name}</div>
                </div>
                <div>
                  <div style={{ ...t('caption'), fontSize: 9, color: RP.gunmetal }}>TRACK · CAR CLASS</div>
                  <div style={{ ...t('h2'), fontSize: 13, color: RP.ink, marginTop: 2 }}>{l.track} · {l.car}</div>
                </div>
                <div>
                  <div style={{ ...t('caption'), fontSize: 9, color: RP.gunmetal }}>HOST</div>
                  <div style={{ ...t('bodySm'), fontSize: 11, color: RP.inkDim, marginTop: 2 }}>{l.host}</div>
                </div>
                <div>
                  <div style={{ ...t('caption'), fontSize: 9, color: RP.gunmetal }}>PODS IN</div>
                  <div style={{ ...t('mono'), fontSize: 12, color: l.pods.length > 0 ? RP.green : RP.inkDim, marginTop: 2 }}>
                    {l.pods.length} / 8
                  </div>
                </div>
                {dim ? (
                  <div style={{ ...t('caption'), fontSize: 9, color: RP.amber }}>● LOCKED</div>
                ) : s ? (
                  <Icon name="check" size={16} color={RP.red} />
                ) : (
                  <div style={{ ...t('mono'), fontSize: 10, color: RP.gunmetal }}>OPEN</div>
                )}
              </div>
            );
          })}
        </div>

        <div style={{ marginTop: 14, display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 12 }}>
          <div style={{ padding: '12px 14px', background: RP.base, border: `1px dashed ${RP.border}` }}>
            <div style={{ ...t('caption'), fontSize: 9, color: RP.gunmetal, marginBottom: 4 }}>LOBBY-CONTROLLED</div>
            <div style={{ ...t('mono'), fontSize: 10, color: RP.inkDim, lineHeight: 1.5 }}>
              ● TRANSMISSION + AIDS ARE SET BY THE LOBBY HOST<br />
              ● DRIVER INHERITS LOBBY CONFIG ON JOIN
            </div>
          </div>
          <div style={{ padding: '12px 14px', background: RP.card, border: `1px solid ${ai ? RP.red : RP.border}` }}>
            <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: 8 }}>
              <div style={{ ...t('caption'), fontSize: 9, color: RP.gunmetal }}>FILL FIELD WITH AI</div>
              <div onClick={() => setAi(v => !v)} style={{
                display: 'inline-flex', alignItems: 'center', gap: 6, cursor: 'pointer',
              }}>
                <div style={{
                  width: 22, height: 12, borderRadius: 8, position: 'relative',
                  background: ai ? RP.red : RP.gunmetal, transition: 'background 0.15s',
                }}>
                  <div style={{
                    position: 'absolute', top: 1, left: ai ? 11 : 1, width: 10, height: 10,
                    borderRadius: '50%', background: '#fff', transition: 'left 0.15s',
                  }} />
                </div>
                <div style={{ ...t('mono'), fontSize: 10, color: ai ? RP.red : RP.inkDim }}>{ai ? 'ON' : 'OFF'}</div>
              </div>
            </div>
            <div style={{
              display: 'grid', gridTemplateColumns: 'repeat(4, 1fr)', gap: 4,
              opacity: ai ? 1 : 0.4, pointerEvents: ai ? 'auto' : 'none',
            }}>
              {[
                { id: 'easy',   label: 'EASY' },
                { id: 'medium', label: 'MED' },
                { id: 'hard',   label: 'HARD' },
                { id: 'pro',    label: 'PRO' },
              ].map(d => {
                const sd = aiDiff === d.id;
                return (
                  <div key={d.id} onClick={() => setAiDiff(d.id)} style={{
                    padding: '6px 4px', textAlign: 'center', cursor: 'pointer',
                    border: `1px solid ${sd ? RP.red : RP.border}`,
                    background: sd ? 'rgba(225,6,0,0.08)' : 'transparent',
                    ...t('mono'), fontSize: 10, color: sd ? RP.red : RP.ink,
                  }}>{d.label}</div>
                );
              })}
            </div>
          </div>
        </div>

        <div style={{ display: 'flex', alignItems: 'center', gap: 10, marginTop: 12 }}>
          <div style={{ ...t('mono'), fontSize: 10, color: RP.gunmetal }}>↑↓ NAVIGATE · ENTER TO JOIN</div>
          <div style={{ flex: 1 }} />
          <PrimaryCTA onClick={onJoin} shortcut="ENTER">Join lobby →</PrimaryCTA>
        </div>
      </div>
    </KioskFrame>
  );
}

// ── KioskAcSetup (single-player setup: tx + aids + AI) ────────
type AidLevel = 'easy' | 'hard' | 'ultra' | 'extreme';

export function KioskAcSetup({ driverName = 'Uday Singh', onContinue, onBack }: {
  driverName?: string;
  onContinue: () => void;
  onBack: () => void;
}) {
  const [tx, setTx] = React.useState<'Manual' | 'Auto'>('Auto');
  const [aid, setAid] = React.useState<AidLevel>('hard');
  const [ai, setAi] = React.useState(false);
  const [aiDiff, setAiDiff] = React.useState('medium');

  return (
    <KioskFrame podId="POD-01" state="idle">
      <div style={{ height: '100%', display: 'flex', flexDirection: 'column', padding: '24px 40px', overflow: 'auto' }}>
        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
          <div style={{ ...t('caption'), fontSize: 10, color: RP.gunmetal }}>STEP 4 OF 5 · ASSETTO CORSA · SOLO</div>
          <SecondaryCTA onClick={onBack} shortcut="ESC">Back</SecondaryCTA>
        </div>
        <div style={{ ...t('displayM'), fontFamily: FONT.display, fontSize: 28, color: RP.ink, marginTop: 6 }}>Configure the car</div>
        <div style={{ ...t('body'), fontSize: 12, color: RP.inkDim, marginTop: 2 }}>
          Single-player setup. Aids and transmission affect every session.
        </div>

        <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 24, marginTop: 18, flex: 1, minHeight: 0 }}>
          <div>
            <div style={{ ...t('caption'), fontSize: 10, color: RP.gunmetal, marginBottom: 6 }}>TRANSMISSION</div>
            <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 8 }}>
              {[{ id: 'Manual' as const, d: 'H-pattern + clutch' }, { id: 'Auto' as const, d: 'Paddles, no clutch' }].map(opt => {
                const sel = tx === opt.id;
                return (
                  <div key={opt.id} onClick={() => setTx(opt.id)} style={{
                    padding: '14px 16px', border: `2px solid ${sel ? RP.red : RP.border}`,
                    background: sel ? 'rgba(225,6,0,0.1)' : RP.card,
                    cursor: 'pointer', display: 'flex', alignItems: 'center', gap: 12,
                  }}>
                    <Icon name={opt.id === 'Manual' ? 'gear' : 'gauge'} size={22} color={sel ? RP.red : RP.inkDim} />
                    <div>
                      <div style={{ ...t('h2'), fontFamily: FONT.display, fontSize: 15, color: RP.ink }}>{opt.id.toUpperCase()}</div>
                      <div style={{ ...t('bodySm'), fontSize: 11, color: RP.inkDim, marginTop: 2 }}>{opt.d}</div>
                    </div>
                  </div>
                );
              })}
            </div>

            <div style={{ ...t('caption'), fontSize: 10, color: RP.gunmetal, marginTop: 18, marginBottom: 6 }}>DRIVING AIDS · 4 TIERS</div>
            <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 8 }}>
              <AidCard sel={aid === 'easy'}    onClick={() => setAid('easy')}    title="EASY"    sub="All aids on"  desc="ABS, TC, stability" />
              <AidCard sel={aid === 'hard'}    onClick={() => setAid('hard')}    title="HARD"    sub="Minimal aids" desc="Auto blip + clutch" />
              <AidCard sel={aid === 'ultra'}   onClick={() => setAid('ultra')}   title="ULTRA"   sub="No aids"      desc="As-shipped car" />
              <AidCard sel={aid === 'extreme'} onClick={() => setAid('extreme')} title="EXTREME" sub="Damage on"    desc="No assists" warn />
            </div>

            <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginTop: 18, marginBottom: 6 }}>
              <div style={{ ...t('caption'), fontSize: 10, color: RP.gunmetal }}>AI OPPONENTS</div>
              <div onClick={() => setAi(v => !v)} style={{
                display: 'inline-flex', alignItems: 'center', gap: 8, cursor: 'pointer',
                padding: '4px 8px', border: `1px solid ${ai ? RP.red : RP.border}`,
                background: ai ? 'rgba(225,6,0,0.08)' : 'transparent',
              }}>
                <div style={{
                  width: 22, height: 12, borderRadius: 8, position: 'relative',
                  background: ai ? RP.red : RP.gunmetal, transition: 'background 0.15s',
                }}>
                  <div style={{
                    position: 'absolute', top: 1, left: ai ? 11 : 1, width: 10, height: 10,
                    borderRadius: '50%', background: '#fff', transition: 'left 0.15s',
                  }} />
                </div>
                <div style={{ ...t('mono'), fontSize: 10, color: ai ? RP.red : RP.inkDim }}>{ai ? 'ON' : 'OFF'}</div>
              </div>
            </div>
            <div style={{
              padding: '10px 12px', border: `1px solid ${RP.border}`, background: RP.card,
              opacity: ai ? 1 : 0.4, pointerEvents: ai ? 'auto' : 'none',
            }}>
              <div style={{ ...t('bodySm'), fontSize: 10, color: RP.inkDim, marginBottom: 8 }}>
                Field size is preset by the track. Pick a difficulty:
              </div>
              <div style={{ display: 'grid', gridTemplateColumns: 'repeat(4, 1fr)', gap: 6 }}>
                {[
                  { id: 'easy',   label: 'EASY',   sub: '~70%' },
                  { id: 'medium', label: 'MEDIUM', sub: '~85%' },
                  { id: 'hard',   label: 'HARD',   sub: '~95%' },
                  { id: 'pro',    label: 'PRO',    sub: '100%+' },
                ].map(d => {
                  const sd = aiDiff === d.id;
                  return (
                    <div key={d.id} onClick={() => setAiDiff(d.id)} style={{
                      padding: '8px 6px', textAlign: 'center', cursor: 'pointer',
                      border: `2px solid ${sd ? RP.red : RP.border}`,
                      background: sd ? 'rgba(225,6,0,0.08)' : 'transparent',
                    }}>
                      <div style={{ ...t('h2'), fontFamily: FONT.display, fontSize: 12, color: RP.ink, letterSpacing: '0.04em' }}>{d.label}</div>
                      <div style={{ ...t('mono'), fontSize: 9, color: RP.inkDim, marginTop: 2 }}>{d.sub}</div>
                    </div>
                  );
                })}
              </div>
            </div>
          </div>

          <div>
            <div style={{ ...t('caption'), fontSize: 10, color: RP.gunmetal, marginBottom: 6 }}>SUMMARY</div>
            <div style={{ padding: '18px 20px', background: RP.base, border: `1px solid ${RP.border}`, borderLeft: `2px solid ${RP.red}` }}>
              <div style={{ display: 'grid', gridTemplateColumns: 'auto 1fr', rowGap: 10, columnGap: 18 }}>
                <div style={{ ...t('caption'), fontSize: 9, color: RP.gunmetal }}>DRIVER</div>
                <div style={{ ...t('mono'), fontSize: 12, color: RP.ink }}>{driverName}</div>
                <div style={{ ...t('caption'), fontSize: 9, color: RP.gunmetal }}>GAME</div>
                <div style={{ ...t('mono'), fontSize: 12, color: RP.ink }}>Assetto Corsa</div>
                <div style={{ ...t('caption'), fontSize: 9, color: RP.gunmetal }}>MODE</div>
                <div style={{ ...t('mono'), fontSize: 12, color: RP.ink }}>Single-player</div>
                <div style={{ ...t('caption'), fontSize: 9, color: RP.gunmetal }}>TRANSMISSION</div>
                <div style={{ ...t('mono'), fontSize: 12, color: RP.red }}>{tx.toUpperCase()}</div>
                <div style={{ ...t('caption'), fontSize: 9, color: RP.gunmetal }}>AIDS</div>
                <div style={{ ...t('mono'), fontSize: 12, color: RP.red }}>{aid.toUpperCase()}</div>
                <div style={{ ...t('caption'), fontSize: 9, color: RP.gunmetal }}>AI OPPONENTS</div>
                <div style={{ ...t('mono'), fontSize: 12, color: ai ? RP.red : RP.inkDim }}>{ai ? aiDiff.toUpperCase() : 'OFF'}</div>
              </div>
            </div>

            <div style={{
              marginTop: 14, padding: '12px 14px', background: RP.base, border: `1px dashed ${RP.border}`,
              ...t('bodySm'), fontSize: 11, color: RP.inkDim, lineHeight: 1.6,
            }}>
              <div style={{ ...t('caption'), fontSize: 9, color: RP.gunmetal, marginBottom: 6 }}>NOTE</div>
              These options apply to the car. Track and session length are picked next.
            </div>
          </div>
        </div>

        <div style={{ display: 'flex', alignItems: 'center', gap: 10, marginTop: 14 }}>
          <div style={{ ...t('mono'), fontSize: 10, color: RP.gunmetal }}>TAB TO NAVIGATE · ENTER TO CONTINUE</div>
          <div style={{ flex: 1 }} />
          <PrimaryCTA onClick={onContinue} shortcut="ENTER">Continue → Pick session</PrimaryCTA>
        </div>
      </div>
    </KioskFrame>
  );
}

function AidCard({ sel, onClick, title, sub, desc, warn }: {
  sel: boolean;
  onClick: () => void;
  title: string;
  sub: string;
  desc: string;
  warn?: boolean;
}) {
  return (
    <div onClick={onClick} style={{
      padding: '10px 12px', border: `2px solid ${sel ? (warn ? RP.amber : RP.red) : RP.border}`,
      background: sel ? (warn ? 'rgba(255,176,0,0.1)' : 'rgba(225,6,0,0.08)') : RP.card,
      cursor: 'pointer',
    }}>
      <div style={{ display: 'flex', alignItems: 'baseline', gap: 6 }}>
        <div style={{ ...t('h2'), fontFamily: FONT.display, fontSize: 13, color: RP.ink, letterSpacing: '0.04em' }}>{title}</div>
        <div style={{ ...t('caption'), fontSize: 8, color: warn ? RP.amber : RP.inkDim }}>{sub}</div>
      </div>
      <div style={{ ...t('bodySm'), fontSize: 10, color: RP.inkDim, marginTop: 2 }}>{desc}</div>
    </div>
  );
}
