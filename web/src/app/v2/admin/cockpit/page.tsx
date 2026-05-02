'use client';

// Racing Point V2 Admin · Cockpit (default landing for staff)
// Source: claude.ai/design handoff bundle hq8O1_qIHf27VSd3rvj51g — page-cockpit.jsx (459 lines)
// Hierarchy: top-bar → KPI strip → 3-col body (alerts | pod grid | revenue+health)
// Triple-monitor friendly composition; reads as the leftmost monitor of a triple-mon setup.

import * as React from 'react';
import { RP, FONT, RADIUS, MOTION, t } from '@/components/v2/tokens';
import {
  Icon,
  StatusDot,
  StatusTile,
  AlertRow,
  ActionButton,
  DriverClassBadge,
} from '@/components/v2/atomic';

type PodState = 'idle' | 'ready' | 'racing' | 'paused' | 'fault';

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

export default function CockpitPage() {
  return (
    <div style={{
      width: '100vw', height: '100vh', background: RP.asphalt, color: RP.ink,
      fontFamily: FONT.body, display: 'flex', flexDirection: 'column',
      overflow: 'hidden', boxSizing: 'border-box',
    }}>
      <CockpitTopBar />
      <CockpitNavRail />
      <div style={{ display: 'flex', flex: 1, minHeight: 0 }}>
        <NavRailSpacer />
        <div style={{ flex: 1, padding: '20px 24px 24px', display: 'flex', flexDirection: 'column', gap: 16, minWidth: 0 }}>
          <KpiStrip />
          <div style={{ display: 'grid', gridTemplateColumns: '320px 1fr 320px', gap: 16, flex: 1, minHeight: 0 }}>
            <AlertColumn />
            <PodGridColumn />
            <RightColumn />
          </div>
        </div>
      </div>
    </div>
  );
}

// ── Top bar ─────────────────────────────────────────────────
function CockpitTopBar() {
  return (
    <div style={{
      height: 48, background: RP.base, borderBottom: `1px solid ${RP.border}`,
      display: 'flex', alignItems: 'center', padding: '0 20px', gap: 24, flexShrink: 0,
    }}>
      <div style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
        <div style={{ width: 24, height: 24, background: RP.red, clipPath: 'polygon(0 0, 100% 0, 75% 100%, 0% 100%)' }} />
        <div style={{ ...t('h2'), color: RP.ink, fontSize: 16, letterSpacing: '0.06em' }}>RACING POINT</div>
        <div style={{ ...t('caption'), color: RP.red, fontSize: 9, padding: '2px 6px', border: `1px solid ${RP.red}` }}>ADMIN</div>
      </div>
      <div style={{ width: 1, height: 24, background: RP.border }} />
      <div style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
        <div style={{ ...t('mono'), fontSize: 10, color: RP.ink, letterSpacing: '0.14em', fontWeight: 600 }}>RACECONTROL</div>
        <div style={{ ...t('mono'), fontSize: 9, color: RP.amber, padding: '1px 5px', border: `1px solid ${RP.amber}`, letterSpacing: '0.06em' }}>v2.0</div>
      </div>
      <div style={{ width: 1, height: 24, background: RP.border }} />
      <div style={{ display: 'flex', gap: 6, ...t('caption'), color: RP.gunmetal, fontSize: 10 }}>
        <span>VENUE</span><span style={{ color: RP.ink }}>BANDLAGUDA</span>
        <span style={{ color: RP.borderHi, margin: '0 6px' }}>/</span>
        <span>MODE</span><span style={{ color: RP.green }}>LIVE</span>
      </div>
      <div style={{ flex: 1 }} />
      <div style={{ display: 'flex', alignItems: 'center', gap: 16, ...t('mono'), fontSize: 11, color: RP.inkDim }}>
        <span>16:42:08 IST</span>
        <span style={{ display: 'flex', gap: 6, alignItems: 'center' }}>
          <StatusDot state="green" /> JAMES <span style={{ color: RP.gunmetal }}>· 43ms</span>
        </span>
        <span style={{ display: 'flex', gap: 6, alignItems: 'center' }}>
          <StatusDot state="green" /> BONO
        </span>
      </div>
      <div style={{ display: 'flex', gap: 8 }}>
        <CornerBtn icon="bell" badge="2" />
        <CornerBtn icon="settings" />
      </div>
      <div style={{ width: 28, height: 28, borderRadius: '50%', background: RP.cardHi, border: `1px solid ${RP.borderHi}`, ...t('caption'), color: RP.ink, fontSize: 10, display: 'flex', alignItems: 'center', justifyContent: 'center' }}>UD</div>
    </div>
  );
}

