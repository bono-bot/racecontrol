'use client';

// Racing Point V2 Admin · Pod Drill-Down (artboard 05)
// Source: claude.ai/design handoff bundle hq8O1_qIHf27VSd3rvj51g — page-pod-detail.jsx
// Per-pod state, current driver/game, billing-elapsed/remaining, action buttons,
// live telemetry preview, service health, recent log tail, ops playbook.

import * as React from 'react';
import Link from 'next/link';
import { useParams } from 'next/navigation';
import { RP, FONT, t } from '@/components/v2/tokens';
import { Icon, StatusDot, StatusTile, ActionButton } from '@/components/v2/atomic';
import { AdminFrame, AdminPanel } from '@/components/v2/admin/AdminFrame';

type PodState = 'idle' | 'ready' | 'racing' | 'paused' | 'fault';

const PODS_LIST: { id: string; state: PodState }[] = [
  { id: 'POD-01', state: 'racing' },
  { id: 'POD-02', state: 'racing' },
  { id: 'POD-03', state: 'paused' },
  { id: 'POD-04', state: 'fault' },
  { id: 'POD-05', state: 'ready' },
  { id: 'POD-06', state: 'racing' },
  { id: 'POD-07', state: 'racing' },
  { id: 'POD-08', state: 'idle' },
];

export default function PodDetailPage() {
  const params = useParams<{ id: string }>();
  const podId = (params?.id || 'pod-04').toUpperCase();
  const meta = PODS_LIST.find(p => p.id === podId) || { id: podId, state: 'fault' as const };

  const stateColor = meta.state === 'fault' ? RP.red : meta.state === 'racing' ? RP.green : meta.state === 'paused' ? RP.amber : meta.state === 'ready' ? RP.blue : RP.gunmetal;
  const stateLabel = meta.state.toUpperCase();

  return (
    <AdminFrame
      active="pods"
      breadcrumbs={[
        { label: 'PODS', href: '/v2/admin/pods' },
        { label: podId },
      ]}
      rightStatus={
        <div style={{ display: 'flex', alignItems: 'center', gap: 12, ...t('mono'), fontSize: 11 }}>
          <StatusDot state={meta.state === 'fault' ? 'red' : meta.state === 'racing' ? 'green' : 'amber'} pulse={meta.state === 'fault'} />
          <span style={{ color: stateColor }}>{stateLabel}{meta.state === 'fault' ? ' · E-2204' : ''}</span>
        </div>
      }
    >
      <div style={{ flex: 1, display: 'grid', gridTemplateColumns: '220px 1fr 360px', minHeight: 0 }}>
        <PodSidebar activeId={podId} />
        <PodMain podId={podId} state={meta.state} />
        <PodRightPanel />
      </div>
    </AdminFrame>
  );
}

function PodSidebar({ activeId }: { activeId: string }) {
  return (
    <div style={{ background: RP.base, borderRight: `1px solid ${RP.border}`, padding: '16px 12px', overflow: 'auto' }}>
      <div style={{ ...t('caption'), color: RP.gunmetal, fontSize: 10, padding: '0 8px 12px' }}>8 PODS</div>
      {PODS_LIST.map(p => {
        const active = p.id === activeId;
        const dotState = p.state === 'fault' ? 'red' : p.state === 'racing' ? 'green' : p.state === 'paused' ? 'amber' : p.state === 'ready' ? 'green' : 'grey';
        return (
          <Link key={p.id} href={`/v2/admin/pods/${p.id.toLowerCase()}`} style={{ textDecoration: 'none' }}>
            <div style={{
              display: 'flex', alignItems: 'center', justifyContent: 'space-between',
              padding: '10px 10px', cursor: 'pointer', marginBottom: 2,
              background: active ? RP.cardHi : 'transparent',
              borderLeft: active ? `2px solid ${RP.red}` : '2px solid transparent',
            }}>
              <span style={{ ...t('mono'), fontSize: 12, color: active ? RP.ink : RP.inkDim }}>{p.id}</span>
              <StatusDot state={dotState} pulse={p.state === 'fault'} />
            </div>
          </Link>
        );
      })}
    </div>
  );
}

