import * as React from "react";

// Chef Display layout — kitchen-side surface (1920×1080 fixed). No POS chrome:
// kitchen staff need maximum content area. Audio cue on new order ratifies
// per F29 sub-PACT (V2.0 pod kiosk audio cue, also applied here for chef).

export default function ChefLayout({ children }: { children: React.ReactNode }) {
  return (
    <div className="min-h-screen bg-rp-asphalt">
      <header className="px-8 py-4 bg-rp-dark border-b border-rp-border flex items-center justify-between">
        <div>
          <p className="font-body uppercase text-[10px] tracking-[0.12em] text-rp-grey">
            Kitchen display · audio cue on new order
          </p>
          <p className="font-display text-2xl font-bold text-rp-ink">
            Chef · <span className="text-rp-amber">Cafe Queue</span>
          </p>
        </div>
        <div className="text-right">
          <p className="font-mono text-2xl tabular-nums text-rp-ink">
            {/* Server-rendered time placeholder; Phase 1 hydrates with live IST clock */}
            18:14 IST
          </p>
          <p className="font-body uppercase text-[10px] tracking-[0.08em] text-rp-grey">
            2026-05-06 Wed
          </p>
        </div>
      </header>
      <main>{children}</main>
    </div>
  );
}