function CornerBtn({ icon, badge }: { icon: string; badge?: string }) {
  return (
    <button style={{
      width: 32, height: 32, background: 'transparent', border: `1px solid ${RP.border}`,
      borderRadius: RADIUS.md, color: RP.inkDim, cursor: 'pointer', position: 'relative',
      display: 'flex', alignItems: 'center', justifyContent: 'center',
    }}>
      <Icon name={icon} size={14} />
      {badge && (
        <span style={{
          position: 'absolute', top: -4, right: -4, minWidth: 14, height: 14, borderRadius: 7,
          background: RP.red, color: '#fff', ...t('mono'), fontSize: 9, fontWeight: 700,
          display: 'flex', alignItems: 'center', justifyContent: 'center', padding: '0 4px',
        }}>{badge}</span>
      )}
    </button>
  );
}

// ── Left nav rail ───────────────────────────────────────────
function CockpitNavRail() {
  return (
    <div style={{
      position: 'absolute', left: 0, top: 48, bottom: 0, width: 56,
      background: RP.base, borderRight: `1px solid ${RP.border}`,
      display: 'flex', flexDirection: 'column', alignItems: 'center', padding: '12px 0', gap: 4,
    }}>
      <NavItem icon="grid" active label="Cockpit" />
      <NavItem icon="zap" label="Pods" />
      <NavItem icon="layers" label="Services" />
      <NavItem icon="user" label="Customers" />
      <NavItem icon="activity" label="Telemetry" />
      <NavItem icon="trophy" label="Leaderboard" />
      <div style={{ flex: 1 }} />
      <NavItem icon="flag" label="Flags" />
      <NavItem icon="settings" label="Settings" />
    </div>
  );
}
function NavRailSpacer() { return <div style={{ width: 56, flexShrink: 0 }} />; }

function NavItem({ icon, label, active }: { icon: string; label: string; active?: boolean }) {
  return (
    <div style={{ position: 'relative', width: '100%', display: 'flex', justifyContent: 'center' }}>
      {active && <div style={{ position: 'absolute', left: 0, top: 4, bottom: 4, width: 2, background: RP.red }} />}
      <div style={{
        width: 40, height: 40, borderRadius: RADIUS.sm,
        background: active ? RP.cardHi : 'transparent',
        color: active ? RP.ink : RP.inkDim,
        display: 'flex', flexDirection: 'column', alignItems: 'center', justifyContent: 'center', gap: 2,
        cursor: 'pointer',
      }}>
        <Icon name={icon} size={16} />
        <span style={{ ...t('caption'), fontSize: 7.5 }}>{label.toUpperCase()}</span>
      </div>
    </div>
  );
}

// ── KPI strip ───────────────────────────────────────────────
function KpiStrip() {
  return (
    <div style={{ display: 'grid', gridTemplateColumns: 'repeat(6, 1fr)', gap: 12 }}>
      <StatusTile label="Active Sessions" value="6/8" sub="2 idle · 0 fault" accent={RP.green} />
      <StatusTile label="Today Revenue" value="₹48,200" sub="+12% vs Mon" accent={RP.amber} />
      <StatusTile label="Bookings Today" value="14" sub="3 walk-ins · 11 booked" accent={RP.blue} />
      <StatusTile label="Open Alerts" value="2" sub="1 critical · 1 warn" accent={RP.red} />
      <StatusTile label="Avg Session" value="42:18" sub="−3:12 vs avg" accent={RP.gunmetal} />
      <StatusTile label="Util Today" value="68%" sub="peak 19:00–22:00" accent={RP.green} />
    </div>
  );
}

