'use client';

// Racing Point V2 Admin · Pods grid (full-page 4×2 with bigger tiles + filters)
// Source: claude.ai/design handoff bundle hq8O1_qIHf27VSd3rvj51g — extends Cockpit's
// Pod Live Grid into a dedicated full-bleed view with row-level filtering.

import * as React from 'react';
import Link from 'next/link';
import { RP, FONT, t } from '@/components/v2/tokens';
import { Icon, StatusDot, ActionButton, DriverClassBadge } from '@/components/v2/atomic';
import { AdminFrame, AdminPanel } from '@/components/v2/admin/AdminFrame';

type PodState = 'idle' | 'ready' | 'racing' | 'paused' | 'fault';
type PodFilter = 'all' | PodState;

interface Pod {
  id: string;
  state: PodState;
  driver?: string;
  game?: string;
  elapsed?: string;
  remaining?: string;
  cls?: 'Rookie' | 'Apex' | 'Podium' | 'Champion';
  rpm?: number;
  gear?: string;
  speed?: number;
  lastLap?: string;
  bestLap?: string;
  pos?: string;
}

const PODS: Pod[] = [
  { id: 'POD-01', state: 'racing', driver: 'Uday Singh',     game: 'iRacing · Spa-Francorchamps', elapsed: '14:22', remaining: '15:38', cls: 'Champion', rpm: 7400, gear: '5', speed: 268, lastLap: '2:00.6', bestLap: '2:00.6', pos: 'P1' },
  { id: 'POD-02', state: 'racing', driver: 'Vishal Kaushik', game: 'iRacing · Monza',             elapsed: '08:14', remaining: '21:46', cls: 'Podium',   rpm: 6200, gear: '4', speed: 224, lastLap: '1:48.2', bestLap: '1:47.8', pos: 'P3' },
  { id: 'POD-03', state: 'paused', driver: 'Aruna Singh',    game: 'AC · Nordschleife',           elapsed: '03:01', remaining: '26:59', cls: 'Apex',     rpm: 0,    gear: 'N', speed: 0,   lastLap: '—',     bestLap: '8:14.0', pos: 'P7' },
  { id: 'POD-04', state: 'fault',  driver: 'Shashwat Bhagat',game: 'iRacing · Suzuka',            elapsed: '12:44', remaining: '17:16', cls: 'Apex',     rpm: 0,    gear: '—', speed: 0,   lastLap: '—',     bestLap: '—',     pos: '—' },
  { id: 'POD-05', state: 'ready',  driver: 'R. Bangad',      game: 'iRacing · Awaiting start',    elapsed: '00:00', remaining: '30:00', cls: 'Rookie',   rpm: 0,    gear: 'N', speed: 0,   lastLap: '—',     bestLap: '—',     pos: '—' },
  { id: 'POD-06', state: 'racing', driver: 'Karan M.',       game: 'F1 24 · Bahrain',             elapsed: '22:08', remaining: '07:52', cls: 'Apex',     rpm: 8100, gear: '7', speed: 312, lastLap: '1:31.4', bestLap: '1:31.0', pos: 'P5' },
  { id: 'POD-07', state: 'racing', driver: 'Priya N.',       game: 'iRacing · Silverstone',       elapsed: '04:33', remaining: '25:27', cls: 'Rookie',   rpm: 5400, gear: '3', speed: 198, lastLap: '1:54.8', bestLap: '1:54.8', pos: 'P9' },
  { id: 'POD-08', state: 'idle' },
];

const STATE_META: Record<PodState, { c: string; label: string }> = {
  idle:   { c: RP.gunmetal, label: 'IDLE' },
  ready:  { c: RP.blue,     label: 'READY' },
  racing: { c: RP.green,    label: 'RACING' },
  paused: { c: RP.amber,    label: 'PAUSED' },
  fault:  { c: RP.red,      label: 'FAULT' },
};

