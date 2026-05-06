"use client";

import * as React from "react";
import { Coffee, Plus, Minus, Send, Trash2 } from "lucide-react";
import Link from "next/link";
import { Panel, ActionButton } from "@/components/rp";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";

// Cafe Ordering — POS-side cafe order entry. Per V2 Q3 LOCKED + Q-S6 closure
// (§S-57): system delivery_location: cafe_seating + staff handoff + chef
// display kitchen audio + WhatsApp ready-receipt.
//
// Wallet Framing C: cafe ALWAYS separate from sim/PS5 wallet. Customer pays
// cafe via cash/card/UPI at counter (sim wallet credits NOT redeemable on cafe).
//
// MOCK menu + KOT this pass; Phase 1 wires:
//   - GET /api/v1/cafe/menu (active items per current rate)
//   - POST /api/v1/cafe/orders (KOT to chef + Bill of Order print fallback)
//   - WhatsApp template fire on submit (M1 customers) per §S-57 Print Module

type MenuItem = {
  id: string;
  name: string;
  category: "drink" | "snack" | "meal";
  pricePaise: number;
};

const MOCK_MENU: MenuItem[] = [
  { id: "drink_filter_coffee", name: "Filter coffee", category: "drink", pricePaise: 5000 },
  { id: "drink_chai", name: "Masala chai", category: "drink", pricePaise: 4000 },
  { id: "drink_lime_soda", name: "Lime soda · sweet", category: "drink", pricePaise: 6000 },
  { id: "drink_butter_milk", name: "Buttermilk", category: "drink", pricePaise: 5500 },
  { id: "snack_samosa", name: "Samosa · 2pc", category: "snack", pricePaise: 6000 },
  { id: "snack_sandwich", name: "Veg sandwich", category: "snack", pricePaise: 12000 },
  { id: "snack_french_fries", name: "French fries", category: "snack", pricePaise: 14000 },
  { id: "meal_biryani", name: "Hyderabadi biryani · veg", category: "meal", pricePaise: 22000 },
  { id: "meal_thali", name: "South Indian thali", category: "meal", pricePaise: 18000 },
];

type CartLine = { item: MenuItem; qty: number };

function formatPaise(p: number) {
  return `₹${(p / 100).toLocaleString("en-IN", { minimumFractionDigits: 2, maximumFractionDigits: 2 })}`;
}

function inrShort(p: number) {
  return `₹${Math.round(p / 100)}`;
}

