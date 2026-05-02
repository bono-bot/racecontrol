'use client';

// Racing Point V2 Admin · Feature Flag Hub (artboard 06)
// Source: claude.ai/design handoff bundle hq8O1_qIHf27VSd3rvj51g — page-flags.jsx (299 lines)
// Demonstrates the four-layer flag pattern: route / section / action / endpoint.
// Off = invisible, never disabled. UI shell only — backend feature-flag service
// (Phase 449-452) lands later; flag state here is illustrative.

import * as React from 'react';
import { RP, RADIUS, MOTION, t } from '@/components/v2/tokens';
import { Icon, StatusDot, ActionButton } from '@/components/v2/atomic';
import { AdminFrame } from '@/components/v2/admin/AdminFrame';

type FlagLayer = 'route' | 'section' | 'action' | 'endpoint';

const LAYER_META: Record<FlagLayer, { c: string; label: string }> = {
  route:    { c: RP.red,   label: 'ROUTE' },
  section:  { c: RP.amber, label: 'SECTION' },
  action:   { c: RP.green, label: 'ACTION' },
  endpoint: { c: RP.blue,  label: 'API' },
};

interface ScopeRow {
  name: string;
  count: number;
  active?: boolean;
}

const SCOPES: ScopeRow[] = [
  { name: 'All Flags', count: 47, active: true },
  { name: 'Cockpit',   count: 8 },
  { name: 'Pods',      count: 6 },
  { name: 'Customers', count: 9 },
  { name: 'POS',       count: 7 },
  { name: 'Kiosk',     count: 5 },
  { name: 'PWA',       count: 8 },
  { name: 'Billing',   count: 4 },
];

export default function FlagsHubPage() {
  return (
    <AdminFrame
      active="flags"
      breadcrumbs={[{ label: 'SETTINGS' }, { label: 'FEATURE FLAGS' }]}
      rightStatus={
        <div style={{ display: 'flex', alignItems: 'center', gap: 10, ...t('mono'), fontSize: 11, color: RP.inkDim }}>
          <span style={{ display: 'flex', gap: 6, alignItems: 'center' }}>
            <StatusDot state="amber" /> 2 PENDING
          </span>
          <ActionButton variant="secondary" size="sm">Discard</ActionButton>
          <ActionButton variant="primary" size="sm" icon={<Icon name="check" size={12} />}>Apply 2 Changes</ActionButton>
        </div>
      }
    >
      <div style={{ flex: 1, display: 'grid', gridTemplateColumns: '240px 1fr 360px', minHeight: 0 }}>
        <FlagsSidebar />
        <FlagsMain />
        <FlagsImpact />
      </div>
    </AdminFrame>
  );
}

function FlagsSidebar() {
  return (
    <div style={{ background: RP.base, borderRight: `1px solid ${RP.border}`, padding: '20px 12px', display: 'flex', flexDirection: 'column', gap: 4 }}>
      <div style={{ ...t('caption'), color: RP.gunmetal, fontSize: 10, padding: '0 8px 12px' }}>SCOPES</div>
      {SCOPES.map(s => (
        <div key={s.name} style={{
          display: 'flex', justifyContent: 'space-between', alignItems: 'center',
          padding: '8px 10px', borderRadius: RADIUS.sm, cursor: 'pointer',
          background: s.active ? RP.cardHi : 'transparent',
          borderLeft: s.active ? `2px solid ${RP.red}` : '2px solid transparent',
        }}>
          <span style={{ ...t('body'), fontSize: 12, color: s.active ? RP.ink : RP.inkDim }}>{s.name}</span>
          <span style={{ ...t('mono'), fontSize: 10, color: RP.gunmetal }}>{s.count}</span>
        </div>
      ))}
      <div style={{ flex: 1 }} />
      <div style={{ background: RP.card, border: `1px solid ${RP.border}`, padding: 12, marginTop: 16 }}>
        <div style={{ ...t('caption'), color: RP.amber, fontSize: 9 }}>ENVIRONMENT</div>
        <div style={{ ...t('h2'), fontSize: 14, color: RP.ink, marginTop: 4 }}>PRODUCTION</div>
        <div style={{ ...t('bodySm'), fontSize: 10, color: RP.inkDim, marginTop: 4 }}>Bandlaguda venue · live</div>
        <div style={{ display: 'flex', gap: 6, marginTop: 8 }}>
          <ActionButton variant="ghost" size="sm">Switch</ActionButton>
        </div>
      </div>
    </div>
  );
}

