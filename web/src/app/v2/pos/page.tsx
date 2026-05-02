'use client';

// Racing Point V2 POS · Customer Lookup → Detail
// Source: claude.ai/design handoff bundle hq8O1_qIHf27VSd3rvj51g — page-pos.jsx (349 lines)
// Counter-staff workstation; touch-first; 48px primary targets; lower density than admin.
// 2-col layout: lookup column (search + 6 results) | customer detail (focused customer + sessions table).

import * as React from 'react';
import { RP, FONT, RADIUS, t } from '@/components/v2/tokens';
import { Icon, StatusDot, ActionButton, DriverClassBadge } from '@/components/v2/atomic';

interface CustomerResult {
  id: number;
  name: string;
  phone: string;
  last: string;
  sessions: number;
  cls: 'Rookie' | 'Apex' | 'Podium' | 'Champion';
  focused?: boolean;
}

const RESULTS: CustomerResult[] = [
  { id: 1, name: 'Uday Singh',         phone: '+91 98••• ••421', last: '2h ago',    sessions: 142, cls: 'Champion', focused: true },
  { id: 2, name: 'Vishal Kaushik',     phone: '+91 99••• ••103', last: 'Today',     sessions: 38,  cls: 'Podium' },
  { id: 3, name: 'Aruna Singh',        phone: '+91 98••• ••812', last: 'Today',     sessions: 22,  cls: 'Apex' },
  { id: 4, name: 'Shashwat Bhagat',    phone: '+91 90••• ••741', last: 'Yesterday', sessions: 14,  cls: 'Apex' },
  { id: 5, name: 'Ramgopal Bangad',    phone: '+91 98••• ••092', last: '3d ago',    sessions: 4,   cls: 'Rookie' },
  { id: 6, name: 'Karan Mehta',        phone: '+91 70••• ••558', last: '1w ago',    sessions: 31,  cls: 'Apex' },
];

export default function POSPage() {
  return (
    <div style={{
      width: '100vw', height: '100vh', background: RP.asphalt, color: RP.ink,
      fontFamily: FONT.body, display: 'flex', flexDirection: 'column',
      overflow: 'hidden', boxSizing: 'border-box',
    }}>
      <POSTopBar />
      <div style={{ flex: 1, display: 'grid', gridTemplateColumns: '380px 1fr', minHeight: 0 }}>
        <LookupColumn />
        <CustomerDetail />
      </div>
    </div>
  );
}

function POSTopBar() {
  return (
    <div style={{
      height: 56, background: RP.base, borderBottom: `2px solid ${RP.red}`,
      display: 'flex', alignItems: 'center', padding: '0 20px', gap: 20, flexShrink: 0,
    }}>
      <div style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
        <div style={{ width: 28, height: 28, background: RP.red, clipPath: 'polygon(0 0, 100% 0, 75% 100%, 0% 100%)' }} />
        <div style={{ ...t('h2'), color: RP.ink, fontSize: 17, letterSpacing: '0.06em' }}>RACING POINT</div>
        <div style={{ ...t('caption'), color: '#000', background: RP.amber, fontSize: 10, padding: '3px 8px' }}>POS · COUNTER 1</div>
      </div>
      <div style={{ width: 1, height: 28, background: RP.border }} />
      <NavTab label="Lookup"          icon="search" active />
      <NavTab label="Active Sessions" icon="zap"    badge="6" />
      <NavTab label="Quick Issue"     icon="plus" />
      <NavTab label="Intake"          icon="user" />
      <div style={{ flex: 1 }} />
      <HwStatus icon="printer" label="PRINTER"     state="green" sub="Roll 88%" />
      <HwStatus icon="drawer"  label="CASH DRAWER" state="green" sub="Closed" />
      <HwStatus icon="camera"  label="WAIVER CAM"  state="green" sub="Ready" />
      <div style={{ width: 1, height: 28, background: RP.border }} />
      <div style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
        <div style={{ width: 32, height: 32, borderRadius: '50%', background: RP.cardHi, ...t('caption'), color: RP.ink, fontSize: 11, display: 'flex', alignItems: 'center', justifyContent: 'center' }}>SK</div>
        <div>
          <div style={{ ...t('bodyB'), fontSize: 12, color: RP.ink }}>Suresh K.</div>
          <div style={{ ...t('mono'), fontSize: 9, color: RP.inkDim }}>Shift · 14:00–22:00</div>
        </div>
      </div>
    </div>
  );
}