export default function PodsPage() {
  const [filter, setFilter] = React.useState<PodFilter>('all');

  const counts = React.useMemo(() => ({
    all:    PODS.length,
    racing: PODS.filter(p => p.state === 'racing').length,
    paused: PODS.filter(p => p.state === 'paused').length,
    fault:  PODS.filter(p => p.state === 'fault').length,
    ready:  PODS.filter(p => p.state === 'ready').length,
    idle:   PODS.filter(p => p.state === 'idle').length,
  }), []);

  const visible = filter === 'all' ? PODS : PODS.filter(p => p.state === filter);

  return (
    <AdminFrame active="pods" breadcrumbs={[{ label: 'PODS' }]}>
      <div style={{ padding: '20px 24px 24px', display: 'flex', flexDirection: 'column', gap: 16, flex: 1, minHeight: 0 }}>
        <div style={{ display: 'flex', alignItems: 'baseline', justifyContent: 'space-between' }}>
          <div>
            <div style={{ ...t('caption'), color: RP.gunmetal, fontSize: 10 }}>FLEET</div>
            <div style={{ ...t('displayM'), fontFamily: FONT.display, fontSize: 32, color: RP.ink, marginTop: 2 }}>8 Pods</div>
            <div style={{ ...t('body'), fontSize: 12, color: RP.inkDim, marginTop: 4 }}>
              {counts.racing} racing · {counts.paused} paused · {counts.fault} fault · {counts.ready} ready · {counts.idle} idle
            </div>
          </div>
          <div style={{ display: 'flex', gap: 6 }}>
            <FilterChip label={`ALL · ${counts.all}`}      sel={filter === 'all'}    onClick={() => setFilter('all')} />
            <FilterChip label={`RACING · ${counts.racing}`} sel={filter === 'racing'} accent={RP.green}  onClick={() => setFilter('racing')} />
            <FilterChip label={`PAUSED · ${counts.paused}`} sel={filter === 'paused'} accent={RP.amber}  onClick={() => setFilter('paused')} />
            <FilterChip label={`FAULT · ${counts.fault}`}   sel={filter === 'fault'}  accent={RP.red}    onClick={() => setFilter('fault')} />
            <FilterChip label={`READY · ${counts.ready}`}   sel={filter === 'ready'}  accent={RP.blue}   onClick={() => setFilter('ready')} />
            <FilterChip label={`IDLE · ${counts.idle}`}     sel={filter === 'idle'}                       onClick={() => setFilter('idle')} />
          </div>
        </div>

        <AdminPanel
          title="Live Grid"
          trailing={<span style={{ ...t('mono'), fontSize: 10, color: RP.inkDim }}>CLICK ANY POD FOR DRILL-DOWN</span>}
        >
          <div style={{ flex: 1, padding: 12, display: 'grid', gridTemplateColumns: 'repeat(4, 1fr)', gridAutoRows: 'minmax(220px, 1fr)', gap: 10, minHeight: 0 }}>
            {visible.map(p => <PodTile key={p.id} {...p} />)}
            {visible.length === 0 && (
              <div style={{ gridColumn: '1 / -1', padding: '60px 0', textAlign: 'center', ...t('caption'), color: RP.gunmetal }}>
                NO PODS MATCH THIS FILTER
              </div>
            )}
          </div>
        </AdminPanel>
      </div>
    </AdminFrame>
  );
}

function FilterChip({ label, sel, accent = RP.red, onClick }: {
  label: string;
  sel: boolean;
  accent?: string;
  onClick: () => void;
}) {
  return (
    <button onClick={onClick} style={{
      padding: '6px 12px', cursor: 'pointer', fontFamily: 'inherit',
      background: sel ? `${accent}1F` : 'transparent',
      border: `1px solid ${sel ? accent : RP.border}`,
      color: sel ? accent : RP.inkDim,
      ...t('mono'), fontSize: 10, letterSpacing: '0.06em', fontWeight: 600,
    }}>{label}</button>
  );
}

