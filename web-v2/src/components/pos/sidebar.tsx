"use client";

import * as React from "react";
import Link from "next/link";
import { usePathname } from "next/navigation";
import {
  Activity,
  Coffee,
  CreditCard,
  Search,
  UserPlus,
  AlertTriangle,
  type LucideIcon,
} from "lucide-react";
import { cn } from "@/lib/utils";

// POS sidebar nav — 5 staff workflows + cafe + cafe-orders.
// Captain bundle 2026-05-02 page-pos.jsx + V2 Q3 mid-session cafe lock.

type NavItem = {
  href: string;
  label: string;
  icon: LucideIcon;
  hint?: string;
};

const PRIMARY: NavItem[] = [
  { href: "/pos", label: "Dashboard", icon: Activity, hint: "Active state" },
  { href: "/pos/lookup", label: "Lookup", icon: Search, hint: "CIRS phone search" },
  { href: "/pos/intake", label: "Intake", icon: UserPlus, hint: "New customer" },
  { href: "/pos/sessions", label: "Sessions", icon: CreditCard, hint: "Pods + PS5" },
  { href: "/pos/quick-issue", label: "Quick issue", icon: AlertTriangle, hint: "Apology / refund" },
];

const SECONDARY: NavItem[] = [
  { href: "/pos/cafe", label: "Cafe order", icon: Coffee, hint: "Walk-in food" },
  { href: "/pos/cafe/orders", label: "Cafe queue", icon: Coffee, hint: "Pending + ready" },
];

export function PosSidebar() {
  const pathname = usePathname();

  return (
    <aside className="w-60 shrink-0 border-r border-rp-border bg-rp-dark flex flex-col">
      <div className="px-5 py-5 border-b border-rp-border">
        <p className="font-body uppercase text-[10px] tracking-[0.12em] text-rp-grey">
          POS · .130
        </p>
        <p className="font-display text-2xl font-bold text-rp-ink">
          Racing<span className="text-rp-red">Point</span>
        </p>
        <p className="font-mono text-[10px] text-rp-ink-faint">v2.0 / Phase 0.1</p>
      </div>

      <nav className="flex-1 overflow-y-auto py-4">
        <NavGroup title="Customer ops" items={PRIMARY} pathname={pathname} />
        <NavGroup title="Cafe · separate billing" items={SECONDARY} pathname={pathname} />
      </nav>

      <div className="px-5 py-3 border-t border-rp-border space-y-1">
        <p className="font-body uppercase text-[10px] tracking-[0.08em] text-rp-grey">
          Wallet · sim/PS5 only
        </p>
        <p className="font-mono text-[10px] text-rp-ink-faint">
          Cafe always separate · 18% GST at top-up
        </p>
      </div>
    </aside>
  );
}

function NavGroup({ title, items, pathname }: { title: string; items: NavItem[]; pathname: string }) {
  return (
    <div className="mb-4">
      <p className="px-5 font-body uppercase text-[10px] tracking-[0.08em] text-rp-grey mb-2">
        {title}
      </p>
      <ul className="space-y-0.5">
        {items.map((item) => {
          const active = pathname === item.href || (item.href !== "/pos" && pathname.startsWith(item.href));
          const Icon = item.icon;
          return (
            <li key={item.href}>
              <Link
                href={item.href}
                aria-current={active ? "page" : undefined}
                className={cn(
                  "flex items-center gap-3 px-5 py-2 transition-colors duration-(--motion-fast)",
                  "border-l-2 border-transparent",
                  active
                    ? "bg-rp-card-hi border-l-rp-red text-rp-ink"
                    : "text-rp-ink-dim hover:bg-rp-card hover:text-rp-ink",
                )}
              >
                <Icon className="size-4 shrink-0" />
                <div className="flex flex-col leading-tight">
                  <span className="font-body text-sm font-medium">{item.label}</span>
                  {item.hint && (
                    <span className="font-mono text-[10px] text-rp-ink-faint">{item.hint}</span>
                  )}
                </div>
              </Link>
            </li>
          );
        })}
      </ul>
    </div>
  );
}
