"use client";

// Layer 1 Foundation demo — exercises Captain bundle 2026-05-02 components.jsx
// API surface via shadcn-vendored primitives + 6 rp wrappers + lucide-react.
// Per CAPTAIN-RATIFIED Layer 1 Single Statement 2026-05-06 ~17:50 IST.

import { Activity, Check, Power, Search, Zap } from "lucide-react";
import {
  ActionButton,
  Panel,
  FlagSwitch,
  StatusDot,
  PodCard,
  StatusTile,
} from "@/components/rp";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";

export default function Home() {
  return (
    <main className="mx-auto max-w-6xl px-6 py-12 space-y-10">
      <header className="space-y-3">
        <p className="font-body uppercase tracking-[0.08em] text-xs font-semibold text-rp-grey">
          Phase 0.1 · Layer 1 Foundation
        </p>
        <h1 className="font-display font-bold text-5xl tracking-tight text-rp-ink leading-none">
          RacingPoint <span className="text-rp-red">V2</span>
        </h1>
        <p className="font-body text-base text-rp-ink-dim leading-relaxed max-w-xl">
          Tokens + 9 shadcn primitives + 6 bundle-API wrappers (ActionButton /
          Panel / FlagSwitch / StatusDot / PodCard / StatusTile) + lucide icons.
          Ratified 2026-05-06 from Captain claude.ai/design bundle 2026-05-02
          (v2-design-h-V3XSuJJ).
        </p>
      </header>

      <Panel
        title="ActionButton variants"
        action={<span className="font-mono text-xs text-rp-ink-dim">primary / secondary / ghost / danger</span>}
      >
        <div className="flex flex-wrap items-center gap-3">
          <ActionButton variant="primary" size="md">Confirm payment</ActionButton>
          <ActionButton variant="secondary" size="md">
            <Search className="size-4" /> Lookup
          </ActionButton>
          <ActionButton variant="ghost" size="md">Cancel</ActionButton>
          <ActionButton variant="danger" size="md">
            <Power className="size-4" /> End session
          </ActionButton>
        </div>
        <div className="flex flex-wrap items-center gap-3 mt-4">
          <ActionButton variant="primary" size="sm">sm</ActionButton>
          <ActionButton variant="primary" size="md">md</ActionButton>
          <ActionButton variant="primary" size="lg">lg</ActionButton>
          <ActionButton variant="primary" size="xl">
            <Check className="size-5" /> XL · Start session
          </ActionButton>
        </div>
      </Panel>

      <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
        <Panel title="StatusTile · KPIs">
          <div className="grid grid-cols-3 gap-3">
            <StatusTile caption="Active" value="6" sub="of 8 pods" display />
            <StatusTile caption="Lap best" value="1:48.951" sub="Spa · SF15-T" />
            <StatusTile caption="Revenue" value="₹2,700" sub="last 60 min" />
          </div>
        </Panel>

        <Panel title="StatusDot · States">
          <div className="flex items-center gap-6">
            <div className="flex items-center gap-2"><StatusDot state="green" /><span className="font-mono text-xs text-rp-ink-dim">healthy</span></div>
            <div className="flex items-center gap-2"><StatusDot state="amber" pulse /><span className="font-mono text-xs text-rp-ink-dim">warn · pulse</span></div>
            <div className="flex items-center gap-2"><StatusDot state="red" /><span className="font-mono text-xs text-rp-ink-dim">fault</span></div>
            <div className="flex items-center gap-2"><StatusDot state="grey" /><span className="font-mono text-xs text-rp-ink-dim">idle</span></div>
          </div>
        </Panel>
      </div>

      <Panel
        title="PodCard · Fleet view"
        action={<span className="font-mono text-xs text-rp-ink-dim">8 pods</span>}
      >
        <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
          <PodCard podNumber={1} state="green" caption="AC · Spa" />
          <PodCard podNumber={2} state="green" caption="iRacing" />
          <PodCard podNumber={3} state="amber" pulse caption="warn · ws stale" />
          <PodCard podNumber={4} state="red" caption="fault · 0xC0000005" />
          <PodCard podNumber={5} state="green" caption="F1 25" />
          <PodCard podNumber={6} state="green" caption="LMU" />
          <PodCard podNumber={7} state="grey" caption="idle" />
          <PodCard podNumber={8} state="grey" caption="idle" />
        </div>
      </Panel>

      <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
        <Panel title="FlagSwitch · Feature flags">
          <div className="space-y-3">
            <FlagRow label="pos.lookup.cirs" description="POST /api/v1/cirs/lookup" defaultChecked />
            <FlagRow label="cafe.kot" description="Cafe KOT dispatch · V2.1 dark-launch" pending />
            <FlagRow label="loyalty.points" description="Loyalty + merch · V2.1 dark-launch" />
          </div>
        </Panel>

        <Panel title="Input · Customer lookup">
          <form className="space-y-3">
            <div className="space-y-1.5">
              <Label htmlFor="phone" className="font-body uppercase text-xs tracking-[0.08em] text-rp-ink-dim">
                Indian mobile (+91)
              </Label>
              <Input id="phone" type="tel" placeholder="98765 43210" className="font-mono tabular-nums" />
            </div>
            <ActionButton variant="primary" size="md">
              <Search className="size-4" /> Lookup
            </ActionButton>
          </form>
        </Panel>
      </div>

      <Panel title="Lucide icon set sample">
        <div className="flex flex-wrap items-center gap-6 text-rp-ink">
          <IconLabel icon={<Zap className="size-5 text-rp-red" />} label="zap" />
          <IconLabel icon={<Activity className="size-5 text-rp-green" />} label="activity" />
          <IconLabel icon={<Power className="size-5 text-rp-amber" />} label="power" />
          <IconLabel icon={<Check className="size-5 text-rp-green" />} label="check" />
          <IconLabel icon={<Search className="size-5 text-rp-blue" />} label="search" />
        </div>
      </Panel>

      <footer className="pt-6 border-t border-rp-border">
        <a
          className="inline-flex items-center gap-2 font-mono text-sm font-medium text-rp-red hover:text-rp-red-glow transition-colors"
          href="/v2/api/v1/health"
        >
          <Activity className="size-4" /> Health probe (PACT-20260503-001)
        </a>
      </footer>
    </main>
  );
}

function FlagRow({
  label,
  description,
  defaultChecked,
  pending,
}: {
  label: string;
  description: string;
  defaultChecked?: boolean;
  pending?: boolean;
}) {
  return (
    <div className="flex items-start justify-between gap-4 py-2 border-b border-rp-border last:border-0">
      <div className="space-y-0.5">
        <p className="font-mono text-sm text-rp-ink">{label}</p>
        <p className="font-body text-xs text-rp-ink-dim">{description}</p>
      </div>
      <FlagSwitch defaultChecked={defaultChecked} pending={pending} />
    </div>
  );
}

function IconLabel({ icon, label }: { icon: React.ReactNode; label: string }) {
  return (
    <div className="flex flex-col items-center gap-1.5">
      <div className="flex items-center justify-center size-12 rounded-md bg-rp-card-hi border border-rp-border">
        {icon}
      </div>
      <span className="font-mono text-[10px] uppercase tracking-wider text-rp-ink-dim">{label}</span>
    </div>
  );
}
