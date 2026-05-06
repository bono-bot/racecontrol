import * as React from "react";
import { PosSidebar } from "@/components/pos/sidebar";
import { ChefNotifications } from "@/components/pos/chef-notifications";

// POS shell — fixed 1920×1080 staff terminal layout. Sidebar nav (left) +
// header with chef notifications widget + main content area. Per Captain
// claude.ai/design bundle 2026-05-02 page-pos.jsx + Captain ratify 2026-05-06.

export default function PosLayout({ children }: { children: React.ReactNode }) {
  return (
    <div className="min-h-screen flex">
      <PosSidebar />
      <div className="flex-1 flex flex-col">
        <header className="flex items-center justify-between px-6 py-3 border-b border-rp-border bg-rp-dark">
          <div>
            <p className="font-body uppercase text-[10px] tracking-[0.12em] text-rp-grey">
              Staff terminal
            </p>
            <p className="font-display text-base font-semibold text-rp-ink">
              Racing Point Hyderabad · Bandlaguda
            </p>
          </div>
          <div className="flex items-center gap-4">
            <ChefNotifications />
          </div>
        </header>
        <main className="flex-1 overflow-y-auto bg-rp-asphalt">{children}</main>
      </div>
    </div>
  );
}