// ── Alert column ────────────────────────────────────────────
function AlertColumn() {
  return (
    <Panel title="Active Alerts" trailing={<span style={{ ...t('caption'), color: RP.red, fontSize: 9 }}>● 2 OPEN</span>}>
      <div style={{ flex: 1, overflow: 'hidden' }}>
        <AlertRow severity="red"   code="E-2204" msg="Pedals offline · failover engaged"     source="POD-04" time="16:32" />
        <AlertRow severity="amber" code="W-1108" msg="Telemetry latency 240ms"               source="POD-07" time="16:28" />
        <AlertRow severity="blue"  code="I-0042" msg="James restarted iRacing service"       source="POD-02" time="16:14" />
        <AlertRow severity="blue"  code="I-0041" msg="Customer A. Singh checked in"          source="POS"    time="16:11" />
        <AlertRow severity="green" code="I-0040" msg="POD-05 build deployed · 2026.05.02-a4f" source="DEPLOY" time="15:58" />
        <AlertRow severity="amber" code="W-1107" msg="Wheel base re-calibration suggested"   source="POD-03" time="15:42" />
        <AlertRow severity="blue"  code="I-0039" msg="Bono nightly profit summary ready"     source="BONO"   time="15:00" />
      </div>
      <div style={{ padding: '10px 12px', borderTop: `1px solid ${RP.border}`, display: 'flex', gap: 6 }}>
        <ActionButton variant="secondary" size="sm" fullWidth>Acknowledge All</ActionButton>
        <ActionButton variant="ghost" size="sm">Mute</ActionButton>
      </div>
    </Panel>
  );
}

