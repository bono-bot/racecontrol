"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";

const nav = [
  { href: "/", label: "Live Overview", icon: "&#9673;" },
  { href: "/pods", label: "Pods", icon: "&#9881;" },
  { href: "/games", label: "Games", icon: "&#127918;" },
  { href: "/telemetry", label: "Telemetry", icon: "&#128200;" },
  { href: "/ac-lan", label: "AC LAN Race", icon: "&#127937;" },
  { href: "/ac-sessions", label: "AC Results", icon: "&#127942;" },
  { href: "/sessions", label: "Sessions", icon: "&#9654;" },
  { href: "/drivers", label: "Drivers", icon: "&#9786;" },
  { href: "/leaderboards", label: "Leaderboards", icon: "&#9733;" },
  { href: "/events", label: "Events", icon: "&#9876;" },
  { href: "/billing", label: "Billing", icon: "&#128176;" },
  { href: "/billing/pricing", label: "Pricing", icon: "&#128181;" },
  { href: "/billing/history", label: "History", icon: "&#128203;" },
  { href: "/bookings", label: "Bookings", icon: "&#128197;" },
  { href: "/ai", label: "AI Insights", icon: "&#129302;" },
  { href: "/settings", label: "Settings", icon: "&#9874;" },
];

export default function Sidebar() {
  const pathname = usePathname();

  return (
    <aside className="w-56 bg-rp-black border-r border-rp-border flex flex-col min-h-screen">
      <div className="p-4 border-b border-rp-border">
        <h1 className="text-lg font-bold text-rp-red tracking-wide">RaceControl</h1>
        <p className="text-xs text-rp-grey">RacingPoint Bandlaguda</p>
      </div>
      <nav className="flex-1 py-2">
        {nav.map((item) => {
          const active = pathname === item.href;
          return (
            <Link
              key={item.href}
              href={item.href}
              className={`flex items-center gap-3 px-4 py-2.5 text-sm transition-colors ${
                active
                  ? "bg-rp-red/10 text-rp-red border-r-2 border-rp-red"
                  : "text-neutral-400 hover:text-white hover:bg-rp-card"
              }`}
            >
              <span dangerouslySetInnerHTML={{ __html: item.icon }} />
              {item.label}
            </Link>
          );
        })}
      </nav>
      <div className="p-4 border-t border-rp-border">
        <div className="flex items-center gap-2">
          <Link
            href="/presenter"
            className="text-xs text-rp-grey hover:text-rp-red transition-colors"
          >
            Presenter View
          </Link>
          <span className="text-rp-grey/50">|</span>
          <Link
            href="/kiosk"
            className="text-xs text-rp-grey hover:text-rp-red transition-colors"
          >
            Kiosk Mode
          </Link>
        </div>
      </div>
    </aside>
  );
}