function NavTab({ label, icon, active, badge }: { label: string; icon: string; active?: boolean; badge?: string }) {
  return (
    <button style={{
      height: 38, padding: '0 14px', background: active ? RP.cardHi : 'transparent',
      border: `1px solid ${active ? RP.borderHi : 'transparent'}`,
      borderBottom: active ? `2px solid ${RP.red}` : `1px solid transparent`,
      color: active ? RP.ink : RP.inkDim, ...t('caption'), fontSize: 11,
      display: 'flex', alignItems: 'center', gap: 8, cursor: 'pointer',
    }}>
      <Icon name={icon} size={14} />
      {label}
      {badge && <span style={{ background: RP.red, color: '#fff', borderRadius: 8, padding: '1px 6px', ...t('mono'), fontSize: 10 }}>{badge}</span>}
    </button>
  );
}

function HwStatus({ icon, label, state, sub }: { icon: string; label: string; state: 'green' | 'amber' | 'red'; sub: string }) {
  const c = state === 'green' ? RP.green : state === 'amber' ? RP.amber : RP.red;
  return (
    <div style={{ display: 'flex', alignItems: 'center', gap: 6, paddingRight: 4 }}>
      <Icon name={icon} size={14} color={c} />
      <StatusDot state={state} />
      <div>
        <div style={{ ...t('caption'), fontSize: 9, color: RP.inkDim }}>{label}</div>
        <div style={{ ...t('mono'), fontSize: 10, color: c }}>{sub}</div>
      </div>
    </div>
  );
}

function LookupColumn() {
  return (
    <div style={{ background: RP.base, borderRight: `1px solid ${RP.border}`, display: 'flex', flexDirection: 'column', minHeight: 0 }}>
      <div style={{ padding: 16, borderBottom: `1px solid ${RP.border}` }}>
        <div style={{ ...t('caption'), color: RP.gunmetal, marginBottom: 10 }}>Customer Lookup</div>
        <div style={{
          display: 'flex', alignItems: 'center', gap: 10,
          height: 48, padding: '0 14px', background: RP.card, border: `1px solid ${RP.borderHi}`,
          borderRadius: RADIUS.md,
        }}>
          <Icon name="search" size={18} color={RP.inkDim} />
          <input
            placeholder="Phone, name, or scan QR..."
            defaultValue="uday"
            style={{
              flex: 1, background: 'transparent', border: 'none', outline: 'none',
              color: RP.ink, ...t('body'), fontSize: 15,
            }}
          />
          <span style={{ ...t('caption'), fontSize: 9, color: RP.gunmetal, padding: '2px 6px', border: `1px solid ${RP.border}`, borderRadius: 3 }}>⌘K</span>
        </div>
        <div style={{ display: 'flex', gap: 6, marginTop: 10 }}>
          <FilterChip>All</FilterChip>
          <FilterChip active>Active</FilterChip>
          <FilterChip>Members</FilterChip>
          <FilterChip>Walk-ins</FilterChip>
        </div>
      </div>
      <div style={{ flex: 1, overflowY: 'auto' }}>
        <div style={{ ...t('caption'), color: RP.gunmetal, padding: '12px 16px 6px', fontSize: 9 }}>6 RESULTS · MATCHING &quot;UDAY&quot;</div>
        {RESULTS.map(r => <ResultRow key={r.id} {...r} />)}
      </div>
      <div style={{ padding: 12, borderTop: `1px solid ${RP.border}` }}>
        <ActionButton variant="primary" size="lg" fullWidth icon={<Icon name="plus" size={16} />}>
          New Customer Intake
        </ActionButton>
      </div>
    </div>
  );
}

function FilterChip({ children, active }: { children: React.ReactNode; active?: boolean }) {
  return (
    <button style={{
      height: 28, padding: '0 12px',
      background: active ? RP.cardHi : 'transparent',
      border: `1px solid ${active ? RP.borderHi : RP.border}`,
      color: active ? RP.ink : RP.inkDim,
      ...t('caption'), fontSize: 10, cursor: 'pointer',
      borderRadius: RADIUS.pill,
    }}>{children}</button>
  );
}

