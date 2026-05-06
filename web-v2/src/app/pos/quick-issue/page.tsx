"use client";

import * as React from "react";
import {
  Heart,
  RotateCcw,
  AlertTriangle,
  MessageSquare,
  type LucideIcon,
} from "lucide-react";
import Link from "next/link";
import { Panel, ActionButton } from "@/components/rp";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";

// Quick Issue — POS staff workflow for apology credits + refunds + complaints.
// Each path requires manager-role re-auth (privileged action per Q-DECISION-6
// session-cookie default + carry-forward Captain reserve Q5 PIN lockout).
// MOCK; Phase 1 wires manager-role re-auth + audit-log + WhatsApp notify.

type IssueType = {
  id: string;
  label: string;
  icon: LucideIcon;
  description: string;
  managerAuth: boolean;
};

const ISSUES: IssueType[] = [
  {
    id: "apology",
    label: "Apology credits",
    icon: Heart,
    description: "Goodwill credit to wallet. Customer-visible. Manager re-auth required.",
    managerAuth: true,
  },
  {
    id: "refund",
    label: "Refund / partial",
    icon: RotateCcw,
    description: "Reverse session billing. Audit-logged. Manager re-auth required.",
    managerAuth: true,
  },
  {
    id: "complaint",
    label: "Complaint log",
    icon: MessageSquare,
    description: "Record customer feedback. Routed to ops review.",
    managerAuth: false,
  },
  {
    id: "incident",
    label: "Incident report",
    icon: AlertTriangle,
    description: "Hardware fault / safety event. Routed to maintenance + ops.",
    managerAuth: false,
  },
];

export default function QuickIssue() {
  const [selected, setSelected] = React.useState<IssueType | null>(null);

  return (
    <div className="px-6 py-6 max-w-3xl space-y-6">
      <div>
        <p className="font-body uppercase text-[11px] tracking-[0.12em] text-rp-grey">
          Customer ops
        </p>
        <h1 className="font-display text-3xl font-bold text-rp-ink uppercase tracking-tight">
          Quick issue
        </h1>
      </div>

      {!selected && (
        <Panel title="Pick issue type">
          <div className="grid grid-cols-2 gap-3">
            {ISSUES.map((i) => {
              const Icon = i.icon;
              return (
                <button
                  key={i.id}
                  onClick={() => setSelected(i)}
                  className="flex flex-col items-start gap-2 px-4 py-4 rounded-md bg-rp-card border border-rp-border hover:border-rp-red hover:bg-rp-card-hi transition-colors duration-(--motion-fast) text-left"
                >
                  <div className="flex items-center justify-between w-full">
                    <Icon className="size-5 text-rp-ink" />
                    {i.managerAuth && (
                      <span className="font-mono text-[9px] uppercase tracking-wider text-rp-amber border border-rp-amber/40 rounded px-1.5 py-0.5">
                        Manager
                      </span>
                    )}
                  </div>
                  <p className="font-display font-bold uppercase text-sm tracking-wide text-rp-ink">
                    {i.label}
                  </p>
                  <p className="font-body text-xs text-rp-ink-dim leading-relaxed">{i.description}</p>
                </button>
              );
            })}
          </div>
        </Panel>
      )}

      {selected && (
        <Panel
          title={`${selected.label} · workflow`}
          action={
            <ActionButton variant="ghost" size="sm" onClick={() => setSelected(null)}>
              ← Back
            </ActionButton>
          }
        >
          <form
            onSubmit={(e) => {
              e.preventDefault();
              // TODO Phase 1: POST /api/v1/quick-issue (with manager-role auth if needed)
              console.log("quick-issue submit", selected.id);
            }}
            className="space-y-4"
          >
            {selected.managerAuth && (
              <div className="rounded-md border border-rp-amber/40 bg-rp-amber/5 px-4 py-3 flex items-start gap-3">
                <AlertTriangle className="size-4 text-rp-amber shrink-0 mt-0.5" />
                <div className="space-y-1">
                  <p className="font-body text-sm font-semibold text-rp-amber">Manager re-auth required</p>
                  <p className="font-body text-xs text-rp-ink-dim">
                    Privileged action · audit-logged · phone notify on commit
                  </p>
                </div>
              </div>
            )}

            <div className="space-y-1.5">
              <Label className="font-body uppercase text-xs tracking-[0.08em] text-rp-ink-dim">
                Customer phone (+91)
              </Label>
              <Input type="tel" placeholder="98765 43210" className="font-mono tabular-nums" />
            </div>

            {selected.id === "apology" && (
              <div className="space-y-1.5">
                <Label className="font-body uppercase text-xs tracking-[0.08em] text-rp-ink-dim">
                  Credit amount (₹)
                </Label>
                <Input type="number" placeholder="100" className="font-mono tabular-nums" />
                <p className="font-mono text-[10px] text-rp-ink-faint">
                  Wallet credit · sim/PS5 only · no expiry · subject to discount-ceiling Q4-3
                </p>
              </div>
            )}

            {selected.id === "refund" && (
              <div className="space-y-1.5">
                <Label className="font-body uppercase text-xs tracking-[0.08em] text-rp-ink-dim">
                  Session ID to reverse
                </Label>
                <Input placeholder="ses_..." className="font-mono" />
              </div>
            )}

            <div className="space-y-1.5">
              <Label className="font-body uppercase text-xs tracking-[0.08em] text-rp-ink-dim">
                Reason / notes (audit-visible)
              </Label>
              <textarea
                rows={3}
                className="w-full px-3 py-2 rounded-md bg-rp-card-hi border border-rp-border-hi focus:border-rp-red focus:outline-none focus:ring-2 focus:ring-rp-red/20 font-body text-sm text-rp-ink"
                placeholder="Customer's pod 3 had FFB outage from 17:30-17:45..."
              />
            </div>

            <div className="flex items-center gap-3 pt-2">
              <ActionButton type="submit" variant="primary" size="md">
                Submit{selected.managerAuth && " + manager auth"}
              </ActionButton>
              <ActionButton type="button" variant="ghost" size="md" onClick={() => setSelected(null)}>
                Cancel
              </ActionButton>
            </div>
          </form>
        </Panel>
      )}
    </div>
  );
}
