"use client";

import * as React from "react";
import { Search, UserPlus, Clock } from "lucide-react";
import Link from "next/link";
import { Panel, ActionButton } from "@/components/rp";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";

// Customer Lookup — POS .130 staff workflow.
// CIRS phone search; on-found → /pos/lookup/[id]; on-not-found → /pos/intake.
// MOCK recent customers; Phase 1 wires POST /api/v1/cirs/lookup per
// PACT-20260506-001 §6.

const MOCK_RECENT = [
  { id: "cus_01ja", phone: "+91 98765 43210", name: "Vivek Singh", lastVisit: "today 17:42" },
  { id: "cus_02jb", phone: "+91 98123 67890", name: "Ronit Paul", lastVisit: "today 16:15" },
  { id: "cus_03jc", phone: "+91 99887 11223", name: "Amruth Mann", lastVisit: "yesterday 21:30" },
];

export default function CustomerLookup() {
  const [phone, setPhone] = React.useState("");

  return (
    <div className="px-6 py-6 max-w-3xl space-y-6">
      <div>
        <p className="font-body uppercase text-[11px] tracking-[0.12em] text-rp-grey">
          Customer ops
        </p>
        <h1 className="font-display text-3xl font-bold text-rp-ink uppercase tracking-tight">
          Lookup
        </h1>
      </div>

      <Panel title="CIRS phone search">
        <form
          onSubmit={(e) => {
            e.preventDefault();
            // TODO Phase 1: POST /api/v1/cirs/lookup → router.push(`/pos/lookup/${id}`)
            console.log("lookup", phone);
          }}
          className="space-y-4"
        >
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
              autoFocus
            />
            <p className="font-mono text-[10px] text-rp-ink-faint">
              10 digits · prefix 6/7/8/9 · stored SHA256-only in audit
            </p>
          </div>
          <div className="flex items-center gap-3">
            <ActionButton type="submit" variant="primary" size="md">
              <Search className="size-4" /> Lookup
            </ActionButton>
            <Link href="/pos/intake">
              <ActionButton variant="secondary" size="md">
                <UserPlus className="size-4" /> Walk-in / new
              </ActionButton>
            </Link>
          </div>
        </form>
      </Panel>

      <Panel
        title="Recent · this terminal"
        action={
          <span className="font-mono text-[10px] text-rp-ink-dim">
            <Clock className="size-3 inline mr-1" />
            last 6 hours
          </span>
        }
      >
        <ul className="space-y-1">
          {MOCK_RECENT.map((c) => (
            <li key={c.id}>
              <Link
                href={`/pos/lookup/${c.id}`}
                className="flex items-center justify-between px-3 py-2 rounded-md hover:bg-rp-card-hi transition-colors duration-(--motion-fast)"
              >
                <div>
                  <p className="font-body text-sm font-medium text-rp-ink">{c.name}</p>
                  <p className="font-mono text-[11px] text-rp-ink-dim">{c.phone}</p>
                </div>
                <span className="font-mono text-[11px] text-rp-ink-faint">{c.lastVisit}</span>
              </Link>
            </li>
          ))}
        </ul>
      </Panel>
    </div>
  );
}