function ResultRow({ name, phone, last, sessions, cls, focused }: CustomerResult) {
  return (
    <div style={{
      display: 'grid', gridTemplateColumns: '40px 1fr auto', gap: 10, alignItems: 'center',
      padding: '12px 16px',
      background: focused ? RP.cardHi : 'transparent',
      borderLeft: focused ? `3px solid ${RP.red}` : `3px solid transparent`,
      borderBottom: `1px solid ${RP.border}`, cursor: 'pointer',
    }}>
      <div style={{
        width: 40, height: 40, borderRadius: '50%', background: RP.card,
        border: `1px solid ${RP.borderHi}`,
        ...t('caption'), color: RP.ink, fontSize: 11,
        display: 'flex', alignItems: 'center', justifyContent: 'center',
      }}>{name.split(' ').map(s => s[0]).join('').slice(0, 2)}</div>
      <div style={{ minWidth: 0 }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
          <div style={{ ...t('bodyB'), fontSize: 13, color: RP.ink }}>{name}</div>
          <DriverClassBadge tier={cls} size="sm" />
        </div>
        <div style={{ ...t('mono'), fontSize: 10, color: RP.inkDim, marginTop: 2 }}>{phone}</div>
      </div>
      <div style={{ textAlign: 'right' }}>
        <div style={{ ...t('mono'), fontSize: 11, color: RP.ink }}>{sessions} ses</div>
        <div style={{ ...t('caption'), fontSize: 9, color: RP.inkDim, marginTop: 2 }}>{last}</div>
      </div>
    </div>
  );
}

function CustomerDetail() {
  return (
    <div style={{ display: 'flex', flexDirection: 'column', minHeight: 0, overflow: 'hidden' }}>
      <div style={{
        padding: '20px 28px', borderBottom: `1px solid ${RP.border}`, background: RP.base,
        display: 'flex', gap: 20, alignItems: 'flex-start',
      }}>
        <div style={{
          width: 88, height: 88, background: RP.card, border: `1px solid ${RP.borderHi}`,
          borderRadius: RADIUS.sm, position: 'relative', overflow: 'hidden', flexShrink: 0,
        }}>
          <div style={{
            position: 'absolute', inset: 0, background: `linear-gradient(135deg, ${RP.cardHi} 0%, ${RP.card} 100%)`,
          }} />
          <div style={{ position: 'absolute', inset: 0, display: 'flex', alignItems: 'center', justifyContent: 'center', ...t('displayM'), color: RP.gunmetal, fontFamily: FONT.display }}>US</div>
          <div style={{ position: 'absolute', bottom: 0, right: 0, padding: 3, background: RP.green, borderRadius: '50% 0 0 0', width: 14, height: 14 }} />
        </div>
        <div style={{ flex: 1 }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: 10, marginBottom: 6 }}>
            <div style={{ ...t('caption'), color: RP.gunmetal, fontSize: 10 }}>CUSTOMER · #4291</div>
            <div style={{ ...t('caption'), color: RP.green, fontSize: 9, padding: '2px 6px', border: `1px solid ${RP.green}` }}>● ON-PREMISE</div>
            <div style={{ ...t('caption'), color: RP.amber, fontSize: 9, padding: '2px 6px', border: `1px solid ${RP.amber}` }}>MEMBER · GOLD</div>
          </div>
          <div style={{ ...t('displayM'), color: RP.ink, fontFamily: FONT.display, fontSize: 30 }}>Uday Singh</div>
          <div style={{ display: 'flex', gap: 20, marginTop: 10, ...t('mono'), fontSize: 12, color: RP.inkDim, alignItems: 'center' }}>
            <span style={{ display: 'inline-flex', alignItems: 'center', gap: 4 }}><Icon name="phone" size={11} /> +91 98••• ••421</span>
            <span style={{ display: 'inline-flex', alignItems: 'center', gap: 4 }}><Icon name="mail"  size={11} /> u•••gh@ra•••oint.in</span>
            <span style={{ display: 'inline-flex', alignItems: 'center', gap: 4 }}><Icon name="home"  size={11} /> Bandlaguda · Hyderabad</span>
          </div>
          <div style={{ display: 'flex', alignItems: 'center', gap: 14, marginTop: 14 }}>
            <DriverClassBadge tier="Champion" />
            <div style={{ ...t('mono'), fontSize: 11, color: RP.inkDim }}>
              Member since <span style={{ color: RP.ink }}>2024-03-12</span> · Last visit <span style={{ color: RP.ink }}>2h ago</span>
            </div>
          </div>
        </div>
        <div style={{ display: 'flex', gap: 8, alignItems: 'flex-start' }}>
          <ActionButton variant="secondary" size="lg" icon={<Icon name="mail" size={14} />}>SMS</ActionButton>
          <ActionButton variant="primary" size="lg" icon={<Icon name="zap" size={14} />}>Quick Issue</ActionButton>
        </div>
      </div>

      <div style={{ display: 'grid', gridTemplateColumns: 'repeat(5, 1fr)', borderBottom: `1px solid ${RP.border}`, background: RP.asphalt }}>
        <BigStat label="Total Sessions" value="142"     sub="+8 this month" />
        <BigStat label="Wallet Balance" value="₹2,400"  sub="3 packages active" accent={RP.green} />
        <BigStat label="Best Lap"       value="2:00.630" sub="Spa · iRacing" mono accent={RP.red} />
        <BigStat label="Lifetime Spend" value="₹86,400" sub="₹608/session avg" />
        <BigStat label="Class Progress" value="—"        sub="Champion · max tier" custom={<ChampionBar />} />
      </div>

      <div style={{ display: 'flex', gap: 0, borderBottom: `1px solid ${RP.border}`, padding: '0 28px', background: RP.base }}>
        <DetailTab label="Sessions"          count="142" active />
        <DetailTab label="Wallet & Packages" count="3" />
        <DetailTab label="Notes"             count="2" />
        <DetailTab label="Waivers"           count="1" />
        <DetailTab label="Activity" />
      </div>

      <div style={{ flex: 1, padding: '16px 28px', display: 'flex', flexDirection: 'column', minHeight: 0, gap: 12 }}>
        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
          <div style={{ display: 'flex', alignItems: 'baseline', gap: 12 }}>
            <div style={{ ...t('h2'), color: RP.ink, fontSize: 14 }}>Recent Sessions</div>
            <div style={{ ...t('caption'), color: RP.gunmetal, fontSize: 10 }}>SHOWING 6 OF 142</div>
          </div>
          <div style={{ display: 'flex', gap: 8 }}>
            <ActionButton variant="ghost" size="sm" icon={<Icon name="filter" size={12} />}>Filter</ActionButton>
            <ActionButton variant="ghost" size="sm" icon={<Icon name="download" size={12} />}>Export</ActionButton>
          </div>
        </div>
        <SessionsTable />
      </div>
    </div>
  );
}

