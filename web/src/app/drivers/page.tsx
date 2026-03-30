"use client";

import { useEffect, useState } from "react";
import DashboardLayout from "@/components/DashboardLayout";
import { Skeleton, EmptyState } from "@/components/Skeleton";
import { Users } from "lucide-react";
import type { Driver } from "@/lib/api";
import { api } from "@/lib/api";

function formatDuration(ms: number): string {
  const hours = Math.floor(ms / 3600000);
  const minutes = Math.floor((ms % 3600000) / 60000);
  if (hours > 0) return `${hours}h ${minutes}m`;
  return `${minutes}m`;
}

export default function DriversPage() {
  const [drivers, setDrivers] = useState<Driver[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    api.listDrivers().then((res) => {
      setDrivers(res.drivers || []);
      setLoading(false);
    }).catch(() => setLoading(false));
  }, []);

  return (
    <DashboardLayout>
      <div className="flex items-center justify-between mb-6">
        <div>
          <h1 className="text-2xl font-bold text-white">Drivers</h1>
          <p className="text-sm text-rp-grey">Registered driver profiles</p>
        </div>
        <span className="text-xs text-rp-grey">{drivers.length} drivers</span>
      </div>

      {loading ? (
        <div className="grid grid-cols-2 lg:grid-cols-4 gap-4">
          {Array.from({ length: 8 }).map((_, i) => (
            <Skeleton key={i} className="h-32 rounded-lg" />
          ))}
        </div>
      ) : drivers.length === 0 ? (
        <EmptyState
          icon={<Users className="w-10 h-10" />}
          headline="No drivers registered"
          hint="Drivers can register through the kiosk or be added via the API."
        />
      ) : (
        <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4">
          {drivers.map((driver) => (
            <div
              key={driver.id}
              className="bg-rp-card border border-rp-border rounded-lg p-4"
            >
              <div className="flex items-center gap-3 mb-3">
                <div className="w-10 h-10 rounded-full bg-rp-red/20 flex items-center justify-center text-rp-red font-bold text-lg">
                  {driver.name.charAt(0).toUpperCase()}
                </div>
                <div>
                  <div className="text-neutral-200 font-medium">{driver.name}</div>
                  {driver.email && (
                    <div className="text-xs text-rp-grey">{driver.email}</div>
                  )}
                </div>
              </div>
              <div className="grid grid-cols-2 gap-2 text-xs">
                <div className="bg-rp-card/50 rounded px-2 py-1.5">
                  <div className="text-rp-grey">Laps</div>
                  <div className="text-neutral-300 font-mono">{driver.total_laps}</div>
                </div>
                <div className="bg-rp-card/50 rounded px-2 py-1.5">
                  <div className="text-rp-grey">Track Time</div>
                  <div className="text-neutral-300 font-mono">
                    {formatDuration(driver.total_time_ms)}
                  </div>
                </div>
              </div>
            </div>
          ))}
        </div>
      )}
    </DashboardLayout>
  );
}
