'use client';

// Racing Point V2 Admin Frame — shared topbar + left nav rail
// Source: claude.ai/design handoff bundle hq8O1_qIHf27VSd3rvj51g
// (extracted from page-cockpit.jsx + page-pod-detail.jsx so admin pages share chrome)

import * as React from 'react';
import Link from 'next/link';
import { RP, FONT, RADIUS, t } from '../tokens';
import { Icon, StatusDot } from '../atomic';

export type AdminNav = 'cockpit' | 'pods' | 'services' | 'customers' | 'telemetry' | 'leaderboard' | 'flags' | 'settings';

export function AdminFrame({
  active,
  breadcrumbs,
  rightStatus,
  children,
}: {
  active: AdminNav;
  breadcrumbs?: { label: string; href?: string }[];
  rightStatus?: React.ReactNode;
  children: React.ReactNode;
}) {
  return (
    <div style={{
      width: '100vw', height: '100vh', background: RP.asphalt, color: RP.ink,
      fontFamily: FONT.body, display: 'flex', flexDirection: 'column',
      overflow: 'hidden', boxSizing: 'border-box',
    }}>
      <AdminTopBar breadcrumbs={breadcrumbs} rightStatus={rightStatus} />
      <AdminNavRail active={active} />
      <div style={{ display: 'flex', flex: 1, minHeight: 0 }}>
        <div style={{ width: 56, flexShrink: 0 }} />
        <div style={{ flex: 1, minWidth: 0, display: 'flex', flexDirection: 'column' }}>
          {children}
        </div>
      </div>
    </div>
  );
}

function AdminTopBar({
  breadcrumbs,
  rightStatus,
}: {
  breadcrumbs?: { label: string; href?: string }[];
  rightStatus?: React.ReactNode;
}) {
  return (
    <div style={{
      height: 48, background: RP.base, borderBottom: `1px solid ${RP.border}`,
      display: 'flex', alignItems: 'center', padding: '0 20px', gap: 16, flexShrink: 0,
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
      {breadcrumbs && breadcrumbs.length > 0 && (
        <>
          <div style={{ width: 1, height: 24, background: RP.border }} />
          <div style={{ display: 'flex', alignItems: 'center', gap: 6, ...t('caption'), fontSize: 10 }}>
            {breadcrumbs.map((b, i) => (
              <React.Fragment key={i}>
                {i > 0 && <Icon name="chevron" size={9} color={RP.gunmetal} />}
                {b.href
                  ? <Link href={b.href} style={{ color: RP.gunmetal, textDecoration: 'none' }}>{b.label}</Link>
                  : <span style={{ color: i === breadcrumbs.length - 1 ? RP.ink : RP.gunmetal }}>{b.label}</span>
                }
              </React.Fragment>
            ))}
          </div>
        </>
      )}
      {!breadcrumbs && (
        <>
          <div style={{ width: 1, height: 24, background: RP.border }} />
          <div style={{ display: 'flex', gap: 6, ...t('caption'), color: RP.gunmetal, fontSize: 10 }}>
            <span>VENUE</span><span style={{ color: RP.ink }}>BANDLAGUDA</span>
            <span style={{ color: RP.borderHi, margin: '0 6px' }}>/</span>
            <span>MODE</span><span style={{ color: RP.green }}>LIVE</span>
          </div>
        </>
      )}
      <div style={{ flex: 1 }} />
      {rightStatus || (
        <div style={{ display: 'flex', alignItems: 'center', gap: 16, ...t('mono'), fontSize: 11, color: RP.inkDim }}>
          <span>16:42:08 IST</span>
          <span style={{ display: 'flex', gap: 6, alignItems: 'center' }}>
            <StatusDot state="green" /> JAMES <span style={{ color: RP.gunmetal }}>· 43ms</span>
          </span>
          <span style={{ display: 'flex', gap: 6, alignItems: 'center' }}>
            <StatusDot state="green" /> BONO
          </span>
        </div>
      )}
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

const NAV_ITEMS: { id: AdminNav; icon: string; label: string; href: string }[] = [
  { id: 'cockpit',     icon: 'grid',     label: 'Cockpit',     href: '/v2/admin/cockpit' },
  { id: 'pods',        icon: 'zap',      label: 'Pods',        href: '/v2/admin/pods' },
  { id: 'services',    icon: 'layers',   label: 'Services',    href: '/v2/admin/services' },
  { id: 'customers',   icon: 'user',     label: 'Customers',   href: '/v2/admin/customers' },
  { id: 'telemetry',   icon: 'activity', label: 'Telemetry',   href: '/v2/admin/telemetry' },
  { id: 'leaderboard', icon: 'trophy',   label: 'Leaderboard', href: '/v2/admin/leaderboard' },
];

const NAV_BOTTOM: { id: AdminNav; icon: string; label: string; href: string }[] = [
  { id: 'flags',    icon: 'flag',     label: 'Flags',    href: '/v2/admin/flags' },
  { id: 'settings', icon: 'settings', label: 'Settings', href: '/v2/admin/settings' },
];

function AdminNavRail({ active }: { active: AdminNav }) {
  return (
    <div style={{
      position: 'absolute', left: 0, top: 48, bottom: 0, width: 56,
      background: RP.base, borderRight: `1px solid ${RP.border}`,
      display: 'flex', flexDirection: 'column', alignItems: 'center', padding: '12px 0', gap: 4,
    }}>
      {NAV_ITEMS.map(item => <NavItem key={item.id} {...item} active={active === item.id} />)}
      <div style={{ flex: 1 }} />
      {NAV_BOTTOM.map(item => <NavItem key={item.id} {...item} active={active === item.id} />)}
    </div>
  );
}

function NavItem({ icon, label, href, active }: { icon: string; label: string; href: string; active?: boolean }) {
  return (
    <Link href={href} style={{ position: 'relative', width: '100%', display: 'flex', justifyContent: 'center', textDecoration: 'none' }}>
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
    </Link>
  );
}

export function AdminPanel({
  title,
  trailing,
  children,
  compact,
}: {
  title: string;
  trailing?: React.ReactNode;
  children: React.ReactNode;
  compact?: boolean;
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
