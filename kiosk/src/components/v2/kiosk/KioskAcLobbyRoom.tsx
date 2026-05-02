'use client';

// Racing Point V2 Kiosk — AC MP Lobby Room (post-server-pick waiting room)
// Source: claude.ai/design handoff bundle (hq8O1_qIHf27VSd3rvj51g) — 2026-05-02
// Sits between KioskAcLobby (server pick) and KioskHandoff. MP path skips Experience —
// host config (track/car/format) is locked at lobby creation.

import * as React from 'react';
import { RP, FONT, t } from '../tokens';
import { StatusDot, SecondaryCTA } from '../atomic';
import { KioskFrame } from './KioskFrame';

export interface AcLobbyInfo {
  name: string;
  track: string;
  car: string;
  format?: string;
  host?: string;
}

interface RosterDriver {
  pod: string;
  name: string;
  car: string;
  livery: string;
  ping: number;
  ready: boolean;
  you: boolean;
  cls: string;
}

export function KioskAcLobbyRoom({
  lobby,
  onLaunch,
  onLeave,
}: {
  lobby?: AcLobbyInfo;
  onLaunch: () => void;
  onLeave: () => void;
}) {
  const L: AcLobbyInfo = lobby || {
    name: 'TUESDAY-LEAGUE',
    track: 'Spa-Francorchamps',
    car: 'GT3',
    format: 'Sprint · 8 laps',
    host: 'Staff (Aman)',
  };

  const [iAmReady, setIAmReady] = React.useState(false);
  const [tick, setTick] = React.useState(0);
  const [greenLit, setGreenLit] = React.useState(false);
  const [countdown, setCountdown] = React.useState<number | null>(null);

  React.useEffect(() => {
    const id = setInterval(() => setTick(x => x + 1), 1500);
    return () => clearInterval(id);
  }, []);

  const ROSTER: RosterDriver[] = [
    { pod: 'POD-01', name: 'Uday Singh',  car: 'BMW M4 GT3 #14',          livery: '#e10600', ping: 4, ready: iAmReady, you: true,  cls: 'Silver' },
    { pod: 'POD-02', name: 'V. Kaushik',  car: 'Mercedes AMG GT3 #7',     livery: '#1e90ff', ping: 6, ready: tick >= 1, you: false, cls: 'Silver' },
    { pod: 'POD-03', name: 'A. Singh',    car: 'Audi R8 LMS #22',         livery: '#00d26a', ping: 5, ready: tick >= 3, you: false, cls: 'Bronze' },
    { pod: 'POD-04', name: 'R. Bengali',  car: 'Porsche 911 GT3 R #88',   livery: '#ffb000', ping: 7, ready: tick >= 5, you: false, cls: 'Gold' },
  ];
  const allReady = ROSTER.every(r => r.ready);
  const readyCount = ROSTER.filter(r => r.ready).length;

  React.useEffect(() => {
    if (allReady && !greenLit) {
      const id = setTimeout(() => setGreenLit(true), 3000);
      return () => clearTimeout(id);
    }
  }, [allReady, greenLit]);

  React.useEffect(() => {
    if (!greenLit) return;
    setCountdown(3);
    const id = setInterval(() => {
      setCountdown(c => {
        if (c === null) return null;
        if (c <= 1) { clearInterval(id); onLaunch(); return 0; }
        return c - 1;
      });
    }, 1000);
    return () => clearInterval(id);
  }, [greenLit, onLaunch]);

  const CHAT: { who: string; when: string; txt: string; sys?: boolean }[] = [
    { who: 'HOST · Aman',   when: '−02:14', txt: `Welcome to ${L.name}. ${L.format || '8-lap sprint'}, no pit stops.`, sys: true },
    { who: 'V. Kaushik',    when: '−01:48', txt: 'Ready when you are.' },
    { who: 'A. Singh',      when: '−01:22', txt: 'Same livery as last week, locked in.' },
    { who: 'HOST · Aman',   when: '−00:34', txt: 'Race auto-launches when ALL drivers are READY and I green-light the grid.', sys: true },
    ...(greenLit ? [{ who: 'HOST · Aman', when: 'NOW', txt: '● GREEN-LIGHT · launching all pods simultaneously…', sys: true }] : []),
  ];

  return (
    <KioskFrame podId="POD-01" state={greenLit ? 'live' : 'idle'}>
      <div style={{ height: '100%', display: 'flex', flexDirection: 'column', padding: '16px 28px', overflow: 'hidden', position: 'relative' }}>
        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
          <div style={{ ...t('caption'), fontSize: 10, color: RP.amber }}>● IN LOBBY · {L.name} · WAITING TO START</div>
          <SecondaryCTA onClick={onLeave} shortcut="ESC">Leave lobby</SecondaryCTA>
        </div>
        <div style={{ ...t('displayM'), fontFamily: FONT.display, fontSize: 26, color: RP.ink, marginTop: 4 }}>Lobby room</div>
        <div style={{ ...t('body'), fontSize: 12, color: RP.inkDim, marginTop: 2 }}>
          {readyCount} of {ROSTER.length} drivers ready · race launches when staff at <span style={{ color: RP.amber }}>Race Control</span> green-lights the grid.
        </div>

        <div style={{ marginTop: 14, flex: 1, minHeight: 0, display: 'grid', gridTemplateColumns: '1.4fr 1fr', gap: 14 }}>
          <div style={{ display: 'flex', flexDirection: 'column', gap: 6, minHeight: 0 }}>
            <div style={{ ...t('caption'), fontSize: 10, color: RP.gunmetal, letterSpacing: '0.12em' }}>CONNECTED DRIVERS · {ROSTER.length}/8</div>
            <div style={{ display: 'flex', flexDirection: 'column', gap: 6, overflow: 'auto' }}>
              {ROSTER.map((r, i) => (
                <RosterRow key={i} r={r} pos={i + 1} />
              ))}
              {Array.from({ length: 8 - ROSTER.length }).map((_, i) => (
                <div key={`empty-${i}`} style={{
                  padding: '10px 14px', border: `1px dashed ${RP.border}`, background: 'transparent',
                  display: 'flex', alignItems: 'center', gap: 12,
                }}>
                  <div style={{ ...t('mono'), fontSize: 11, color: RP.gunmetal, width: 38 }}>P{ROSTER.length + i + 1}</div>
                  <div style={{ ...t('mono'), fontSize: 10, color: RP.gunmetal, flex: 1 }}>OPEN SLOT · WAITING FOR DRIVER</div>
                </div>
              ))}
            </div>
          </div>

          <div style={{ display: 'flex', flexDirection: 'column', gap: 10, minHeight: 0 }}>
            <div style={{ padding: '12px 14px', background: RP.card, border: `1px solid ${RP.border}`, borderLeft: `3px solid ${RP.amber}` }}>
              <div style={{ ...t('caption'), fontSize: 9, color: RP.gunmetal, letterSpacing: '0.12em' }}>HOST CONFIG · LOCKED</div>
              <div style={{ ...t('h2'), fontSize: 13, color: RP.ink, marginTop: 6 }}>{L.track}</div>
              <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', rowGap: 6, columnGap: 12, marginTop: 8 }}>
                <Spec k="CAR CLASS" v={L.car} />
                <Spec k="FORMAT" v={L.format || 'Sprint · 8 laps'} />
                <Spec k="WEATHER" v="Dry · 24°C" />
                <Spec k="TIME" v="14:30 (afternoon)" />
                <Spec k="ASSISTS" v="Host: ABS auto · TC off" />
                <Spec k="PIT STOPS" v="Disabled" />
                <Spec k="DAMAGE" v="Visual + reduced mech." />
                <Spec k="FUEL / TYRES" v="Off · arcade" />
              </div>
            </div>

            <div style={{ flex: 1, minHeight: 0, padding: '12px 14px', background: RP.base, border: `1px solid ${RP.border}`, display: 'flex', flexDirection: 'column' }}>
              <div style={{ ...t('caption'), fontSize: 9, color: RP.gunmetal, letterSpacing: '0.12em' }}>LOBBY CHAT · READ-ONLY ON POD</div>
              <div style={{ flex: 1, overflow: 'auto', marginTop: 6, display: 'flex', flexDirection: 'column', gap: 6 }}>
                {CHAT.map((c, i) => (
                  <div key={i} style={{
                    padding: '6px 8px',
                    background: c.sys ? 'rgba(255,176,0,0.06)' : 'transparent',
                    borderLeft: c.sys ? `2px solid ${RP.amber}` : `2px solid ${RP.border}`,
                  }}>
                    <div style={{ display: 'flex', justifyContent: 'space-between', gap: 8 }}>
                      <span style={{ ...t('mono'), fontSize: 10, color: c.sys ? RP.amber : RP.ink, fontWeight: 600 }}>{c.who}</span>
                      <span style={{ ...t('mono'), fontSize: 9, color: RP.gunmetal }}>{c.when}</span>
                    </div>
                    <div style={{ ...t('bodySm'), fontSize: 11, color: RP.inkDim, marginTop: 2 }}>{c.txt}</div>
                  </div>
                ))}
              </div>
            </div>
          </div>
        </div>

        <div style={{
          marginTop: 14, padding: '14px 18px',
          background: greenLit ? 'rgba(45,209,116,0.10)' : (allReady ? 'rgba(255,176,0,0.06)' : RP.card),
          border: `1px solid ${greenLit ? RP.green : (allReady ? RP.amber : RP.border)}`,
          borderLeft: `3px solid ${greenLit ? RP.green : (allReady ? RP.amber : RP.gunmetal)}`,
          display: 'grid', gridTemplateColumns: '1fr auto auto', gap: 16, alignItems: 'center',
        }}>
          <div>
            <div style={{ ...t('caption'), fontSize: 9, color: RP.gunmetal, letterSpacing: '0.12em' }}>YOUR STATUS</div>
            <div style={{ ...t('h2'), fontSize: 14, color: iAmReady ? (greenLit ? RP.green : RP.amber) : RP.ink, marginTop: 4 }}>
              {!iAmReady && 'Click Ready when seated and pedals tested'}
              {iAmReady && !allReady && '● You are READY · waiting for other drivers'}
              {iAmReady && allReady && !greenLit && '● ALL DRIVERS READY · waiting for Race Control to green-light grid'}
              {greenLit && '● GREEN LIGHT · launching all pods simultaneously'}
            </div>
            <div style={{ ...t('mono'), fontSize: 10, color: RP.inkDim, marginTop: 4 }}>
              {readyCount}/{ROSTER.length} READY
              {!allReady && ` · WAITING ON ${ROSTER.length - readyCount} DRIVER${ROSTER.length - readyCount === 1 ? '' : 'S'}`}
              {allReady && !greenLit && ' · STAFF GREEN-LIGHTS THE WHOLE GRID — POD CAN\'T SELF-LAUNCH IN MP'}
              {greenLit && ' · ALL PODS LOAD INTO RACE TOGETHER — NO HEAD-START'}
            </div>
          </div>
          {!iAmReady ? (
            <button onClick={() => setIAmReady(true)} style={{
              background: RP.green, color: '#000', border: 'none', padding: '14px 22px', cursor: 'pointer',
              fontFamily: FONT.display, fontSize: 14, letterSpacing: '0.08em', fontWeight: 700,
            }}>I&apos;M READY ↵</button>
          ) : (
            <div style={{ ...t('mono'), fontSize: 11, color: greenLit ? RP.green : RP.amber, padding: '12px 16px', border: `1px solid ${greenLit ? RP.green : RP.amber}` }}>● READY</div>
          )}
          <div style={{
            padding: '12px 16px', border: `1px dashed ${greenLit ? RP.green : RP.border}`,
            background: greenLit ? 'rgba(45,209,116,0.06)' : 'transparent',
            display: 'flex', alignItems: 'center', gap: 8,
          }}>
            <StatusDot state={greenLit ? 'green' : 'grey'} pulse={greenLit} />
            <div style={{ ...t('mono'), fontSize: 10, color: greenLit ? RP.green : RP.gunmetal, letterSpacing: '0.08em' }}>
              {greenLit ? `LAUNCHING IN ${countdown ?? 0}…` : 'AWAITING GREEN-LIGHT FROM RACE CONTROL'}
            </div>
          </div>
        </div>

        {greenLit && (
          <div style={{
            position: 'absolute', inset: 0, background: 'rgba(0,0,0,0.78)',
            display: 'flex', flexDirection: 'column', alignItems: 'center', justifyContent: 'center',
            zIndex: 10, animation: 'rp-fade-in 0.2s ease',
          }}>
            <div style={{ ...t('caption'), fontSize: 12, color: RP.green, letterSpacing: '0.2em' }}>● GREEN LIGHT · GRID LAUNCHING</div>
            <div style={{ fontFamily: FONT.display, fontSize: 220, color: RP.green, lineHeight: 1, marginTop: 16, fontWeight: 800 }}>
              {countdown ?? '—'}
            </div>
            <div style={{ ...t('mono'), fontSize: 11, color: RP.inkDim, marginTop: 12, letterSpacing: '0.12em' }}>
              ALL 4 PODS LOADING INTO RACE · NO HEAD-START
            </div>
          </div>
        )}
        <style>{`@keyframes rp-fade-in { from { opacity: 0; } to { opacity: 1; } }`}</style>
      </div>
    </KioskFrame>
  );
}

