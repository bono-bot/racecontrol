"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";

const nav = [
  { href: "/", label: "Live Overview", icon: "\u25C9" },
  { href: "/pods", label: "Pods", icon: "\u2699" },
  { href: "/games", label: "Games", icon: "\uD83C\uDFAE" },
  { href: "/telemetry", label: "Telemetry", icon: "\uD83D\uDCC8" },
  { href: "/ac-lan", label: "AC LAN Race", icon: "\uD83C\uDFC1" },
  { href: "/ac-sessions", label: "AC Results", icon: "\uD83C\uDFC6" },
  { href: "/sessions", label: "Sessions", icon: "\u25B6" },
  { href: "/drivers", label: "Drivers", icon: "\u263A" },
  { href: "/leaderboards", label: "Leaderboards", icon: "\u2605" },
  { href: "/events", label: "Events", icon: "\u2694" },
  { href: "/billing", label: "Billing", icon: "\uD83D\uDCB0" },
  { href: "/billing/pricing", label: "Pricing", icon: "\uD83D\uDCB5" },
  { href: "/billing/history", label: "History", icon: "\uD83D\uDCCB" },
  { href: "/bookings", label: "Bookings", icon: "\uD83D\uDCC5" },
  { href: "/ai", label: "AI Insights", icon: "\uD83E\uDD16" },
  { href: "/cameras", label: "Cameras", icon: "\uD83D\uDCF7" },
  { href: "/cameras/playback", label: "Playback", icon: "\u23F2" },
  { href: "/cafe", label: "Cafe Menu", icon: "\u2615" },
  { href: "/settings", label: "Settings", icon: "\u2692" },
  { href: "/flags", label: "Feature Flags", icon: "\u2691" },
  { href: "/ota", label: "OTA Releases", icon: "\uD83D\uDE80" },
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
              <span>{item.icon}</span>
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
