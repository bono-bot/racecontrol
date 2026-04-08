"use client";

import { useState, useEffect } from "react";
import { RacingHUD } from "@/components/RacingHUD";
import type { TelemetryFrame, Lap, BillingSession } from "@/lib/types";

function useMockTelemetry(): TelemetryFrame {
  const [t, setT] = useState(0);
  useEffect(() => {
    const iv = setInterval(() => setT((p) => p + 1), 100);
    return () => clearInterval(iv);
  }, []);
  const phase = t * 0.05;
  const speed = 80 + 160 * Math.abs(Math.sin(phase * 0.3));
  const throttle = Math.max(0, Math.sin(phase * 0.4));
  const brake = Math.max(0, -Math.sin(phase * 0.4) * 0.8);
  const rpm = 3000 + speed * 40;
  const gear = speed < 80 ? 2 : speed < 130 ? 3 : speed < 180 ? 4 : speed < 230 ? 5 : 6;
  return {
    pod_id: "pod-5", driver_name: "Uday Singh", car: "Ferrari 488 GT3 Evo",
    track: "Autodromo di Monza", lap_number: Math.floor(t / 950) + 1,
    lap_time_ms: (t * 100) % 95000, speed_kmh: speed, throttle, brake, gear, rpm,
  };
}

const mockLaps: Lap[] = [
  { id: "1", driver_id: "d1", track: "monza", car: "ferrari_488_gt3", lap_number: 1, lap_time_ms: 103218, sector1_ms: 28432, sector2_ms: 34891, sector3_ms: 39895, valid: true },
  { id: "2", driver_id: "d1", track: "monza", car: "ferrari_488_gt3", lap_number: 2, lap_time_ms: 102891, sector1_ms: 28104, sector2_ms: 35012, sector3_ms: 39775, valid: true },
  { id: "3", driver_id: "d1", track: "monza", car: "ferrari_488_gt3", lap_number: 3, lap_time_ms: 101543, sector1_ms: 27891, sector2_ms: 34298, sector3_ms: 39354, valid: true },
];

const mockBilling: BillingSession = {
  id: "sess-1", pod_id: "pod-5", driver_id: "d1", driver_name: "Uday Singh",
  pricing_tier_name: "30 Min Session", status: "active", driving_state: "active",
  allocated_seconds: 1800, driving_seconds: 412, remaining_seconds: 1388,
  split_count: 1, current_split_number: 1,
  rate_per_min_paise: 2333, cost_paise: 16100, elapsed_seconds: 412,
};

export default function HUDPreview() {
  const telemetry = useMockTelemetry();
  return (
    <div className="h-screen w-screen bg-zinc-900 flex flex-col">
      {/* HUD — thin strip pinned to top */}
      <RacingHUD telemetry={telemetry} billing={mockBilling} recentLaps={mockLaps} />
      {/* Game area — takes all available space */}
      <div className="flex-1 flex items-center justify-center">
        <p className="text-zinc-700 text-lg font-mono">Game renders here (triple screen 7680x1440)</p>
      </div>
    </div>
  );
}
