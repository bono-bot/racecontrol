"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";

const nav = [
  { href: "/", label: "Live Overview", icon: "&#9673;" },
  { href: "/pods", label: "Pods", icon: "&#9881;" },
  { href: "/games", label: "Games", icon: "&#127918;" },
  { href: "/sessions", label: "Sessions", icon: "&#9654;" },
  { href: "/drivers", label: "Drivers", icon: "&#9786;" },
  { href: "/leaderboards", label: "Leaderboards", icon: "&#9733;" },
  { href: "/events", label: "Events", icon: "&#9876;" },
  { href: "/billing", label: "Billing", icon: "&#128176;" },
  { href: "/billing/pricing", label: "Pricing", icon: "&#128181;" },
  { href: "/billing/history", label: "History", icon: "&#128203;" },
  { href: "/bookings", label: "Bookings", icon: "&#128197;" },
  { href: "/settings", label: "Settings", icon: "&#9874;" },
];

export default function Sidebar() {
  const pathname = usePathname();

  return (
    <aside className="w-56 bg-zinc-900 border-r border-zinc-800 flex flex-col min-h-screen">
      <div className="p-4 border-b border-zinc-800">
        <h1 className="text-lg font-bold text-orange-500">RaceControl</h1>
        <p className="text-xs text-zinc-500">RacingPoint Bandlaguda</p>
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
                  ? "bg-orange-500/10 text-orange-500 border-r-2 border-orange-500"
                  : "text-zinc-400 hover:text-zinc-200 hover:bg-zinc-800"
              }`}
            >
              <span dangerouslySetInnerHTML={{ __html: item.icon }} />
              {item.label}
            </Link>
          );
        })}
      </nav>
      <div className="p-4 border-t border-zinc-800">
        <div className="flex items-center gap-2">
          <Link
            href="/presenter"
            className="text-xs text-zinc-500 hover:text-orange-500 transition-colors"
          >
            Presenter View
          </Link>
          <span className="text-zinc-700">|</span>
          <Link
            href="/kiosk"
            className="text-xs text-zinc-500 hover:text-orange-500 transition-colors"
          >
            Kiosk Mode
          </Link>
        </div>
      </div>
    </aside>
  );
}
