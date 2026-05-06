import Link from "next/link";
import { Search, UserPlus, CreditCard, AlertTriangle, Coffee, Activity } from "lucide-react";
import { Panel, StatusTile, PodCard, ActionButton } from "@/components/rp";

// POS landing dashboard — quick actions + glanceable state.
// MOCK data; Phase 1 wires to /api/v1/sessions/active + /api/v1/cafe/orders.

const MOCK_PODS = [
  { n: 1, state: "green" as const, caption: "AC · Spa · Vivek S." },
  { n: 2, state: "green" as const, caption: "iRacing · Daytona" },
  { n: 3, state: "amber" as const, pulse: true, caption: "warn · ws stale" },
  { n: 4, state: "red" as const, caption: "fault · 0xC0000005" },
  { n: 5, state: "green" as const, caption: "F1 25 · Monaco" },
  { n: 6, state: "green" as const, caption: "LMU · Le Mans" },
  { n: 7, state: "grey" as const, caption: "idle" },
  { n: 8, state: "grey" as const, caption: "idle" },
];

export default function PosDashboard() {
  return (
    <div className="px-6 py-6 space-y-6">
      <div>
        <p className="font-body uppercase text-[11px] tracking-[0.12em] text-rp-grey">
          Dashboard · 2026-05-06 18:14 IST
        </p>
        <h1 className="font-display text-3xl font-bold text-rp-ink uppercase tracking-tight">
          Active state
        </h1>
      </div>

      <div className="grid grid-cols-4 gap-3">
        <StatusTile caption="Pods active" value="4" sub="of 8" display />
        <StatusTile caption="PS5 active" value="1" sub="of 3" display />
        <StatusTile caption="Revenue · today" value="₹38,400" sub="22 sessions" />
        <StatusTile caption="Cafe queue" value="2 + 1" sub="pending + ready" />
      </div>

      <Panel
        title="Quick actions"
        action={<span className="font-mono text-xs text-rp-ink-dim">staff workflow shortcuts</span>}
      >
        <div className="grid grid-cols-5 gap-3">
          <QuickAction href="/pos/lookup" icon={<Search className="size-5" />} label="Lookup" sub="CIRS phone" />
          <QuickAction href="/pos/intake" icon={<UserPlus className="size-5" />} label="Intake" sub="New customer" />
          <QuickAction href="/pos/sessions" icon={<CreditCard className="size-5" />} label="Sessions" sub="Pods + PS5" />
          <QuickAction href="/pos/quick-issue" icon={<AlertTriangle className="size-5" />} label="Quick issue" sub="Apology / refund" />
          <QuickAction href="/pos/cafe" icon={<Coffee className="size-5" />} label="Cafe order" sub="Walk-in" />
        </div>
      </Panel>

      <Panel
        title="Pods · live"
        action={
          <Link href="/pos/sessions" className="font-body uppercase text-xs tracking-wide text-rp-ink-dim hover:text-rp-red">
            View all sessions →
          </Link>
        }
      >
        <div className="grid grid-cols-4 gap-3">
          {MOCK_PODS.map((p) => (
            <PodCard
              key={p.n}
              podNumber={p.n}
              state={p.state}
              pulse={p.pulse}
              caption={p.caption}
            />
          ))}
        </div>
      </Panel>
    </div>
  );
}

function QuickAction({
  href,
  icon,
  label,
  sub,
}: {
  href: string;
  icon: React.ReactNode;
  label: string;
  sub: string;
}) {
  return (
    <Link
      href={href}
      className="flex flex-col items-start gap-2 px-4 py-4 rounded-md bg-rp-card border border-rp-border hover:border-rp-red hover:bg-rp-card-hi transition-colors duration-(--motion-fast)"
    >
      <div className="flex items-center justify-center size-10 rounded-md bg-rp-card-hi border border-rp-border-hi text-rp-ink">
        {icon}
      </div>
      <div className="space-y-0.5">
        <p className="font-display font-bold uppercase text-sm tracking-wide text-rp-ink">{label}</p>
        <p className="font-mono text-[10px] text-rp-ink-dim">{sub}</p>
      </div>
    </Link>
  );
}