// ── Pod grid (centerpiece) ──────────────────────────────────
function PodGridColumn() {
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
  return (
    <Panel
      title="Pod Live Grid"
      trailing={
        <div style={{ display: 'flex', gap: 12, ...t('caption'), fontSize: 9 }}>
          <span style={{ color: RP.green }}>● 5 RACING</span>
          <span style={{ color: RP.amber }}>● 1 PAUSED</span>
          <span style={{ color: RP.red }}>● 1 FAULT</span>
          <span style={{ color: RP.gunmetal }}>● 1 IDLE</span>
        </div>
      }
    >
      <div style={{ flex: 1, padding: 12, display: 'grid', gridTemplateColumns: 'repeat(4, 1fr)', gridTemplateRows: 'repeat(2, 1fr)', gap: 8, minHeight: 0 }}>
        {PODS.map(p => <PodTileLarge key={p.id} {...p} />)}
      </div>
      <div style={{ padding: '10px 12px', borderTop: `1px solid ${RP.border}`, display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
        <div style={{ ...t('mono'), fontSize: 11, color: RP.inkDim }}>POD-01 selected · uptime 14:22</div>
        <div style={{ display: 'flex', gap: 6 }}>
          <ActionButton variant="secondary" size="sm" icon={<Icon name="refresh" size={12} />}>Restart</ActionButton>
          <ActionButton variant="secondary" size="sm" icon={<Icon name="eye" size={12} />}>Blank</ActionButton>
          <ActionButton variant="danger" size="sm" icon={<Icon name="power" size={12} />}>Kill</ActionButton>
          <ActionButton variant="primary" size="sm" icon={<Icon name="play" size={12} />}>Launch</ActionButton>
        </div>
      </div>
    </Panel>
  );
}

function PodTileLarge({ id, state, driver, game, elapsed, remaining, cls, rpm, gear, speed, lastLap, bestLap, pos }: Pod) {
  const STATES: Record<PodState, { c: string; label: string }> = {
    idle:   { c: RP.gunmetal, label: 'IDLE' },
    ready:  { c: RP.blue,     label: 'READY' },
    racing: { c: RP.green,    label: 'RACING' },
    paused: { c: RP.amber,    label: 'PAUSED' },
    fault:  { c: RP.red,      label: 'FAULT' },
  };
  const s = STATES[state];
  const elapsedPct = elapsed && remaining ? mmToS(elapsed) / (mmToS(elapsed) + mmToS(remaining)) : 0;
  const dotState: 'green' | 'amber' | 'red' | 'grey' =
    state === 'fault' ? 'red' : state === 'paused' ? 'amber' : state === 'racing' ? 'green' : 'grey';
  return (
    <div style={{
      background: state === 'fault' ? '#1a0808' : RP.card,
      border: `1px solid ${state === 'fault' ? RP.red : RP.border}`,
      borderTop: `3px solid ${s.c}`,
      padding: 8, display: 'flex', flexDirection: 'column', gap: 6, minHeight: 0,
      position: 'relative', overflow: 'hidden',
    }}>
      <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
        <div style={{ display: 'flex', alignItems: 'baseline', gap: 6 }}>
          <div style={{ ...t('h2'), color: RP.ink, fontSize: 12, fontFamily: FONT.display }}>{id}</div>
          {driver && <div style={{ ...t('mono'), fontSize: 10, color: RP.inkDim, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap', maxWidth: 90 }}>{driver}</div>}
        </div>
        <div style={{ display: 'flex', gap: 4, alignItems: 'center' }}>
          <StatusDot state={dotState} pulse={state === 'fault' || state === 'racing'} />
          <span style={{ ...t('caption'), fontSize: 8, color: s.c }}>{s.label}</span>
        </div>
      </div>

      {state === 'idle' && (
        <>
          <div style={{ flex: 1, display: 'flex', alignItems: 'center', justifyContent: 'center', color: RP.gunmetal, ...t('caption'), fontSize: 10 }}>AVAILABLE</div>
          <ActionButton variant="secondary" size="sm" fullWidth>Assign</ActionButton>
        </>
      )}

      {state === 'fault' && (
        <>
          <div style={{ ...t('bodySm'), fontSize: 10, color: RP.inkDim, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{game}</div>
          <div style={{ flex: 1, display: 'flex', flexDirection: 'column', alignItems: 'center', justifyContent: 'center', gap: 4, padding: '4px 0' }}>
            <Icon name="alert" size={20} color={RP.red} />
            <div style={{ ...t('mono'), fontSize: 10, color: RP.red, letterSpacing: '0.06em' }}>PEDALS OFFLINE</div>
            <div style={{ ...t('caption'), fontSize: 9, color: RP.amber }}>FAILOVER ENGAGED</div>
          </div>
          <ActionButton variant="danger" size="sm" fullWidth>Open ticket</ActionButton>
        </>
      )}

      {state === 'ready' && (
        <>
          <div style={{ ...t('bodySm'), fontSize: 10, color: RP.inkDim, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{game}</div>
          {cls && <div style={{ display: 'flex', justifyContent: 'flex-end' }}><DriverClassBadge tier={cls} size="sm" /></div>}
          <div style={{ flex: 1, display: 'flex', alignItems: 'center', justifyContent: 'center', ...t('mono'), fontSize: 11, color: RP.blue, letterSpacing: '0.06em' }}>● AWAITING START</div>
        </>
      )}

      {(state === 'racing' || state === 'paused') && (
        <>
          <div style={{ ...t('bodySm'), fontSize: 9, color: RP.inkDim, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap', lineHeight: 1.2 }}>{game}</div>

          <div style={{ display: 'grid', gridTemplateColumns: '38px 36px 1fr', gap: 6, alignItems: 'center', marginTop: 2 }}>
            <MiniRpmDial value={rpm ?? 0} max={9000} dimmed={state === 'paused'} />
            <MiniGearBox gear={gear ?? '—'} dimmed={state === 'paused'} />
            <div style={{ textAlign: 'right' }}>
              <div style={{ ...t('mono'), fontFamily: FONT.display, fontSize: 24, color: state === 'paused' ? RP.inkDim : RP.ink, lineHeight: 1, fontWeight: 700 }}>{speed}</div>
              <div style={{ ...t('caption'), fontSize: 7, color: RP.gunmetal, letterSpacing: '0.12em' }}>KM/H</div>
            </div>
          </div>

          <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr 1fr', gap: 4, marginTop: 2 }}>
            <MiniStat label="LAST" value={lastLap ?? '—'} />
            <MiniStat label="BEST" value={bestLap ?? '—'} hi />
            <MiniStat label="POS"  value={pos ?? '—'} />
          </div>

          <div style={{ display: 'flex', justifyContent: 'space-between', ...t('mono'), fontSize: 10, marginTop: 2 }}>
            <span style={{ color: RP.ink }}>+{elapsed}</span>
            <span style={{ color: s.c }}>−{remaining}</span>
          </div>
          <div style={{ height: 2, background: RP.border, position: 'relative' }}>
            <div style={{ position: 'absolute', top: 0, left: 0, height: '100%', width: `${elapsedPct * 100}%`, background: s.c }} />
          </div>
        </>
      )}
    </div>
  );
}

function MiniRpmDial({ value, max, dimmed }: { value: number; max: number; dimmed: boolean }) {
  const SIZE = 38, R = 14, STROKE = 4;
  const CIRC = 2 * Math.PI * R;
  const ARC_FRAC = 0.75;
  const norm = Math.max(0, Math.min(1, value / max));
  const dash = CIRC * ARC_FRAC * norm;
  const inRed = norm > 0.85;
  const color = dimmed ? RP.gunmetal : (inRed ? RP.red : RP.green);
  return (
    <div style={{ position: 'relative', width: SIZE, height: SIZE }}>
      <svg width={SIZE} height={SIZE} style={{ transform: 'rotate(135deg)' }}>
        <circle cx={SIZE/2} cy={SIZE/2} r={R} fill="none" stroke={RP.border} strokeWidth={STROKE} strokeDasharray={`${CIRC * ARC_FRAC} ${CIRC}`} />
        <circle cx={SIZE/2} cy={SIZE/2} r={R} fill="none" stroke={color} strokeWidth={STROKE} strokeDasharray={`${dash} ${CIRC}`} />
      </svg>
      <div style={{ position: 'absolute', inset: 0, display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
        <span style={{ ...t('mono'), fontSize: 9, color: dimmed ? RP.gunmetal : RP.ink, lineHeight: 1 }}>
          {dimmed || !value ? '—' : Math.floor(value/1000)}
        </span>
      </div>
    </div>
  );
}

function MiniGearBox({ gear, dimmed }: { gear: string; dimmed: boolean }) {
  return (
    <div style={{
      width: 36, height: 36,
      border: `1.5px solid ${dimmed ? RP.border : RP.red}`,
      background: dimmed ? 'transparent' : 'rgba(225,6,0,0.05)',
      display: 'flex', alignItems: 'center', justifyContent: 'center',
    }}>
      <span style={{ fontFamily: FONT.display, fontSize: 22, color: dimmed ? RP.gunmetal : RP.ink, lineHeight: 1, fontWeight: 800 }}>{gear}</span>
    </div>
  );
}

function MiniStat({ label, value, hi }: { label: string; value: string; hi?: boolean }) {
  return (
    <div style={{ padding: '3px 5px', background: RP.base, border: `1px solid ${RP.border}` }}>
      <div style={{ ...t('caption'), fontSize: 7, color: RP.gunmetal, letterSpacing: '0.06em' }}>{label}</div>
      <div style={{ ...t('mono'), fontSize: 10, color: hi ? RP.red : RP.ink, lineHeight: 1.1, marginTop: 1 }}>{value}</div>
    </div>
  );
}

function mmToS(s: string): number {
  const [m, sec] = s.split(':').map(Number);
  return m * 60 + sec;
}

// ── Right column ────────────────────────────────────────────
function RightColumn() {
  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 16, minHeight: 0 }}>
      <Panel title="Revenue · Today" compact>
        <div style={{ padding: 16 }}>
          <div style={{ ...t('displayL'), color: RP.ink, fontFamily: FONT.display, fontSize: 36 }}>₹48,200</div>
          <div style={{ display: 'flex', alignItems: 'center', gap: 6, marginTop: 4 }}>
            <span style={{ color: RP.green, ...t('mono'), fontSize: 12 }}>↑ 12.4%</span>
            <span style={{ ...t('bodySm'), fontSize: 11, color: RP.inkDim }}>vs Mon avg</span>
          </div>
          <Sparkline />
          <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 8, marginTop: 12 }}>
            <Mini label="Sessions" value="₹38,200" />
            <Mini label="Cafe" value="₹6,800" muted />
            <Mini label="Memberships" value="₹3,200" />
            <Mini label="Refunds" value="−₹0" />
          </div>
        </div>
      </Panel>

      <Panel title="Service Health" trailing={<span style={{ ...t('caption'), fontSize: 9, color: RP.green }}>● 22/24 ONLINE</span>}>
        <div style={{ flex: 1, overflowY: 'hidden' }}>
          <SvcRow name="vms-orchestrator" host="srv-01"  state="green" uptime="14d 3h" />
          <SvcRow name="iracing-launcher" host="pod-01"  state="green" uptime="14:22" />
          <SvcRow name="telemetry-relay"  host="pod-04"  state="red"   uptime="—" issue="connection refused" />
          <SvcRow name="kiosk-frontend"   host="kiosk-1" state="green" uptime="2d 11h" />
          <SvcRow name="pos-printer"      host="pos-01"  state="amber" uptime="0:14" issue="paper low" />
          <SvcRow name="bono-strategist"  host="cloud"   state="green" uptime="36d" />
        </div>
      </Panel>
    </div>
  );
}

function Sparkline() {
  const pts = [12, 18, 14, 22, 28, 24, 32, 38, 30, 42, 48, 44];
  const max = Math.max(...pts);
  const w = 260, h = 38;
  const path = pts.map((v, i) => `${i === 0 ? 'M' : 'L'} ${(i / (pts.length - 1)) * w} ${h - (v / max) * h}`).join(' ');
  return (
    <svg width={w} height={h} style={{ marginTop: 10, display: 'block' }}>
      <defs>
        <linearGradient id="spark" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0%"   stopColor={RP.red} stopOpacity="0.3" />
          <stop offset="100%" stopColor={RP.red} stopOpacity="0" />
        </linearGradient>
      </defs>
      <path d={`${path} L ${w} ${h} L 0 ${h} Z`} fill="url(#spark)" />
      <path d={path} fill="none" stroke={RP.red} strokeWidth="1.5" />
    </svg>
  );
}

function Mini({ label, value, muted }: { label: string; value: string; muted?: boolean }) {
  return (
    <div style={{ borderLeft: `2px solid ${RP.border}`, paddingLeft: 8 }}>
      <div style={{ ...t('caption'), fontSize: 9, color: RP.gunmetal }}>{label}</div>
      <div style={{ ...t('mono'), fontSize: 13, color: muted ? RP.inkDim : RP.ink, marginTop: 2 }}>{value}</div>
    </div>
  );
}

function SvcRow({ name, host, state, uptime, issue }: {
  name: string; host: string; state: 'green' | 'amber' | 'red' | 'grey'; uptime: string; issue?: string;
}) {
  return (
    <div style={{ display: 'grid', gridTemplateColumns: '8px 1fr auto', gap: 10, alignItems: 'center', padding: '7px 12px', borderBottom: `1px solid ${RP.border}` }}>
      <StatusDot state={state} pulse={state === 'red'} />
      <div style={{ minWidth: 0 }}>
        <div style={{ ...t('mono'), fontSize: 11, color: RP.ink, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{name}</div>
        <div style={{ ...t('bodySm'), fontSize: 10, color: issue ? RP.red : RP.inkDim, marginTop: 1 }}>
          {host}{issue ? ` · ${issue}` : ''}
        </div>
      </div>
      <div style={{ ...t('mono'), fontSize: 10, color: RP.inkDim }}>{uptime}</div>
    </div>
  );
}

// ── Panel chrome ────────────────────────────────────────────
function Panel({ title, trailing, children, compact }: {
  title: string; trailing?: React.ReactNode; children: React.ReactNode; compact?: boolean;
}) {
  return (
    <div style={{
      background: RP.card, border: `1px solid ${RP.border}`,
      display: 'flex', flexDirection: 'column', minHeight: 0, flex: compact ? '0 0 auto' : 1,
    }}>
      <div style={{
        height: 32, padding: '0 12px', display: 'flex', alignItems: 'center', justifyContent: 'space-between',
        background: RP.base, borderBottom: `1px solid ${RP.border}`, flexShrink: 0,
      }}>
        <div style={{ ...t('caption'), color: RP.ink, fontSize: 10 }}>{title}</div>
        {trailing}
      </div>
      <div style={{ display: 'flex', flexDirection: 'column', flex: 1, minHeight: 0, overflow: 'hidden' }}>
        {children}
      </div>
    </div>
  );
}

// MOTION import needed for ActionButton transitions but unused at top level — silence linter
void MOTION;
