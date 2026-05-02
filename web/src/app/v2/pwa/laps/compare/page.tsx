'use client';

// Racing Point V2 PWA · My Laps → Lap Comparison
// Source: claude.ai/design handoff bundle hq8O1_qIHf27VSd3rvj51g — page-pwa.jsx (413 lines)
// Mobile-first ("gym tracker for sim racing"). Phone viewport 390×844 (iPhone 14 ref).
// Telemetry chart: you (solid) vs ghost (dashed grey) overlays + sector deltas + insights.

import * as React from 'react';
import { RP, FONT, RADIUS, t } from '@/components/v2/tokens';
import { Icon, ActionButton, DriverClassBadge } from '@/components/v2/atomic';

export default function PWALapComparePage() {
  return (
    <div style={{
      width: '100vw', minHeight: '100vh', background: '#000', color: RP.ink,
      fontFamily: FONT.body, display: 'flex', alignItems: 'center', justifyContent: 'center',
      overflow: 'hidden', boxSizing: 'border-box', padding: 16,
    }}>
      <div style={{
        width: 390, height: 844, background: RP.asphalt, color: RP.ink,
        fontFamily: FONT.body, display: 'flex', flexDirection: 'column',
        overflow: 'hidden', position: 'relative',
        boxShadow: `0 30px 80px rgba(0,0,0,0.5), 0 0 0 1px ${RP.borderHi}`,
        borderRadius: 30,
      }}>
        <PWAStatusBar />
        <PWAHeader />
        <div style={{ flex: 1, overflow: 'auto', display: 'flex', flexDirection: 'column' }}>
          <SessionMeta />
          <CompareToggle />
          <DeltaCallout />
          <TelemetryChart />
          <SectorBand />
          <SectorBreakdown />
          <SessionFooter />
        </div>
        <PWATabBar />
      </div>
    </div>
  );
}

function PWAStatusBar() {
  return (
    <div style={{
      height: 44, padding: '0 24px', display: 'flex', alignItems: 'center',
      justifyContent: 'space-between', background: RP.asphalt, flexShrink: 0,
      ...t('mono'), fontSize: 14, color: RP.ink, fontWeight: 600,
    }}>
      <span>9:41</span>
      <span style={{ display: 'flex', gap: 6, alignItems: 'center' }}>
        <svg width="16" height="11" viewBox="0 0 16 11" fill={RP.ink}>
          <rect x="0"    y="6" width="3" height="5" rx="0.5" />
          <rect x="4.5"  y="4" width="3" height="7" rx="0.5" />
          <rect x="9"    y="2" width="3" height="9" rx="0.5" />
          <rect x="13.5" y="0" width="3" height="11" rx="0.5" />
        </svg>
        <svg width="14" height="10" viewBox="0 0 14 10" fill="none" stroke={RP.ink} strokeWidth="1">
          <path d="M1 4 Q 7 -1 13 4 M3 6 Q 7 2 11 6 M5 8 Q 7 6 9 8" />
        </svg>
        <div style={{ width: 22, height: 10, border: `1px solid ${RP.ink}`, borderRadius: 2, padding: 1, position: 'relative' }}>
          <div style={{ height: '100%', width: '85%', background: RP.ink, borderRadius: 1 }} />
          <div style={{ position: 'absolute', right: -3, top: 3, width: 1.5, height: 4, background: RP.ink, borderRadius: 1 }} />
        </div>
      </span>
    </div>
  );
}

