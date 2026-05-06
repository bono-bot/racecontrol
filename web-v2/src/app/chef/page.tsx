import { Volume2, ChefHat } from "lucide-react";
import { StatusDot, StatusTile } from "@/components/rp";

// Chef Display — kitchen-side queue. Per Q-S6 closure: dedicated chef display
// kitchen audio + visual cues. Per F29 V2.0 audio cue sub-PACT applied to
// kitchen surface as well. Layout is large + readable across kitchen distance:
// big type, color-coded lanes, chronological ordering.
//
// MOCK; Phase 1 wires WS feed from /api/v1/cafe/orders + audio cue on
// pending → preparing transition.

type Order = {
  id: string;
  table: string;
  receivedAgo: string;
  items: { qty: number; name: string }[];
  state: "pending" | "preparing" | "ready";
};

const MOCK: Order[] = [
  {
    id: "ord_a1",
    table: "Pod 5 (mid)",
    receivedAgo: "3 min ago",
    items: [
      { qty: 1, name: "Filter coffee" },
      { qty: 1, name: "Samosa · 2pc" },
    ],
    state: "pending",
  },
  {
    id: "ord_a2",
    table: "Table 3",
    receivedAgo: "6 min ago",
    items: [
      { qty: 2, name: "Masala chai" },
      { qty: 1, name: "Veg sandwich" },
    ],
    state: "preparing",
  },
  {
    id: "ord_a3",
    table: "Pod 1 (mid)",
    receivedAgo: "18 min ago",
    items: [
      { qty: 1, name: "Hyderabadi biryani · veg" },
      { qty: 1, name: "Buttermilk" },
    ],
    state: "ready",
  },
];

const LANES: { state: Order["state"]; title: string; subtitle: string; accent: string; dot: "amber" | "green" | "grey" }[] = [
  { state: "pending", title: "Pending", subtitle: "Audio + visual cue", accent: "border-l-rp-amber", dot: "amber" },
  { state: "preparing", title: "Preparing", subtitle: "Chef working", accent: "border-l-rp-blue", dot: "grey" },
  { state: "ready", title: "Ready", subtitle: "Staff to deliver · pulse", accent: "border-l-rp-green", dot: "green" },
];

export default function ChefDisplay() {
  return (
    <div className="px-8 py-6 space-y-6">
      <div className="grid grid-cols-3 gap-4">
        <StatusTile
          caption="Pending"
          value={MOCK.filter((o) => o.state === "pending").length}
          display
          className="border-l-4 border-l-rp-amber"
        />
        <StatusTile
          caption="Preparing"
          value={MOCK.filter((o) => o.state === "preparing").length}
          display
          className="border-l-4 border-l-rp-blue"
        />
        <StatusTile
          caption="Ready · audio active"
          value={MOCK.filter((o) => o.state === "ready").length}
          display
          className="border-l-4 border-l-rp-green"
        />
      </div>

      <div className="grid grid-cols-3 gap-4">
        {LANES.map((lane) => {
          const orders = MOCK.filter((o) => o.state === lane.state);
          return (
            <div key={lane.state} className={`rounded-md bg-rp-card border border-rp-border border-l-4 ${lane.accent}`}>
              <div className="px-5 py-4 border-b border-rp-border flex items-center justify-between">
                <div>
                  <p className="font-display text-xl font-bold uppercase tracking-wide text-rp-ink">
                    {lane.title}
                  </p>
                  <p className="font-body text-xs uppercase tracking-[0.06em] text-rp-grey">
                    {lane.subtitle}
                  </p>
                </div>
                <StatusDot state={lane.dot} pulse={lane.state === "ready"} size={12} />
              </div>
              <ul className="divide-y divide-rp-border">
                {orders.length === 0 && (
                  <li className="px-5 py-12 text-center font-body text-sm text-rp-ink-faint italic">
                    None
                  </li>
                )}
                {orders.map((o) => (
                  <li key={o.id} className="px-5 py-4 space-y-2">
                    <div className="flex items-center justify-between">
                      <p className="font-display text-2xl font-bold text-rp-ink">{o.table}</p>
                      <p className="font-mono text-[11px] text-rp-ink-dim">{o.receivedAgo}</p>
                    </div>
                    <ul className="space-y-1">
                      {o.items.map((i, ix) => (
                        <li key={ix} className="font-body text-base text-rp-ink-dim flex items-center gap-2">
                          <span className="font-mono text-rp-amber tabular-nums w-6">{i.qty}×</span>
                          <span className="text-rp-ink">{i.name}</span>
                        </li>
                      ))}
                    </ul>
                    <p className="font-mono text-[10px] text-rp-ink-faint">{o.id}</p>
                  </li>
                ))}
              </ul>
            </div>
          );
        })}
      </div>

      <div className="flex items-center gap-3 px-4 py-3 rounded-md border border-rp-border bg-rp-card">
        <ChefHat className="size-5 text-rp-amber" />
        <p className="font-body text-sm text-rp-ink-dim">
          Chef workflow: tap card to advance lane (pending → preparing → ready). Ready triggers
          POS-side chef-notification widget + WhatsApp ready-receipt template (M1 customers only)
          + Bill of Order print-fallback for non-WhatsApp customers.
        </p>
        <span className="ml-auto inline-flex items-center gap-2 font-mono text-[10px] text-rp-ink-dim">
          <Volume2 className="size-3.5" />
          Audio cue · F29 V2.0
        </span>
      </div>
    </div>
  );
}