export default function CafeOrder() {
  const [cart, setCart] = React.useState<CartLine[]>([]);
  const [phone, setPhone] = React.useState("");
  const [tableLabel, setTableLabel] = React.useState("");

  const subtotal = cart.reduce((s, l) => s + l.item.pricePaise * l.qty, 0);
  const gst = Math.round(subtotal * 0.05); // food = 5% GST (vs 18% on wallet top-up)
  const total = subtotal + gst;

  function bump(item: MenuItem, delta: number) {
    setCart((c) => {
      const ix = c.findIndex((l) => l.item.id === item.id);
      if (ix === -1 && delta > 0) return [...c, { item, qty: 1 }];
      if (ix === -1) return c;
      const next = [...c];
      next[ix] = { ...next[ix], qty: Math.max(0, next[ix].qty + delta) };
      return next.filter((l) => l.qty > 0);
    });
  }

  function clear() {
    setCart([]);
    setPhone("");
    setTableLabel("");
  }

  return (
    <div className="px-6 py-6 space-y-6">
      <div className="flex items-center gap-4">
        <Coffee className="size-6 text-rp-amber" />
        <div>
          <p className="font-body uppercase text-[11px] tracking-[0.12em] text-rp-grey">
            Cafe · separate billing · 5% GST
          </p>
          <h1 className="font-display text-3xl font-bold text-rp-ink uppercase tracking-tight">
            New cafe order
          </h1>
        </div>
      </div>

      <div className="grid grid-cols-3 gap-6">
        <div className="col-span-2 space-y-6">
          <MenuGroup title="Drinks" items={MOCK_MENU.filter((i) => i.category === "drink")} cart={cart} bump={bump} />
          <MenuGroup title="Snacks" items={MOCK_MENU.filter((i) => i.category === "snack")} cart={cart} bump={bump} />
          <MenuGroup title="Meals" items={MOCK_MENU.filter((i) => i.category === "meal")} cart={cart} bump={bump} />
        </div>

        <div className="col-span-1">
          <div className="sticky top-4 space-y-4">
            <Panel
              title="Order · KOT preview"
              action={
                cart.length > 0 ? (
                  <button
                    onClick={clear}
                    className="font-mono text-[10px] text-rp-ink-dim hover:text-rp-red transition-colors"
                  >
                    <Trash2 className="size-3 inline mr-1" /> clear
                  </button>
                ) : undefined
              }
            >
              {cart.length === 0 ? (
                <p className="font-body text-sm text-rp-ink-faint italic py-6 text-center">
                  No items yet. Pick from menu →
                </p>
              ) : (
                <ul className="space-y-2">
                  {cart.map((l) => (
                    <li key={l.item.id} className="flex items-center justify-between">
                      <div>
                        <p className="font-body text-sm text-rp-ink">{l.item.name}</p>
                        <p className="font-mono text-[10px] text-rp-ink-dim">
                          {l.qty} × {formatPaise(l.item.pricePaise)}
                        </p>
                      </div>
                      <div className="flex items-center gap-1">
                        <button
                          onClick={() => bump(l.item, -1)}
                          aria-label={`remove one ${l.item.name}`}
                          className="size-7 inline-flex items-center justify-center rounded-md bg-rp-card-hi border border-rp-border-hi hover:border-rp-red transition-colors"
                        >
                          <Minus className="size-3 text-rp-ink" />
                        </button>
                        <span className="font-mono text-sm tabular-nums w-6 text-center text-rp-ink">{l.qty}</span>
                        <button
                          onClick={() => bump(l.item, 1)}
                          aria-label={`add one ${l.item.name}`}
                          className="size-7 inline-flex items-center justify-center rounded-md bg-rp-card-hi border border-rp-border-hi hover:border-rp-red transition-colors"
                        >
                          <Plus className="size-3 text-rp-ink" />
                        </button>
                      </div>
                    </li>
                  ))}
                </ul>
              )}
            </Panel>

            <Panel title="Customer · delivery">
              <div className="space-y-3">
                <div className="space-y-1.5">
                  <Label htmlFor="cafe-phone" className="font-body uppercase text-[10px] tracking-[0.08em] text-rp-ink-dim">
                    Phone (optional)
                  </Label>
                  <Input
                    id="cafe-phone"
                    type="tel"
                    placeholder="+91 98765 43210"
                    value={phone}
                    onChange={(e) => setPhone(e.target.value)}
                    className="font-mono tabular-nums text-sm"
                  />
                  <p className="font-mono text-[9px] text-rp-ink-faint">
                    Captured → WhatsApp ready-receipt fires when chef marks ready
                  </p>
                </div>
                <div className="space-y-1.5">
                  <Label htmlFor="cafe-table" className="font-body uppercase text-[10px] tracking-[0.08em] text-rp-ink-dim">
                    Seating
                  </Label>
                  <Input
                    id="cafe-table"
                    placeholder="Table 3 / Pod 5 (mid-session) / Counter"
                    value={tableLabel}
                    onChange={(e) => setTableLabel(e.target.value)}
                    className="text-sm"
                  />
                  <p className="font-mono text-[9px] text-rp-ink-faint">
                    delivery_location: cafe_seating · staff hand-off
                  </p>
                </div>
              </div>
            </Panel>

            <Panel title="Total">
              <dl className="space-y-1.5 text-sm">
                <Row label="Subtotal" value={formatPaise(subtotal)} />
                <Row label="GST · 5%" value={formatPaise(gst)} muted />
                <div className="pt-2 border-t border-rp-border">
                  <Row label="Pay at counter" value={inrShort(total)} bold />
                </div>
              </dl>
              <p className="font-mono text-[9px] text-rp-ink-faint pt-2 mt-2 border-t border-rp-border">
                Cash / Card / UPI · sim wallet credits NOT redeemable per Wallet Framing C
              </p>
              <ActionButton
                variant="primary"
                size="lg"
                className="w-full mt-3"
                disabled={cart.length === 0 || !tableLabel}
                onClick={() => {
                  // TODO Phase 1: POST /api/v1/cafe/orders → emits to chef display + Bill of Order print + WhatsApp template
                  console.log("send-to-chef", { cart, phone, tableLabel, total });
                }}
              >
                <Send className="size-4" /> Send to chef
              </ActionButton>
              <Link href="/pos/cafe/orders" className="block text-center mt-2 font-mono text-[10px] text-rp-ink-dim hover:text-rp-red">
                View cafe queue →
              </Link>
            </Panel>
          </div>
        </div>
      </div>
    </div>
  );
}

function MenuGroup({
  title,
  items,
  cart,
  bump,
}: {
  title: string;
  items: MenuItem[];
  cart: CartLine[];
  bump: (item: MenuItem, delta: number) => void;
}) {
  return (
    <Panel title={title}>
      <div className="grid grid-cols-2 gap-2">
        {items.map((i) => {
          const line = cart.find((l) => l.item.id === i.id);
          const qty = line?.qty ?? 0;
          return (
            <div
              key={i.id}
              className="flex items-center justify-between gap-3 px-3 py-2 rounded-md bg-rp-card-hi border border-rp-border hover:border-rp-border-hi"
            >
              <div className="min-w-0">
                <p className="font-body text-sm text-rp-ink truncate">{i.name}</p>
                <p className="font-mono text-[10px] text-rp-ink-dim">{inrShort(i.pricePaise)}</p>
              </div>
              <div className="flex items-center gap-1 shrink-0">
                {qty > 0 && (
                  <>
                    <button
                      onClick={() => bump(i, -1)}
                      className="size-7 inline-flex items-center justify-center rounded-md bg-rp-card border border-rp-border-hi hover:border-rp-red transition-colors"
                      aria-label={`remove one ${i.name}`}
                    >
                      <Minus className="size-3 text-rp-ink" />
                    </button>
                    <span className="font-mono text-sm tabular-nums w-5 text-center text-rp-ink">{qty}</span>
                  </>
                )}
                <button
                  onClick={() => bump(i, 1)}
                  className="size-7 inline-flex items-center justify-center rounded-md bg-rp-red text-rp-ink hover:bg-rp-red-glow transition-colors"
                  aria-label={`add one ${i.name}`}
                >
                  <Plus className="size-3" />
                </button>
              </div>
            </div>
          );
        })}
      </div>
    </Panel>
  );
}

function Row({ label, value, bold, muted }: { label: string; value: string; bold?: boolean; muted?: boolean }) {
  return (
    <div className="flex items-center justify-between">
      <dt className={`font-body ${muted ? "text-rp-ink-dim" : "text-rp-ink"}`}>{label}</dt>
      <dd className={`font-mono tabular-nums ${bold ? "text-2xl font-bold text-rp-ink" : muted ? "text-rp-ink-dim" : "text-rp-ink"}`}>
        {value}
      </dd>
    </div>
  );
}