function PodTile(p: Pod) {
  const s = STATE_META[p.state];
  const elapsedPct = p.elapsed && p.remaining ? mmToS(p.elapsed) / (mmToS(p.elapsed) + mmToS(p.remaining)) : 0;
  const dotState: 'green' | 'amber' | 'red' | 'grey' =
    p.state === 'fault' ? 'red' : p.state === 'paused' ? 'amber' : p.state === 'racing' ? 'green' : p.state === 'ready' ? 'green' : 'grey';

  return (
    <Link href={`/v2/admin/pods/${p.id.toLowerCase()}`} style={{ textDecoration: 'none', color: 'inherit' }}>
      <div style={{
        background: p.state === 'fault' ? '#1a0808' : RP.card,
        border: `1px solid ${p.state === 'fault' ? RP.red : RP.border}`,
        borderTop: `3px solid ${s.c}`,
        padding: 12, height: '100%', display: 'flex', flexDirection: 'column', gap: 8, minHeight: 0,
        position: 'relative', cursor: 'pointer',
      }}>
        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
          <div style={{ display: 'flex', alignItems: 'baseline', gap: 8 }}>
            <div style={{ ...t('h2'), color: RP.ink, fontSize: 16, fontFamily: FONT.display }}>{p.id}</div>
            {p.driver && <div style={{ ...t('mono'), fontSize: 11, color: RP.inkDim }}>{p.driver}</div>}
          </div>
          <div style={{ display: 'flex', gap: 6, alignItems: 'center' }}>
            <StatusDot state={dotState} pulse={p.state === 'fault' || p.state === 'racing'} />
            <span style={{ ...t('caption'), fontSize: 9, color: s.c }}>{s.label}</span>
          </div>
        </div>

        {p.state === 'idle' && (
          <div style={{ flex: 1, display: 'flex', alignItems: 'center', justifyContent: 'center', color: RP.gunmetal, ...t('caption'), fontSize: 11 }}>
            AVAILABLE
          </div>
        )}

        {p.state === 'fault' && (
          <>
            <div style={{ ...t('bodySm'), fontSize: 11, color: RP.inkDim }}>{p.game}</div>
            <div style={{ flex: 1, display: 'flex', flexDirection: 'column', alignItems: 'center', justifyContent: 'center', gap: 6 }}>
              <Icon name="alert" size={28} color={RP.red} />
              <div style={{ ...t('mono'), fontSize: 11, color: RP.red, letterSpacing: '0.08em' }}>PEDALS OFFLINE</div>
              <div style={{ ...t('caption'), fontSize: 9, color: RP.amber }}>FAILOVER ENGAGED · E-2204</div>
            </div>
          </>
        )}

        {p.state === 'ready' && (
          <>
            <div style={{ ...t('bodySm'), fontSize: 11, color: RP.inkDim }}>{p.game}</div>
            {p.cls && <div style={{ display: 'flex' }}><DriverClassBadge tier={p.cls} size="sm" /></div>}
            <div style={{ flex: 1, display: 'flex', alignItems: 'center', justifyContent: 'center', ...t('mono'), fontSize: 12, color: RP.blue, letterSpacing: '0.08em' }}>
              ● AWAITING START
            </div>
          </>
        )}

        {(p.state === 'racing' || p.state === 'paused') && (
          <>
            <div style={{ ...t('bodySm'), fontSize: 11, color: RP.inkDim, lineHeight: 1.3 }}>{p.game}</div>
            <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr 1fr', gap: 6 }}>
              <Stat label="GEAR"  value={p.gear ?? '—'} dim={p.state === 'paused'} />
              <Stat label="SPEED" value={`${p.speed ?? 0}`} unit="km/h" dim={p.state === 'paused'} />
              <Stat label="POS"   value={p.pos ?? '—'} hi />
            </div>
            <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 6 }}>
              <Stat label="LAST LAP" value={p.lastLap ?? '—'} small />
              <Stat label="BEST LAP" value={p.bestLap ?? '—'} small hi />
            </div>
            <div style={{ flex: 1 }} />
            <div style={{ display: 'flex', justifyContent: 'space-between', ...t('mono'), fontSize: 11 }}>
              <span style={{ color: RP.ink }}>+{p.elapsed}</span>
              <span style={{ color: s.c }}>−{p.remaining}</span>
            </div>
            <div style={{ height: 3, background: RP.border, position: 'relative' }}>
              <div style={{ position: 'absolute', top: 0, left: 0, height: '100%', width: `${elapsedPct * 100}%`, background: s.c }} />
            </div>
          </>
        )}
      </div>
    </Link>
  );
}

function Stat({ label, value, unit, hi, dim, small }: {
  label: string; value: string; unit?: string; hi?: boolean; dim?: boolean; small?: boolean;
}) {
  return (
    <div style={{ padding: '6px 8px', background: RP.base, border: `1px solid ${RP.border}` }}>
      <div style={{ ...t('caption'), fontSize: 8, color: RP.gunmetal }}>{label}</div>
      <div style={{
        ...t('mono'),
        fontFamily: small ? undefined : FONT.display,
        fontSize: small ? 12 : 18,
        color: hi ? RP.red : (dim ? RP.inkDim : RP.ink),
        marginTop: 2, lineHeight: 1,
      }}>
        {value}{unit && <span style={{ fontSize: 9, color: RP.gunmetal, marginLeft: 4 }}>{unit}</span>}
      </div>
    </div>
  );
}

function mmToS(s: string): number {
  const [m, sec] = s.split(':').map(Number);
  return m * 60 + sec;
}

void ActionButton;