function ChampionBar() {
  return (
    <div style={{ display: 'flex', gap: 2, marginTop: 6 }}>
      {[1, 1, 1, 1].map((_, i) => (
        <div key={i} style={{ flex: 1, height: 4, background: RP.classChampion }} />
      ))}
    </div>
  );
}

function BigStat({ label, value, sub, mono, accent, custom }: {
  label: string; value: string; sub?: string; mono?: boolean; accent?: string; custom?: React.ReactNode;
}) {
  return (
    <div style={{ padding: '14px 20px', borderRight: `1px solid ${RP.border}`, position: 'relative' }}>
      {accent && <div style={{ position: 'absolute', left: 0, top: 14, bottom: 14, width: 2, background: accent }} />}
      <div style={{ ...t('caption'), color: RP.gunmetal, fontSize: 9 }}>{label}</div>
      <div style={{ ...(mono ? t('monoBig') : t('displayM')), fontFamily: mono ? FONT.mono : FONT.display, fontSize: 22, color: RP.ink, marginTop: 4 }}>
        {value}
      </div>
      {custom || <div style={{ ...t('bodySm'), fontSize: 10, color: RP.inkDim, marginTop: 2 }}>{sub}</div>}
    </div>
  );
}

function DetailTab({ label, count, active }: { label: string; count?: string; active?: boolean }) {
  return (
    <div style={{
      padding: '12px 16px',
      borderBottom: active ? `2px solid ${RP.red}` : '2px solid transparent',
      color: active ? RP.ink : RP.inkDim,
      ...t('caption'), fontSize: 11,
      display: 'flex', alignItems: 'center', gap: 6, cursor: 'pointer',
    }}>
      {label}
      {count && <span style={{ ...t('mono'), fontSize: 10, color: RP.gunmetal }}>{count}</span>}
    </div>
  );
}

