"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import { useEffect, useState } from "react";
import {
  LayoutDashboard,
  Cpu,
  Gamepad2,
  BarChart2,
  Flag,
  Trophy,
  Play,
  User,
  Medal,
  Swords,
  CreditCard,
  Tag,
  History,
  CalendarDays,
  Brain,
  Camera,
  Film,
  Coffee,
  Settings,
  ToggleLeft,
  Activity,
  Rocket,
  GitBranch,
  type LucideIcon,
} from "lucide-react";
import { fetchPublic } from "@/lib/api";

// --- Fleet health types ---

interface PodFleetStatus {
  pod_number: number;
  ws_connected: boolean;
  http_reachable: boolean;
  last_seen: string;
}

// --- Nav items with Lucide icons ---

const nav: { href: string; label: string; Icon: LucideIcon }[] = [
  { href: "/", label: "Live Overview", Icon: LayoutDashboard },
  { href: "/pods", label: "Pods", Icon: Cpu },
  { href: "/fleet", label: "Fleet Health", Icon: Activity },
  { href: "/metrics", label: "Metrics", Icon: BarChart2 },
  { href: "/games", label: "Games", Icon: Gamepad2 },
  { href: "/telemetry", label: "Telemetry", Icon: BarChart2 },
  { href: "/ac-lan", label: "AC LAN Race", Icon: Flag },
  { href: "/ac-sessions", label: "AC Results", Icon: Trophy },
  { href: "/sessions", label: "Sessions", Icon: Play },
  { href: "/drivers", label: "Drivers", Icon: User },
  { href: "/leaderboards", label: "Leaderboards", Icon: Medal },
  { href: "/events", label: "Events", Icon: Swords },
  { href: "/billing", label: "Billing", Icon: CreditCard },
  { href: "/billing/pricing", label: "Pricing", Icon: Tag },
  { href: "/billing/history", label: "History", Icon: History },
  { href: "/bookings", label: "Bookings", Icon: CalendarDays },
  { href: "/ai", label: "AI Insights", Icon: Brain },
  { href: "/cameras", label: "Cameras", Icon: Camera },
  { href: "/cameras/playback", label: "Playback", Icon: Film },
  { href: "/cafe", label: "Cafe Menu", Icon: Coffee },
  { href: "/settings", label: "Settings", Icon: Settings },
  { href: "/flags", label: "Feature Flags", Icon: ToggleLeft },
  { href: "/policy", label: "Policy Rules", Icon: GitBranch },
  { href: "/ota", label: "OTA Releases", Icon: Rocket },
];

export default function Sidebar() {
  const pathname = usePathname();
  const [fleet, setFleet] = useState<PodFleetStatus[]>([]);
  const [serverOk, setServerOk] = useState(true);

  // Fleet health polling — every 10s
  useEffect(() => {
    let mounted = true;
    const fetchFleet = async () => {
      try {
        const data = await fetchPublic<PodFleetStatus[]>("/fleet/health");
        if (mounted) setFleet(data);
      } catch {
        if (mounted) setFleet([]);
      }
    };
    fetchFleet();
    const interval = setInterval(fetchFleet, 10_000);
    return () => {
      mounted = false;
      clearInterval(interval);
    };
  }, []);

  // Server health polling — every 15s
  useEffect(() => {
    let mounted = true;
    const checkServer = async () => {
      try {
        const res = await fetch(
          `${process.env.NEXT_PUBLIC_API_URL || "http://localhost:8080"}/api/v1/health`
        );
        if (mounted) setServerOk(res.ok);
      } catch {
        if (mounted) setServerOk(false);
      }
    };
    checkServer();
    const interval = setInterval(checkServer, 15_000);
    return () => {
      mounted = false;
      clearInterval(interval);
    };
  }, []);

  // Sort fleet by pod number
  const sortedFleet = [...fleet].sort((a, b) => a.pod_number - b.pod_number);

  return (
    <aside className="w-56 bg-rp-black border-r border-rp-border flex flex-col min-h-screen">
      {/* Header */}
      <div className="p-4 border-b border-rp-border">
        <h1 className="text-lg font-bold text-rp-red tracking-wide">RaceControl</h1>
        <p className="text-xs text-rp-grey">RacingPoint Bandlaguda</p>
      </div>

      {/* Navigation */}
      <nav className="flex-1 py-2 overflow-y-auto">
        {nav.map((item) => {
          const active = pathname === item.href;
          return (
            <Link
              key={item.href}
              href={item.href}
              className={`flex items-center gap-3 py-2.5 text-sm transition-colors ${
                active
                  ? "bg-rp-red/10 text-rp-red border-l-4 border-rp-red pl-3"
                  : "text-neutral-400 hover:text-white hover:bg-rp-card pl-4"
              }`}
            >
              <item.Icon className="w-4 h-4 flex-shrink-0" />
              {item.label}
            </Link>
          );
        })}
      </nav>

      {/* Footer — fleet heatmap + WS indicator + links */}
      <div className="p-4 border-t border-rp-border space-y-3">
        {/* Fleet heatmap dots */}
        <div>
          <span className="text-[10px] text-rp-grey block mb-1">Fleet</span>
          <div className="flex gap-1.5 items-center">
            {sortedFleet.length > 0
              ? sortedFleet.map((pod) => {
                  const dotColor = pod.ws_connected
                    ? "bg-rp-green"
                    : pod.http_reachable
                    ? "bg-rp-yellow"
                    : "bg-rp-grey";
                  return (
                    <div
                      key={pod.pod_number}
                      className={`w-2 h-2 rounded-full ${dotColor}`}
                      title={`Pod ${pod.pod_number}`}
                    />
                  );
                })
              : Array.from({ length: 8 }).map((_, i) => (
                  <div
                    key={i}
                    className="w-2 h-2 rounded-full bg-rp-grey/30"
                    title={`Pod ${i + 1}`}
                  />
                ))}
          </div>
        </div>

        {/* Server connection indicator */}
        <div className="flex items-center gap-1.5">
          <div
            className={`w-2 h-2 rounded-full ${serverOk ? "bg-rp-green" : "bg-red-500"}`}
          />
          <span className="text-[10px] text-rp-grey">Server</span>
        </div>

        {/* Presenter / Kiosk links */}
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
