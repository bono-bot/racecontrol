import { Coffee, Bell, Check, Clock } from "lucide-react";
import Link from "next/link";
import { Panel, ActionButton, StatusDot, StatusTile } from "@/components/rp";

// Cafe Orders Queue — POS-side visibility into chef display state +
// staff-courier delivery handoff. Per Q-S6 closure (Opt C hybrid) + V2 Q3
// LOCKED. Three lanes: pending (chef hasn't started) / preparing (chef
// active) / ready (staff to deliver).
//
// MOCK; Phase 1 wires GET /api/v1/cafe/orders + WS state stream from
// chef display.

type CafeOrderState = "pending" | "preparing" | "ready" | "delivered";

type CafeOrder = {
  id: string;
  table: string;
  customer: string;
  items: string[];
  totalPaise: number;
  receivedAt: string;
  state: CafeOrderState;
  whatsappOptIn: boolean;
};

const MOCK_ORDERS: CafeOrder[] = [
  {
    id: "ord_a1",
    table: "Pod 5 (mid-session)",
    customer: "Amruth M.",
    items: ["1× Filter coffee", "1× Samosa 2pc"],
    totalPaise: 11000,
    receivedAt: "18:11",
    state: "pending",
    whatsappOptIn: true,
  },
  {
    id: "ord_a2",
    table: "Table 3",
    customer: "—",
    items: ["2× Masala chai", "1× Veg sandwich"],
    totalPaise: 20000,
    receivedAt: "18:08",
    state: "preparing",
    whatsappOptIn: false,
  },
  {
    id: "ord_a3",
    table: "Pod 1 (mid-session)",
    customer: "Vivek S.",
    items: ["1× Hyderabadi biryani · veg", "1× Buttermilk"],
    totalPaise: 27500,
    receivedAt: "17:55",
    state: "ready",
    whatsappOptIn: true,
  },
  {
    id: "ord_a4",
    table: "Counter",
    customer: "Walk-in",
    items: ["1× South Indian thali"],
    totalPaise: 18000,
    receivedAt: "17:30",
    state: "delivered",
    whatsappOptIn: false,
  },
];

function inr(p: number) {
  return `₹${Math.round(p / 100)}`;
}

const STATE_CONFIG: Record<
  CafeOrderState,
  { dot: "amber" | "blue" | "green" | "grey"; label: string; pulse?: boolean }
> = {
  pending: { dot: "amber", label: "Pending · chef" },
  preparing: { dot: "blue" as never, label: "Preparing" }, // dot maps to grey since blue not in StatusDot
  ready: { dot: "green", label: "Ready for delivery", pulse: true },
  delivered: { dot: "grey", label: "Delivered" },
};

// Workaround: StatusDot doesn't accept blue per current type. Map preparing → grey for now.
const STATE_DOT: Record<CafeOrderState, "amber" | "green" | "red" | "grey"> = {
  pending: "amber",
  preparing: "grey",
  ready: "green",
  delivered: "grey",
};

export default function CafeOrdersQueue() {
  const pending = MOCK_ORDERS.filter((o) => o.state === "pending").length;
  const preparing = MOCK_ORDERS.filter((o) => o.state === "preparing").length;
  const ready = MOCK_ORDERS.filter((o) => o.state === "ready").length;
  const delivered = MOCK_ORDERS.filter((o) => o.state === "delivered").length;

  return (
    <div className="px-6 py-6 space-y-6">
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-4">
          <Coffee className="size-6 text-rp-amber" />
          <div>
            <p className="font-body uppercase text-[11px] tracking-[0.12em] text-rp-grey">
              Cafe queue · staff-courier delivery
            </p>
            <h1 className="font-display text-3xl font-bold text-rp-ink uppercase tracking-tight">
              Orders
            </h1>
          </div>
        </div>
        <Link href="/pos/cafe">
          <ActionButton variant="primary" size="md">
            <Coffee className="size-4" /> New cafe order
          </ActionButton>
        </Link>
      </div>

      <div className="grid grid-cols-4 gap-3">
        <StatusTile caption="Pending · chef" value={pending} display />
        <StatusTile caption="Preparing" value={preparing} display />
        <StatusTile caption="Ready" value={ready} display />
        <StatusTile caption="Delivered · today" value={delivered} sub="last 60 min" />
      </div>

      <Panel
        title="Live queue"
        action={
          <span className="font-mono text-[10px] text-rp-ink-dim">
            <Clock className="size-3 inline mr-1" /> auto-refresh 5s
          </span>
        }
      >
        <table className="w-full text-sm">
          <thead className="border-b border-rp-border">
            <tr className="font-body uppercase text-[10px] tracking-[0.08em] text-rp-grey">
              <th className="text-left py-2 pr-4">Order</th>
              <th className="text-left py-2 pr-4">Table / Pod</th>
              <th className="text-left py-2 pr-4">Customer</th>
              <th className="text-left py-2 pr-4">Items</th>
              <th className="text-right py-2 pr-4">Total</th>
              <th className="text-left py-2 pr-4">State</th>
              <th className="text-right py-2">Action</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-rp-border">
            {MOCK_ORDERS.map((o) => (
              <tr key={o.id} className="hover:bg-rp-card-hi transition-colors duration-(--motion-fast)">
                <td className="py-2 pr-4 font-mono text-xs">
                  <p className="text-rp-ink">{o.id}</p>
                  <p className="text-rp-ink-faint">{o.receivedAt}</p>
                </td>
                <td className="py-2 pr-4 font-body text-rp-ink">{o.table}</td>
                <td className="py-2 pr-4 font-body text-rp-ink-dim">
                  {o.customer}
                  {o.whatsappOptIn && (
                    <span className="ml-2 font-mono text-[9px] uppercase tracking-wider text-rp-green border border-rp-green/40 rounded px-1 py-0.5">
                      WA
                    </span>
                  )}
                </td>
                <td className="py-2 pr-4 font-body text-xs text-rp-ink-dim">
                  {o.items.map((i, ix) => (
                    <p key={ix}>{i}</p>
                  ))}
                </td>
                <td className="py-2 pr-4 text-right font-mono tabular-nums text-rp-ink">{inr(o.totalPaise)}</td>
                <td className="py-2 pr-4">
                  <span className="inline-flex items-center gap-2">
                    <StatusDot state={STATE_DOT[o.state]} pulse={STATE_CONFIG[o.state].pulse} />
                    <span className="font-mono text-xs text-rp-ink">{STATE_CONFIG[o.state].label}</span>
                  </span>
                </td>
                <td className="py-2 text-right">
                  {o.state === "ready" && (
                    <ActionButton variant="primary" size="sm">
                      <Check className="size-3.5" /> Mark delivered
                    </ActionButton>
                  )}
                  {o.state === "preparing" && (
                    <span className="font-mono text-[10px] text-rp-ink-faint">chef working</span>
                  )}
                  {o.state === "pending" && (
                    <span className="font-mono text-[10px] text-rp-amber">
                      <Bell className="size-3 inline mr-1" /> chef notified
                    </span>
                  )}
                  {o.state === "delivered" && (
                    <span className="font-mono text-[10px] text-rp-ink-faint">paid · counter</span>
                  )}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </Panel>
    </div>
  );
}