function SessionsTable() {
  const ROWS = [
    { date: '2026-05-02', time: '14:30', pod: 'POD-01', game: 'iRacing · Spa-Francorchamps', dur: '30:00', laps: 18, best: '2:00.630', state: 'Active', amt: '₹600' },
    { date: '2026-05-02', time: '11:15', pod: 'POD-03', game: 'iRacing · Monza',              dur: '60:00', laps: 32, best: '1:42.318', state: 'Done',   amt: '₹1,200' },
    { date: '2026-04-30', time: '20:42', pod: 'POD-07', game: 'iRacing · Suzuka',             dur: '30:00', laps: 14, best: '1:35.821', state: 'Done',   amt: '₹600' },
    { date: '2026-04-29', time: '19:08', pod: 'POD-02', game: 'F1 24 · Bahrain',              dur: '45:00', laps: 22, best: '1:31.402', state: 'Done',   amt: '₹900' },
    { date: '2026-04-28', time: '17:22', pod: 'POD-01', game: 'iRacing · Silverstone',        dur: '30:00', laps: 16, best: '1:28.114', state: 'Done',   amt: '₹600' },
    { date: '2026-04-26', time: '15:55', pod: 'POD-04', game: 'AC · Nordschleife',            dur: '60:00', laps: 8,  best: '7:24.918', state: 'Done',   amt: '₹1,200' },
  ];
  return (
    <div style={{ background: RP.card, border: `1px solid ${RP.border}`, flex: 1, overflow: 'hidden', display: 'flex', flexDirection: 'column', minHeight: 0 }}>
      <div style={{
        display: 'grid',
        gridTemplateColumns: '110px 90px 220px 80px 60px 120px 90px 90px',
        padding: '10px 16px', background: RP.base, borderBottom: `1px solid ${RP.border}`,
        ...t('caption'), color: RP.gunmetal, fontSize: 9, gap: 12,
      }}>
        <div>DATE</div><div>POD</div><div>GAME · TRACK</div><div>DURATION</div><div>LAPS</div><div>BEST LAP</div><div>STATE</div><div style={{ textAlign: 'right' }}>AMOUNT</div>
      </div>
      <div style={{ flex: 1, overflowY: 'auto' }}>
        {ROWS.map((r, i) => (
          <div key={i} style={{
            display: 'grid',
            gridTemplateColumns: '110px 90px 220px 80px 60px 120px 90px 90px',
            padding: '12px 16px', borderBottom: `1px solid ${RP.border}`, gap: 12, alignItems: 'center',
            background: i === 0 ? 'rgba(225,6,0,0.04)' : 'transparent',
          }}>
            <div>
              <div style={{ ...t('mono'), fontSize: 11, color: RP.ink }}>{r.date}</div>
              <div style={{ ...t('mono'), fontSize: 10, color: RP.inkDim }}>{r.time}</div>
            </div>
            <div style={{ ...t('mono'), fontSize: 11, color: RP.ink }}>{r.pod}</div>
            <div style={{ ...t('body'), fontSize: 12, color: RP.ink, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{r.game}</div>
            <div style={{ ...t('mono'), fontSize: 11, color: RP.inkDim }}>{r.dur}</div>
            <div style={{ ...t('mono'), fontSize: 11, color: RP.ink }}>{r.laps}</div>
            <div style={{ ...t('mono'), fontSize: 12, color: i === 0 ? RP.red : RP.ink, fontWeight: 600 }}>{r.best}</div>
            <div>
              <span style={{ ...t('caption'), fontSize: 9, padding: '2px 6px',
                color: r.state === 'Active' ? RP.green : RP.inkDim,
                border: `1px solid ${r.state === 'Active' ? RP.green : RP.border}` }}>
                {r.state === 'Active' ? '● LIVE' : r.state.toUpperCase()}
              </span>
            </div>
            <div style={{ ...t('mono'), fontSize: 11, color: RP.ink, textAlign: 'right' }}>{r.amt}</div>
          </div>
        ))}
      </div>
    </div>
  );
}
