"use client";

import * as React from "react";
import { useRouter } from "next/navigation";
import Link from "next/link";
import { ArrowLeft, UserPlus, Check } from "lucide-react";
import { Panel, ActionButton } from "@/components/rp";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";

// Customer Intake — POS new-customer registration. Two paths:
//   1. M1 sim-racing customer: phone + name (consent flow inferred from
//      explicit registration action per §S-57 DPDP-dissolved doctrine)
//   2. Walk-in cafe-only customer: anonymous (skip phone capture; cafe
//      orders billed at counter without customer record)
// MOCK only this pass; Phase 1 wires POST /api/v1/customers + voucher mint.

export default function CustomerIntake() {
  const router = useRouter();
  const [path, setPath] = React.useState<"sim" | "walkin" | null>(null);
  const [name, setName] = React.useState("");
  const [phone, setPhone] = React.useState("");

  return (
    <div className="px-6 py-6 max-w-3xl space-y-6">
      <div className="flex items-center gap-4">
        <Link href="/pos/lookup">
          <ActionButton variant="ghost" size="sm">
            <ArrowLeft className="size-4" /> Back to lookup
          </ActionButton>
        </Link>
        <div>
          <p className="font-body uppercase text-[11px] tracking-[0.12em] text-rp-grey">
            Customer ops
          </p>
          <h1 className="font-display text-3xl font-bold text-rp-ink uppercase tracking-tight">
            Intake
          </h1>
        </div>
      </div>

      {!path && (
        <Panel title="Pick path · what does the customer need?">
          <div className="grid grid-cols-2 gap-4">
            <button
              onClick={() => setPath("sim")}
              className="flex flex-col items-start gap-2 px-5 py-5 rounded-md bg-rp-card border border-rp-border hover:border-rp-red hover:bg-rp-card-hi transition-colors duration-(--motion-fast) text-left"
            >
              <p className="font-display font-bold uppercase text-base tracking-wide text-rp-ink">
                Sim-racing / PS5
              </p>
              <p className="font-body text-sm text-rp-ink-dim">
                Phone capture required · M1 registration · wallet voucher mint
              </p>
              <p className="font-mono text-[10px] text-rp-ink-faint">
                Path 1 · sim / PS5 / kiosk launch
              </p>
            </button>
            <button
              onClick={() => setPath("walkin")}
              className="flex flex-col items-start gap-2 px-5 py-5 rounded-md bg-rp-card border border-rp-border hover:border-rp-amber hover:bg-rp-card-hi transition-colors duration-(--motion-fast) text-left"
            >
              <p className="font-display font-bold uppercase text-base tracking-wide text-rp-ink">
                Walk-in cafe only
              </p>
              <p className="font-body text-sm text-rp-ink-dim">
                Anonymous · no phone capture · cafe billed at counter
              </p>
              <p className="font-mono text-[10px] text-rp-ink-faint">
                Path 2 · walk-in guest · skip registration
              </p>
            </button>
          </div>
        </Panel>
      )}

      {path === "sim" && (
        <Panel title="Sim-racing registration · M1">
          <form
            onSubmit={(e) => {
              e.preventDefault();
              // TODO Phase 1: POST /api/v1/customers → router.push(`/pos/lookup/${id}`)
              console.log("register sim", { name, phone });
            }}
            className="space-y-4"
          >
            <div className="space-y-1.5">
              <Label htmlFor="name" className="font-body uppercase text-xs tracking-[0.08em] text-rp-ink-dim">
                Full name
              </Label>
              <Input
                id="name"
                type="text"
                placeholder="Vivek Singh"
                value={name}
                onChange={(e) => setName(e.target.value)}
                className="text-lg h-12"
                autoFocus
              />
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="phone" className="font-body uppercase text-xs tracking-[0.08em] text-rp-ink-dim">
                Indian mobile (+91)
              </Label>
              <Input
                id="phone"
                type="tel"
                inputMode="numeric"
                pattern="[6-9][0-9]{9}"
                placeholder="98765 43210"
                value={phone}
                onChange={(e) => setPhone(e.target.value)}
                className="font-mono tabular-nums text-lg h-12"
              />
              <p className="font-mono text-[10px] text-rp-ink-faint">
                10 digits · prefix 6/7/8/9 · WhatsApp marketing opt-in implied by registration
              </p>
            </div>
            <div className="flex items-center gap-3 pt-2">
              <ActionButton type="submit" variant="primary" size="md">
                <Check className="size-4" /> Register + mint wallet
              </ActionButton>
              <ActionButton type="button" variant="ghost" size="md" onClick={() => setPath(null)}>
                Cancel
              </ActionButton>
            </div>
            <p className="font-mono text-[10px] text-rp-ink-faint pt-2 border-t border-rp-border">
              Wallet voucher: ₹0 initial · top-up at next step · 18% GST collected at top-up · sim/PS5 only
            </p>
          </form>
        </Panel>
      )}

      {path === "walkin" && (
        <Panel title="Walk-in cafe-only · anonymous">
          <div className="space-y-4">
            <p className="font-body text-sm text-rp-ink-dim">
              No customer record created. Cafe order goes to chef + staff delivers.
              Customer pays at counter. No wallet, no phone capture.
            </p>
            <div className="flex items-center gap-3">
              <Link href="/pos/cafe?walkin=1">
                <ActionButton variant="primary" size="md">
                  <UserPlus className="size-4" /> Start cafe order
                </ActionButton>
              </Link>
              <ActionButton variant="ghost" size="md" onClick={() => setPath(null)}>
                Cancel
              </ActionButton>
            </div>
            <p className="font-mono text-[10px] text-rp-ink-faint pt-2 border-t border-rp-border">
              Per Wallet Framing C: cafe always separate · sim/PS5 customers can pay
              cafe via cash/card/UPI but not wallet credits
            </p>
          </div>
        </Panel>
      )}
    </div>
  );
}