function PWAHeader() {
  return (
    <div style={{
      padding: '8px 20px 12px', display: 'flex', alignItems: 'center', justifyContent: 'space-between',
      borderBottom: `1px solid ${RP.border}`, flexShrink: 0,
    }}>
      <button style={{ background: 'transparent', border: 'none', color: RP.ink, padding: 6, cursor: 'pointer' }}>
        <Icon name="chevron" size={20} color={RP.ink} strokeWidth={2} />
      </button>
      <div style={{ textAlign: 'center' }}>
        <div style={{ ...t('caption'), color: RP.gunmetal, fontSize: 9 }}>SESSION 142 · LAP 14</div>
        <div style={{ ...t('h2'), fontSize: 14, color: RP.ink }}>LAP COMPARISON</div>
      </div>
      <button style={{ background: 'transparent', border: 'none', color: RP.ink, padding: 6, cursor: 'pointer' }}>
        <Icon name="download" size={18} color={RP.ink} strokeWidth={2} />
      </button>
    </div>
  );
}

function SessionMeta() {
  return (
    <div style={{ padding: '16px 20px', display: 'flex', alignItems: 'center', gap: 12 }}>
      <div style={{
        width: 44, height: 44, borderRadius: RADIUS.sm, background: RP.card,
        border: `1px solid ${RP.borderHi}`, display: 'flex', alignItems: 'center', justifyContent: 'center',
        flexShrink: 0,
      }}>
        <Icon name="flag" size={20} color={RP.red} />
      </div>
      <div style={{ flex: 1, minWidth: 0 }}>
        <div style={{ ...t('bodyB'), fontSize: 14, color: RP.ink }}>Spa-Francorchamps</div>
        <div style={{ ...t('mono'), fontSize: 11, color: RP.inkDim, marginTop: 2 }}>iRacing · GT3 · Today 14:30</div>
      </div>
      <DriverClassBadge tier="Champion" size="sm" />
    </div>
  );
}