function PodMain({ podId, state }: { podId: string; state: PodState }) {
  return (
    <div style={{ padding: '20px 24px', overflow: 'auto', display: 'flex', flexDirection: 'column', gap: 18 }}>
      <div style={{ display: 'flex', alignItems: 'flex-end', justifyContent: 'space-between', gap: 16 }}>
        <div>
          <div style={{ ...t('caption'), color: state === 'fault' ? RP.red : RP.green, fontSize: 10 }}>
            ● {state.toUpperCase()}{state === 'fault' ? ' · E-2204 · 12 MIN AGO' : ''}
          </div>
          <div style={{ ...t('displayL'), fontFamily: FONT.display, fontSize: 56, color: RP.ink, marginTop: 4 }}>{podId}</div>
          <div style={{ ...t('body'), fontSize: 13, color: RP.inkDim, marginTop: 6, maxWidth: 720 }}>
            {state === 'fault'
              ? 'Pedals offline · USB enumeration failed. Failover engaged at 16:32. Driver re-seated to POD-08.'
              : state === 'racing'
                ? 'Active session — telemetry healthy, no faults logged in the last hour.'
                : state === 'paused'
                  ? 'Session paused by staff at 14:36. Wallet not burning.'
                  : state === 'ready'
                    ? 'Pod is set up and awaiting Start.'
                    : 'Pod available for next driver.'}
          </div>
        </div>
        <div style={{ display: 'flex', gap: 8, flexShrink: 0 }}>
          <ActionButton variant="secondary" icon={<Icon name="eye" size={14} />}>Blank Screen</ActionButton>
          <ActionButton variant="secondary" icon={<Icon name="refresh" size={14} />}>Restart Services</ActionButton>
          <ActionButton variant="danger" icon={<Icon name="power" size={14} />}>Hard Power Cycle</ActionButton>
          <ActionButton variant="primary" icon={<Icon name="play" size={14} />}>{state === 'fault' ? 'Mark Resolved' : 'Launch'}</ActionButton>
        </div>
      </div>

      <div style={{ display: 'grid', gridTemplateColumns: 'repeat(4, 1fr)', gap: 10 }}>
        <StatusTile label="Driver (failed-over)" value="S. Bhagat" sub="moved to POD-08 · 14:34" mono={false} accent={RP.amber} />
        <StatusTile label="Last Known Game" value="iRacing" sub="Suzuka · GT3 · lap 6/12" mono={false} accent={RP.gunmetal} />
        <StatusTile label="Billing Elapsed" value="12:44" sub="of 30:00 · refund pending" accent={RP.red} />
        <StatusTile label="Uptime" value="—" sub="last boot 14d 3h ago" accent={RP.gunmetal} />
      </div>

      <AdminPanel
        title="Telemetry · last 60s before fault"
        trailing={<span style={{ ...t('mono'), fontSize: 10, color: RP.inkDim }}>16:31:00 → 16:32:00</span>}
      >
        <div style={{ padding: 14 }}>
          <TelemetryStrip />
          <div style={{ display: 'grid', gridTemplateColumns: 'repeat(4, 1fr)', gap: 10, marginTop: 12 }}>
            <Stat label="THROTTLE" value="0%" sub="dropped 16:31:48" red />
            <Stat label="BRAKE" value="—" sub="signal lost" red />
            <Stat label="SPEED" value="142 km/h" sub="last sample" />
            <Stat label="GEAR" value="4" sub="last sample" />
          </div>
        </div>
      </AdminPanel>

      <AdminPanel title={`Services on ${podId} · 6 of 7 online`}>
        <div style={{ display: 'grid', gridTemplateColumns: 'repeat(2, 1fr)' }}>
          <SvcRow name="vms-pod-agent"      state="green" build="2026.05.02-a4f" up="14d 3h" />
          <SvcRow name="iracing-launcher"   state="green" build="2026.04.28-1c"  up="12:44" />
          <SvcRow name="telemetry-relay"    state="red"   build="2026.05.01-9d"  up="—" issue="connection refused" />
          <SvcRow name="usb-pedals-bridge"  state="red"   build="2026.04.20-77"  up="—" issue="device 0x044f unbound" />
          <SvcRow name="wheelbase-driver"   state="green" build="2026.04.20-77"  up="14d 3h" />
          <SvcRow name="display-controller" state="green" build="2026.05.02-a4f" up="14d 3h" />
          <SvcRow name="audio-relay"        state="amber" build="2026.04.10-3a"  up="14d 3h" issue="latency 88ms" />
        </div>
      </AdminPanel>
    </div>
  );
}

