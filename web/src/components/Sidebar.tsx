"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import {
  Activity,
  Settings2,
  Gamepad2,
  TrendingUp,
  Flag,
  Trophy,
  Play,
  User,
  Star,
  Zap,
  Wallet,
  DollarSign,
  ClipboardList,
  Calendar,
  Bot,
  Camera,
  Timer,
  Coffee,
  Wrench,
  ToggleLeft,
  Rocket,
  type LucideIcon,
} from "lucide-react";

const nav: { href: string; label: string; icon: LucideIcon }[] = [
  { href: "/", label: "Live Overview", icon: Activity },
  { href: "/pods", label: "Pods", icon: Settings2 },
  { href: "/games", label: "Games", icon: Gamepad2 },
  { href: "/telemetry", label: "Telemetry", icon: TrendingUp },
  { href: "/ac-lan", label: "AC LAN Race", icon: Flag },
  { href: "/ac-sessions", label: "AC Results", icon: Trophy },
  { href: "/sessions", label: "Sessions", icon: Play },
  { href: "/drivers", label: "Drivers", icon: User },
  { href: "/leaderboards", label: "Leaderboards", icon: Star },
  { href: "/events", label: "Events", icon: Zap },
  { href: "/billing", label: "Billing", icon: Wallet },
  { href: "/billing/pricing", label: "Pricing", icon: DollarSign },
  { href: "/billing/history", label: "History", icon: ClipboardList },
  { href: "/bookings", label: "Bookings", icon: Calendar },
  { href: "/ai", label: "AI Insights", icon: Bot },
  { href: "/cameras", label: "Cameras", icon: Camera },
  { href: "/cameras/playback", label: "Playback", icon: Timer },
  { href: "/cafe", label: "Cafe Menu", icon: Coffee },
  { href: "/settings", label: "Settings", icon: Wrench },
  { href: "/flags", label: "Feature Flags", icon: ToggleLeft },
  { href: "/ota", label: "OTA Releases", icon: Rocket },
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
              <item.icon size={16} className="shrink-0" />
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