function FlagsMain() {
  return (
    <div style={{ padding: '20px 24px', overflow: 'auto', display: 'flex', flexDirection: 'column', gap: 20 }}>
      <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
        <div>
          <div style={{ ...t('h1'), color: RP.ink, fontSize: 24 }}>Feature Flags</div>
          <div style={{ ...t('body'), fontSize: 12, color: RP.inkDim, marginTop: 4 }}>
            Four-layer modular pattern: route / section / action / endpoint. Off = invisible, never disabled.
          </div>
        </div>
        <div style={{ display: 'flex', gap: 8 }}>
          <ActionButton variant="ghost" size="sm" icon={<Icon name="search" size={12} />}>Search</ActionButton>
          <ActionButton variant="ghost" size="sm" icon={<Icon name="filter" size={12} />}>Filter</ActionButton>
          <ActionButton variant="secondary" size="sm" icon={<Icon name="download" size={12} />}>Export YAML</ActionButton>
        </div>
      </div>

      <FlagGroup title="Cockpit · Admin landing" desc="Operator dashboard sections and live data feeds.">
        <FlagItem layer="section" key_="cockpit.alerts.live"   desc="Live alert feed panel"                    on rollout="100%" />
        <FlagItem layer="section" key_="cockpit.bono.summary"  desc="Bono nightly profit summary tile"          on rollout="100%" />
        <FlagItem layer="action"  key_="cockpit.pod.kill"      desc="Force-kill pod button (destructive)"       on rollout="100%" warn />
        <FlagItem layer="route"   key_="cockpit.heatmap"       desc="Venue heatmap page (v2.1)"                 pending="enable" />
      </FlagGroup>

      <FlagGroup title="PWA · Customer portal" desc="Customer-facing surfaces gated for v2.0 launch.">
        <FlagItem layer="route"   key_="pwa.lap_compare"                    desc="My Laps + comparison page"           on rollout="100%" />
        <FlagItem layer="route"   key_="pwa.driver_class"                   desc="Class & progression page"            on rollout="100%" />
        <FlagItem layer="section" key_="pwa.compare.telemetry_overlay"      desc="Throttle/brake trace overlay"        on rollout="100%" />
        <FlagItem layer="action"  key_="pwa.share.lap"                      desc="Share lap via WhatsApp"              pending="enable" />
        <FlagItem layer="route"   key_="pwa.wallet.topup"                   desc="Customer-facing wallet top-up"       note="v2.1 — held" />
      </FlagGroup>

      <FlagGroup title="POS · Counter ops" desc="Counter-staff workstation features.">
        <FlagItem layer="section"  key_="pos.lookup.recent"        desc="Recent customers strip"             on rollout="100%" />
        <FlagItem layer="action"   key_="pos.intake.waiver_cam"    desc="Photo + waiver camera capture"      on rollout="100%" />
        <FlagItem layer="endpoint" key_="pos.api.refund"           desc="POST /pos/refund (audit-logged)"    on rollout="100%" />
      </FlagGroup>

      <FlagGroup title="Held — v2.1 features" desc="Designed but not shipping. Surfaces invisible until flipped." muted>
        <FlagItem layer="route" key_="cafe.kot"               desc="Cafe KOT dispatch view"   note="held" />
        <FlagItem layer="route" key_="bookings.reservations"  desc="Bookings / reservations"  note="held" />
        <FlagItem layer="route" key_="loyalty.points"         desc="Loyalty points & merch"   note="held" />
        <FlagItem layer="route" key_="cms.marketing"          desc="Marketing CMS"            note="held" />
      </FlagGroup>
    </div>
  );
}

function FlagGroup({ title, desc, children, muted }: { title: string; desc: string; children: React.ReactNode; muted?: boolean }) {
  return (
    <div style={{ opacity: muted ? 0.65 : 1 }}>
      <div style={{ marginBottom: 10 }}>
        <div style={{ ...t('h2'), fontSize: 13, color: RP.ink }}>{title}</div>
        <div style={{ ...t('bodySm'), fontSize: 11, color: RP.inkDim, marginTop: 2 }}>{desc}</div>
      </div>
      <div style={{ background: RP.card, border: `1px solid ${RP.border}` }}>
        <div style={{
          display: 'grid',
          gridTemplateColumns: '90px 280px 1fr 100px 100px 60px',
          padding: '8px 14px', background: RP.base, borderBottom: `1px solid ${RP.border}`,
          ...t('caption'), color: RP.gunmetal, fontSize: 9, gap: 12,
        }}>
          <div>LAYER</div><div>KEY</div><div>DESCRIPTION</div><div>ROLLOUT</div><div>STATE</div><div></div>
        </div>
        {children}
      </div>
    </div>
  );
}