function TelemetryStrip() {
  const W = 940, H = 90;
  const throttle = synthFaultTrace(W, H, 'throttle');
  const brake = synthFaultTrace(W, H, 'brake');
  return (
    <svg width={W} height={H} style={{ display: 'block' }}>
      {[0, 0.5, 1].map(p => <line key={p} x1={0} y1={H * p} x2={W} y2={H * p} stroke={RP.border} strokeWidth="0.5" />)}
      <line x1={W * 0.85} y1={0} x2={W * 0.85} y2={H} stroke={RP.red} strokeWidth="1" strokeDasharray="3,3" />
      <text x={W * 0.85 + 6} y={12} fill={RP.red} fontSize="9" fontFamily={FONT.mono} fontWeight="700">FAULT</text>
      <path d={throttle} fill="none" stroke={RP.green} strokeWidth="1.5" />
      <path d={brake} fill="none" stroke={RP.red} strokeWidth="1.5" opacity="0.85" />
    </svg>
  );
}

function synthFaultTrace(W: number, H: number, kind: 'throttle' | 'brake'): string {
  const N = 120;
  const pts: [number, number][] = [];
  for (let i = 0; i < N; i++) {
    const x = (i / (N - 1)) * W;
    const tt = i / N;
    let v: number;
    if (tt > 0.85) v = 0;
    else if (kind === 'throttle') v = 0.7 + Math.sin(tt * Math.PI * 8) * 0.25;
    else v = Math.max(0, Math.sin(tt * Math.PI * 8 - 0.5) - 0.4) * 1.6;
    v = Math.max(0, Math.min(1, v));
    pts.push([x, H - v * H * 0.85 - 4]);
  }
  return pts.map((p, i) => `${i === 0 ? 'M' : 'L'} ${p[0].toFixed(1)} ${p[1].toFixed(1)}`).join(' ');
}

function Stat({ label, value, sub, red }: { label: string; value: string; sub: string; red?: boolean }) {
  return (
    <div style={{ borderLeft: `2px solid ${red ? RP.red : RP.border}`, paddingLeft: 10 }}>
      <div style={{ ...t('caption'), fontSize: 9, color: RP.gunmetal }}>{label}</div>
      <div style={{ ...t('mono'), fontSize: 16, color: red ? RP.red : RP.ink, marginTop: 2 }}>{value}</div>
      <div style={{ ...t('bodySm'), fontSize: 10, color: RP.inkDim, marginTop: 2 }}>{sub}</div>
    </div>
  );
}

function SvcRow({ name, state, build, up, issue }: {
  name: string;
  state: 'green' | 'amber' | 'red' | 'grey';
  build: string;
  up: string;
  issue?: string;
}) {
  return (
    <div style={{ display: 'grid', gridTemplateColumns: '8px 1fr auto', gap: 10, alignItems: 'center', padding: '10px 14px', borderBottom: `1px solid ${RP.border}`, borderRight: `1px solid ${RP.border}` }}>
      <StatusDot state={state} pulse={state === 'red'} />
      <div style={{ minWidth: 0 }}>
        <div style={{ ...t('mono'), fontSize: 11, color: RP.ink }}>{name}</div>
        <div style={{ ...t('mono'), fontSize: 10, color: issue ? RP.red : RP.inkDim, marginTop: 1 }}>
          {build} · {up}{issue ? ` · ${issue}` : ''}
        </div>
      </div>
      <ActionButton variant="ghost" size="sm" icon={<Icon name="refresh" size={11} />}>Restart</ActionButton>
    </div>
  );
}