function CompareToggle() {
  return (
    <div style={{ padding: '0 20px 16px' }}>
      <div style={{ ...t('caption'), color: RP.gunmetal, marginBottom: 8 }}>COMPARING AGAINST</div>
      <div style={{
        background: RP.card, border: `1px solid ${RP.border}`, borderRadius: RADIUS.md,
        padding: 12, display: 'flex', alignItems: 'center', gap: 12,
      }}>
        <div style={{
          width: 36, height: 36, borderRadius: '50%', background: RP.cardHi, border: `1px solid ${RP.borderHi}`,
          ...t('caption'), color: RP.ink, fontSize: 11,
          display: 'flex', alignItems: 'center', justifyContent: 'center',
        }}>VK</div>
        <div style={{ flex: 1, minWidth: 0 }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
            <div style={{ ...t('bodyB'), fontSize: 13, color: RP.ink }}>Vishal Kaushik</div>
            <DriverClassBadge tier="Podium" size="sm" />
          </div>
          <div style={{ ...t('mono'), fontSize: 11, color: RP.inkDim, marginTop: 2 }}>
            #1 leaderboard · Spa GT3 · Track record
          </div>
        </div>
        <button style={{
          padding: '6px 12px', background: 'transparent', border: `1px solid ${RP.borderHi}`,
          color: RP.inkDim, borderRadius: RADIUS.lg, ...t('caption'), fontSize: 10,
          cursor: 'pointer', display: 'flex', alignItems: 'center', gap: 4,
        }}>
          CHANGE <Icon name="chevronD" size={10} />
        </button>
      </div>
    </div>
  );
}

function DeltaCallout() {
  return (
    <div style={{ padding: '0 20px 16px' }}>
      <div style={{
        background: RP.card, border: `1px solid ${RP.border}`,
        borderTop: `2px solid ${RP.red}`,
        padding: 18, borderRadius: RADIUS.sm, position: 'relative', overflow: 'hidden',
      }}>
        <div style={{
          position: 'absolute', right: -40, top: -40, width: 140, height: 140,
          background: `radial-gradient(circle, ${RP.red}15 0%, transparent 70%)`,
        }} />

        <div style={{ display: 'grid', gridTemplateColumns: '1fr 1px 1fr', gap: 14, alignItems: 'flex-end' }}>
          <div>
            <div style={{ ...t('caption'), color: RP.gunmetal, fontSize: 9 }}>YOUR BEST</div>
            <div style={{ ...t('monoBig'), color: RP.ink, fontSize: 30, marginTop: 4, fontVariantNumeric: 'tabular-nums' }}>
              2:00.630
            </div>
            <div style={{ ...t('mono'), fontSize: 10, color: RP.inkDim, marginTop: 4 }}>Lap 14 of 18</div>
          </div>
          <div style={{ background: RP.border, height: 60, alignSelf: 'center' }} />
          <div>
            <div style={{ ...t('caption'), color: RP.gunmetal, fontSize: 9 }}>GHOST</div>
            <div style={{ ...t('monoBig'), color: RP.inkDim, fontSize: 30, marginTop: 4, fontVariantNumeric: 'tabular-nums' }}>
              1:58.412
            </div>
            <div style={{ ...t('mono'), fontSize: 10, color: RP.inkDim, marginTop: 4 }}>Vishal · 8d ago</div>
          </div>
        </div>

        <div style={{
          marginTop: 14, paddingTop: 14, borderTop: `1px solid ${RP.border}`,
          display: 'flex', alignItems: 'center', gap: 10,
        }}>
          <div style={{
            background: RP.red, padding: '6px 10px',
            ...t('mono'), fontSize: 14, color: '#fff', fontWeight: 700, fontVariantNumeric: 'tabular-nums',
          }}>
            +2.218
          </div>
          <div style={{ ...t('body'), fontSize: 12, color: RP.inkDim, lineHeight: 1.4, flex: 1 }}>
            You&apos;re losing time in <span style={{ color: RP.red, fontWeight: 600 }}>Sector 2</span>. La Source &amp; Eau Rouge are on point.
          </div>
        </div>
      </div>
    </div>
  );
}

function TelemetryChart() {
  const W = 350, H = 140;
  const youThrottle = synthTrace(W, H, 12, 0,   1,    'throttle');
  const ghostThrottle = synthTrace(W, H, 14, 0.3, 0.95, 'throttle');
  const youBrake   = synthTrace(W, H, 9,  0,   0.85, 'brake');
  const ghostBrake = synthTrace(W, H, 11, 0.5, 0.9,  'brake');

  return (
    <div style={{ padding: '0 20px 12px' }}>
      <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: 10 }}>
        <div style={{ ...t('caption'), color: RP.ink }}>TELEMETRY · LAP 14</div>
        <div style={{ display: 'flex', gap: 12, ...t('caption'), fontSize: 9 }}>
          <span style={{ color: RP.green, display: 'flex', alignItems: 'center', gap: 4 }}>
            <span style={{ width: 8, height: 2, background: RP.green }} /> THROTTLE
          </span>
          <span style={{ color: RP.red, display: 'flex', alignItems: 'center', gap: 4 }}>
            <span style={{ width: 8, height: 2, background: RP.red }} /> BRAKE
          </span>
        </div>
      </div>

      <div style={{ background: RP.card, border: `1px solid ${RP.border}`, padding: 10, borderRadius: RADIUS.sm }}>
        <svg width={W} height={H} style={{ display: 'block' }}>
          {[0, 0.25, 0.5, 0.75, 1].map(p => (
            <line key={p} x1={0} y1={H * p} x2={W} y2={H * p} stroke={RP.border} strokeWidth="0.5" />
          ))}
          {[0.33, 0.66].map(p => (
            <line key={p} x1={W * p} y1={0} x2={W * p} y2={H} stroke={RP.borderHi} strokeWidth="0.5" strokeDasharray="2,3" />
          ))}
          {['S1', 'S2', 'S3'].map((s, i) => (
            <text key={s} x={W * (0.165 + i * 0.33)} y={11} textAnchor="middle"
              fill={RP.gunmetal} fontSize="9" fontFamily={FONT.mono} fontWeight="600">{s}</text>
          ))}
          <path d={ghostThrottle} fill="none" stroke={RP.gunmetal} strokeWidth="1"   strokeDasharray="3,2" opacity="0.7" />
          <path d={youThrottle}   fill="none" stroke={RP.green}    strokeWidth="1.5" />
          <path d={ghostBrake}    fill="none" stroke={RP.gunmetal} strokeWidth="1"   strokeDasharray="3,2" opacity="0.5" />
          <path d={youBrake}      fill="none" stroke={RP.red}      strokeWidth="1.5" opacity="0.85" />
          <line x1={W * 0.42} y1={0} x2={W * 0.42} y2={H} stroke={RP.ink} strokeWidth="1" opacity="0.6" />
          <circle cx={W * 0.42} cy={H * 0.32} r="3" fill={RP.green} />
          <circle cx={W * 0.42} cy={H * 0.78} r="3" fill={RP.red} />
        </svg>
        <div style={{ display: 'flex', justifyContent: 'space-between', ...t('mono'), fontSize: 9, color: RP.gunmetal, marginTop: 6 }}>
          <span>0%</span><span>50% LAP</span><span>100%</span>
        </div>
      </div>
    </div>
  );
}