function RosterRow({ r, pos }: { r: RosterDriver; pos: number }) {
  return (
    <div style={{
      padding: '10px 14px',
      background: r.you ? 'rgba(225,6,0,0.06)' : RP.card,
      border: `1px solid ${r.you ? RP.red : RP.border}`,
      borderLeft: `3px solid ${r.ready ? RP.green : RP.amber}`,
      display: 'grid', gridTemplateColumns: 'auto 6px 1fr 110px 70px 80px', gap: 12, alignItems: 'center',
    }}>
      <div style={{ ...t('mono'), fontSize: 11, color: RP.gunmetal, width: 38 }}>P{pos}</div>
      <div style={{ width: 4, height: 24, background: r.livery }} />
      <div>
        <div style={{ ...t('h2'), fontSize: 13, color: RP.ink }}>
          {r.name}{' '}
          {r.you && (
            <span style={{ ...t('mono'), fontSize: 9, color: RP.red, marginLeft: 6, padding: '1px 4px', border: `1px solid ${RP.red}`, letterSpacing: '0.08em' }}>
              YOU
            </span>
          )}
        </div>
        <div style={{ ...t('mono'), fontSize: 10, color: RP.inkDim, marginTop: 2 }}>{r.pod} · {r.cls}</div>
      </div>
      <div style={{ ...t('mono'), fontSize: 11, color: RP.inkDim }}>{r.car}</div>
      <div style={{ ...t('mono'), fontSize: 10, color: r.ping < 10 ? RP.green : RP.amber, textAlign: 'right' }}>
        {r.ping} ms
      </div>
      <div style={{
        ...t('mono'), fontSize: 10, textAlign: 'center', padding: '4px 6px',
        color: r.ready ? RP.green : RP.amber,
        border: `1px solid ${r.ready ? RP.green : RP.amber}`,
        letterSpacing: '0.08em',
      }}>{r.ready ? '● READY' : '○ WAITING'}</div>
    </div>
  );
}

function Spec({ k, v }: { k: string; v: string }) {
  return (
    <div>
      <div style={{ ...t('caption'), fontSize: 8, color: RP.gunmetal, letterSpacing: '0.1em' }}>{k}</div>
      <div style={{ ...t('mono'), fontSize: 11, color: RP.ink, marginTop: 2 }}>{v}</div>
    </div>
  );
}