function PodRightPanel() {
  const LOGS: { lvl: 'ERR' | 'WARN' | 'INFO'; time: string; src: string; msg: string }[] = [
    { lvl: 'ERR',  time: '16:32:04', src: 'usb-pedals', msg: 'device 0x044f unbound · re-init failed' },
    { lvl: 'ERR',  time: '16:32:03', src: 'usb-pedals', msg: 'EIO on read endpoint · reattempt 3/3' },
    { lvl: 'WARN', time: '16:32:01', src: 'tlm-relay',  msg: 'no samples received in 1200ms' },
    { lvl: 'ERR',  time: '16:32:00', src: 'usb-pedals', msg: 'pedal pressure stuck at 0' },
    { lvl: 'WARN', time: '16:31:54', src: 'tlm-relay',  msg: 'sample rate dropped to 30Hz (target 60)' },
    { lvl: 'INFO', time: '16:31:48', src: 'iracing',    msg: 'lap 6 complete · 1:35.821' },
    { lvl: 'INFO', time: '16:31:32', src: 'tlm-relay',  msg: 'reconnect to relay-master OK' },
    { lvl: 'WARN', time: '16:31:30', src: 'tlm-relay',  msg: 'reconnect attempt to relay-master' },
    { lvl: 'INFO', time: '16:30:12', src: 'iracing',    msg: 'lap 5 complete · 1:36.114' },
    { lvl: 'INFO', time: '16:28:58', src: 'iracing',    msg: 'lap 4 complete · 1:36.402' },
  ];
  const C: Record<string, string> = { ERR: RP.red, WARN: RP.amber, INFO: RP.inkDim };
  return (
    <div style={{ background: RP.base, borderLeft: `1px solid ${RP.border}`, display: 'flex', flexDirection: 'column', minHeight: 0 }}>
      <div style={{ padding: '16px 16px 10px', borderBottom: `1px solid ${RP.border}` }}>
        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
          <div style={{ ...t('caption'), color: RP.ink, fontSize: 10 }}>LOG TAIL · POD-04</div>
          <div style={{ display: 'flex', gap: 4 }}>
            <ActionButton variant="ghost" size="sm">ERR</ActionButton>
            <ActionButton variant="ghost" size="sm">WARN</ActionButton>
            <ActionButton variant="ghost" size="sm">INFO</ActionButton>
          </div>
        </div>
      </div>
      <div style={{ flex: 1, overflowY: 'auto', padding: '8px 0' }}>
        {LOGS.map((l, i) => (
          <div key={i} style={{ padding: '6px 16px', display: 'grid', gridTemplateColumns: '40px 60px 70px 1fr', gap: 8, ...t('mono'), fontSize: 10, lineHeight: 1.5 }}>
            <span style={{ color: C[l.lvl], fontWeight: 700 }}>{l.lvl}</span>
            <span style={{ color: RP.gunmetal }}>{l.time}</span>
            <span style={{ color: RP.inkDim }}>{l.src}</span>
            <span style={{ color: l.lvl === 'ERR' ? RP.red : RP.ink }}>{l.msg}</span>
          </div>
        ))}
      </div>
      <div style={{ padding: 14, borderTop: `1px solid ${RP.border}`, ...t('mono'), fontSize: 10, color: RP.gunmetal }}>
        <div style={{ marginBottom: 6 }}>OPS PLAYBOOK · E-2204</div>
        <div style={{ ...t('body'), fontSize: 11, color: RP.inkDim, lineHeight: 1.5 }}>
          1. Verify pedal USB seated · 2. Restart usb-pedals-bridge · 3. If unresolved, hard power-cycle pod · 4. Refund issued automatically.
        </div>
      </div>
    </div>
  );
}