function synthTrace(W: number, H: number, seed: number, phase: number, amp: number, kind: 'throttle' | 'brake'): string {
  const N = 60;
  const pts: [number, number][] = [];
  for (let i = 0; i < N; i++) {
    const x = (i / (N - 1)) * W;
    const tt = i / N;
    let v: number;
    if (kind === 'throttle') {
      v = 0.85 - Math.abs(Math.sin((tt + phase) * Math.PI * 6 + seed)) * 0.7 * amp;
      v = Math.max(0.05, Math.min(1, v + 0.2));
      pts.push([x, H - v * H * 0.45]);
    } else {
      const spike = Math.max(0, Math.sin((tt + phase) * Math.PI * 6 + seed) - 0.3) * 1.4 * amp;
      v = Math.min(1, spike);
      pts.push([x, H - v * H * 0.4 - H * 0.5]);
    }
  }
  return pts.map((p, i) => `${i === 0 ? 'M' : 'L'} ${p[0].toFixed(1)} ${p[1].toFixed(1)}`).join(' ');
}

function SectorBand() {
  const SECTORS = [
    { num: 1, name: 'La Source · Eau Rouge', you: '34.218', ghost: '34.402', delta: '-0.184', state: 'green' as const },
    { num: 2, name: 'Les Combes · Pouhon',   you: '52.812', ghost: '49.910', delta: '+2.902', state: 'red' as const },
    { num: 3, name: 'Stavelot · Bus Stop',   you: '33.600', ghost: '34.100', delta: '-0.500', state: 'green' as const },
  ];
  return (
    <div style={{ padding: '0 20px 16px' }}>
      <div style={{ ...t('caption'), color: RP.ink, marginBottom: 10 }}>SECTOR DELTAS</div>
      <div style={{
        display: 'grid', gridTemplateColumns: '1fr 1fr 1fr',
        background: RP.card, border: `1px solid ${RP.border}`,
        borderRadius: RADIUS.sm, overflow: 'hidden',
      }}>
        {SECTORS.map((s, i) => (
          <div key={s.num} style={{
            padding: 12, borderRight: i < 2 ? `1px solid ${RP.border}` : 'none',
            position: 'relative',
          }}>
            <div style={{
              position: 'absolute', top: 0, left: 0, right: 0, height: 3,
              background: s.state === 'green' ? RP.green : RP.red,
            }} />
            <div style={{ ...t('caption'), color: RP.gunmetal, fontSize: 9 }}>S{s.num}</div>
            <div style={{
              ...t('mono'), fontSize: 14, fontWeight: 700,
              color: s.state === 'green' ? RP.green : RP.red,
              marginTop: 4, fontVariantNumeric: 'tabular-nums',
            }}>{s.delta}</div>
            <div style={{ ...t('mono'), fontSize: 10, color: RP.inkDim, marginTop: 2 }}>{s.you}</div>
          </div>
        ))}
      </div>
    </div>
  );
}