function FlagItem({ layer, key_, desc, on, rollout, pending, note, warn }: {
  layer: FlagLayer;
  key_: string;
  desc: string;
  on?: boolean;
  rollout?: string;
  pending?: string;
  note?: string;
  warn?: boolean;
}) {
  const l = LAYER_META[layer];
  return (
    <div style={{
      display: 'grid',
      gridTemplateColumns: '90px 280px 1fr 100px 100px 60px',
      padding: '12px 14px', borderBottom: `1px solid ${RP.border}`, gap: 12, alignItems: 'center',
      background: pending ? 'rgba(255,176,0,0.04)' : 'transparent',
    }}>
      <div>
        <span style={{ ...t('caption'), fontSize: 9, color: l.c, padding: '2px 6px', border: `1px solid ${l.c}` }}>{l.label}</span>
      </div>
      <div style={{ ...t('mono'), fontSize: 11, color: RP.ink }}>{key_}</div>
      <div style={{ ...t('body'), fontSize: 12, color: RP.inkDim, display: 'flex', alignItems: 'center', gap: 8 }}>
        {desc}
        {warn && <Icon name="bell" size={11} color={RP.amber} />}
        {note && <span style={{ ...t('caption'), fontSize: 8, color: RP.gunmetal, padding: '1px 5px', border: `1px solid ${RP.border}` }}>{note.toUpperCase()}</span>}
      </div>
      <div style={{ ...t('mono'), fontSize: 11, color: on ? RP.ink : RP.gunmetal }}>
        {on ? rollout : '—'}
      </div>
      <div>
        {pending ? (
          <span style={{ ...t('caption'), fontSize: 9, color: RP.amber, padding: '2px 6px', border: `1px solid ${RP.amber}` }}>PEND {pending.toUpperCase()}</span>
        ) : (
          <span style={{ ...t('caption'), fontSize: 9, color: on ? RP.green : RP.gunmetal }}>● {on ? 'ON' : 'OFF'}</span>
        )}
      </div>
      <FlagSwitch on={!!on} pending={!!pending} />
    </div>
  );
}

function FlagSwitch({ on, pending }: { on: boolean; pending: boolean }) {
  return (
    <div style={{
      width: 36, height: 18, borderRadius: 9, background: on ? RP.green : RP.borderHi,
      position: 'relative', flexShrink: 0, cursor: 'pointer',
      border: pending ? `1px solid ${RP.amber}` : 'none',
    }}>
      <div style={{
        position: 'absolute', top: 1, left: on ? 19 : 1, width: 14, height: 14,
        borderRadius: 7, background: '#fff', transition: `left ${MOTION.fast}`,
      }} />
    </div>
  );
}

function FlagsImpact() {
  return (
    <div style={{ background: RP.base, borderLeft: `1px solid ${RP.border}`, display: 'flex', flexDirection: 'column', minHeight: 0 }}>
      <div style={{ padding: '20px 20px 16px', borderBottom: `1px solid ${RP.border}` }}>
        <div style={{ ...t('caption'), color: RP.amber, fontSize: 10 }}>● PENDING IMPACT</div>
        <div style={{ ...t('h2'), fontSize: 14, color: RP.ink, marginTop: 6 }}>2 changes staged</div>
        <div style={{ ...t('bodySm'), fontSize: 11, color: RP.inkDim, marginTop: 4 }}>
          Review then Apply. Rollback within 30s after publish.
        </div>
      </div>

      <div style={{ flex: 1, overflowY: 'auto' }}>
        <ImpactRow
          key_="cockpit.heatmap"
          delta="OFF → ON"
          surfaces={['Admin > Cockpit > Heatmap (route)', 'GET /api/cockpit/heatmap']}
          users="James, Bono, Captain Uday"
        />
        <ImpactRow
          key_="pwa.share.lap"
          delta="OFF → ON"
          surfaces={['PWA > Lap Compare > Share button', 'POST /api/pwa/lap/share']}
          users="All PWA customers · ~340"
        />
      </div>

      <div style={{ padding: 16, borderTop: `1px solid ${RP.border}` }}>
        <div style={{ ...t('caption'), color: RP.gunmetal, fontSize: 9, marginBottom: 8 }}>WHEN APPLIED</div>
        <div style={{ ...t('mono'), fontSize: 11, color: RP.inkDim, lineHeight: 1.7 }}>
          <div>1. config emitted to Redis</div>
          <div>2. clients re-render in &lt;500ms</div>
          <div>3. audit log @ flags.audit.v2</div>
        </div>
        <div style={{ display: 'flex', gap: 8, marginTop: 14 }}>
          <ActionButton variant="secondary" size="md" fullWidth>Discard</ActionButton>
          <ActionButton variant="primary" size="md" fullWidth icon={<Icon name="check" size={12} />}>Apply</ActionButton>
        </div>
      </div>
    </div>
  );
}

function ImpactRow({ key_, delta, surfaces, users }: { key_: string; delta: string; surfaces: string[]; users: string }) {
  return (
    <div style={{ padding: '14px 20px', borderBottom: `1px solid ${RP.border}` }}>
      <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: 8 }}>
        <div style={{ ...t('mono'), fontSize: 11, color: RP.ink }}>{key_}</div>
        <div style={{ ...t('caption'), fontSize: 9, color: RP.amber, padding: '2px 6px', border: `1px solid ${RP.amber}` }}>{delta}</div>
      </div>
      <div style={{ ...t('caption'), fontSize: 9, color: RP.gunmetal, marginBottom: 4 }}>SURFACES AFFECTED</div>
      {surfaces.map((s, i) => (
        <div key={i} style={{ ...t('mono'), fontSize: 10, color: RP.inkDim, lineHeight: 1.5 }}>· {s}</div>
      ))}
      <div style={{ ...t('caption'), fontSize: 9, color: RP.gunmetal, marginTop: 8, marginBottom: 2 }}>WHO SEES THIS</div>
      <div style={{ ...t('bodySm'), fontSize: 11, color: RP.ink }}>{users}</div>
    </div>
  );
}