function SectorBreakdown() {
  return (
    <div style={{ padding: '0 20px 16px' }}>
      <div style={{ ...t('caption'), color: RP.ink, marginBottom: 10 }}>WHERE TO FIND TIME</div>
      <Insight sector="S2" title="Late braking into Les Combes"           body="Ghost brakes 18m later. You're losing 0.9s on entry alone." delta="+0.9s" bad />
      <Insight sector="S2" title="Throttle application out of Pouhon"     body="Ghost is full throttle 0.4s earlier. Try trail-braking less." delta="+1.2s" bad />
      <Insight sector="S3" title="Bus Stop chicane line"                  body="Your apex is tighter — you're gaining time here."           delta="-0.5s" />
    </div>
  );
}

function Insight({ sector, title, body, delta, bad }: { sector: string; title: string; body: string; delta: string; bad?: boolean }) {
  return (
    <div style={{
      display: 'grid', gridTemplateColumns: '40px 1fr auto', gap: 12, alignItems: 'flex-start',
      padding: '12px 14px', background: RP.card, border: `1px solid ${RP.border}`,
      borderLeft: `3px solid ${bad ? RP.red : RP.green}`,
      marginBottom: 8, borderRadius: RADIUS.sm,
    }}>
      <div style={{
        ...t('caption'), color: bad ? RP.red : RP.green, fontSize: 10,
        background: RP.base, padding: '4px 6px', textAlign: 'center',
      }}>{sector}</div>
      <div>
        <div style={{ ...t('bodyB'), fontSize: 12, color: RP.ink, lineHeight: 1.3 }}>{title}</div>
        <div style={{ ...t('bodySm'), fontSize: 11, color: RP.inkDim, marginTop: 4, lineHeight: 1.4 }}>{body}</div>
      </div>
      <div style={{
        ...t('mono'), fontSize: 12, fontWeight: 700,
        color: bad ? RP.red : RP.green, fontVariantNumeric: 'tabular-nums',
      }}>{delta}</div>
    </div>
  );
}

function SessionFooter() {
  return (
    <div style={{ padding: '4px 20px 20px' }}>
      <div style={{ display: 'flex', gap: 8 }}>
        <ActionButton variant="secondary" size="lg" fullWidth icon={<Icon name="layers" size={14} />}>
          All Laps
        </ActionButton>
        <ActionButton variant="primary" size="lg" fullWidth icon={<Icon name="target" size={14} />}>
          Beat It
        </ActionButton>
      </div>
      <div style={{ ...t('mono'), fontSize: 9, color: RP.gunmetal, textAlign: 'center', marginTop: 12 }}>
        SESSION #4291.142 · TELEMETRY @ 60HZ
      </div>
    </div>
  );
}

function PWATabBar() {
  return (
    <div style={{
      height: 70, background: RP.base, borderTop: `1px solid ${RP.border}`,
      display: 'grid', gridTemplateColumns: 'repeat(4, 1fr)', flexShrink: 0,
      paddingBottom: 12,
    }}>
      <Tab icon="home"     label="Home" />
      <Tab icon="activity" label="Laps" active />
      <Tab icon="trophy"   label="Class" />
      <Tab icon="user"     label="Me" />
    </div>
  );
}

function Tab({ icon, label, active }: { icon: string; label: string; active?: boolean }) {
  return (
    <button style={{
      background: 'transparent', border: 'none', padding: '8px 0', cursor: 'pointer',
      display: 'flex', flexDirection: 'column', alignItems: 'center', gap: 4,
      color: active ? RP.red : RP.inkDim,
    }}>
      <Icon name={icon} size={20} color={active ? RP.red : RP.inkDim} strokeWidth={active ? 2 : 1.6} />
      <span style={{ ...t('caption'), fontSize: 9, color: active ? RP.red : RP.inkDim }}>{label}</span>
    </button>
  );
}
